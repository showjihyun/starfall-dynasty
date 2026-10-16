//! Historical Consequence System.
//!
//! `Player Action → DomainEvent → significance → HistoricalEvent + Evidence → Chronicle/Biography/Herald`
//!
//! The engine is event-sourced: every [`DomainEvent`] is appended to a JSONL log and
//! the whole history (events, evidence, biographies, research) is rebuilt by replaying it.
//! Simulation state is the source of truth; evidence and research are interpretations of it.

use serde::{Deserialize, Serialize};
use shared::chronicle::*;
use shared::galaxy::{self, SystemId};
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Side {
    pub name: String,
    pub members: Vec<String>,
    pub losses: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DomainEvent {
    PlayerFounded { player: String, species: String, ship: String, hull: String },
    ShipDestroyed { victim: String, victim_ship: String, victim_rank: String, killer: String, killer_ship: Option<String>, system: SystemId, witnesses: Vec<String>, cargo_lost: u32, pvp: bool, victim_bounty: u64 },
    BossDestroyed { boss: String, system: SystemId, killers: Vec<String> },
    Discovery { player: String, ship: String, system: SystemId, site: String, kind: String },
    EngagementConcluded { system: SystemId, sides: Vec<Side>, winner: Option<String>, seconds: u32 },
    ScenarioCompleted { player: String, ship: String, chapter: u32, title: String },
    ShipAscended { player: String, ship: String, designation: String },
    BountyPlaced { target: String, amount: u64, system: SystemId, reason: String },
    BountyClaimed { hunter: String, target: String, amount: u64, system: SystemId },
    WreckInvestigated { investigator: String, event: EventId, found: u32 },
    ResearchPublished { author: String, event: EventId, thesis: String, cited: Vec<u64>, historian: bool },
    ResearchDisputed { disputer: String, author: String, event: EventId },
    AllianceFounded { alliance: String, founder: String },
    WarDeclared { aggressor: String, target: String, by: String },
    PeaceMade { a: String, b: String, by: String },
    /// An Archaeologist dug a named artifact out of a ruin or gate (`artifact` is what it was known as then).
    ArtifactUnearthed { artifact_id: u64, artifact: String, culture: String, player: String, ship: String, system: SystemId, site: String },
    ArtifactAppraised { artifact_id: u64, artifact: String, player: String, value: u64 },
    /// It went down with its carrier's ship; `wreck_event` is that destruction.
    ArtifactLost { artifact_id: u64, artifact: String, player: String, system: SystemId, wreck_event: EventId },
    /// Someone investigating the wreck brought it back.
    ArtifactRecovered { artifact_id: u64, artifact: String, player: String, from: String, system: SystemId, wreck_event: EventId },
    ArtifactSold { artifact_id: u64, artifact: String, player: String, credits: u64, system: SystemId },
    ArtifactDonated { artifact_id: u64, artifact: String, player: String, system: SystemId },
    /// A forged copy of `original` with `text`. Secret: no chronicle entry, no biography line.
    EvidenceForged { forger: String, original: u64, text: String },
    /// `examiner` had `evidence` examined; research citing a forgery keeps `discredit` of its confidence.
    EvidenceAuthenticated { examiner: String, evidence: u64, discredit: f32 },
    /// A journalist's Herald article about `event`.
    StoryFiled { reporter: String, event: EventId, headline: String, body: String, cited: Vec<u64> },
}

#[derive(Serialize, Deserialize)]
struct LogLine {
    unix: u64,
    ev: DomainEvent,
}

#[derive(Default)]
pub struct HistoryEngine {
    pub events: Vec<HistoricalEvent>,
    pub evidence: Vec<Evidence>,
    /// Journalists' articles, in filing order.
    pub stories: Vec<Story>,
    bios: HashMap<String, Vec<BioEntry>>,
    ship_bios: HashMap<String, Vec<BioEntry>>,
    discovered_sites: HashSet<(SystemId, String)>,
    open_bounties: HashMap<String, EventId>,
    battles_in: HashMap<SystemId, u32>,
    /// Latest chronicle entry about each artifact, so its story links cause to effect.
    artifact_last: HashMap<u64, EventId>,
    /// Forged evidence id → (original id, forger). Only the engine knows until it is examined.
    forgeries: HashMap<u64, (u64, String)>,
    log: Option<File>,
}

pub fn ship_key(owner: &str, ship: &str) -> String {
    format!("{owner}/{ship}")
}

impl HistoryEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds the engine from a log without attaching any file (replays and tools stay read-only).
    pub fn from_log(reader: impl BufRead) -> std::io::Result<Self> {
        let mut eng = HistoryEngine::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<LogLine>(&line) {
                Ok(l) => {
                    eng.apply(l.unix, l.ev);
                }
                Err(e) => eprintln!("history: skipping corrupt line: {e}"),
            }
        }
        Ok(eng)
    }

    /// Replays an existing log (if any) and keeps appending to it.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let mut eng = if path.exists() { Self::from_log(BufReader::new(File::open(path)?))? } else { HistoryEngine::new() };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        eng.log = Some(OpenOptions::new().create(true).append(true).open(path)?);
        Ok(eng)
    }

    /// Record a domain event: persisted first, then applied. Returns the historical event id if one was created.
    pub fn record(&mut self, unix: u64, ev: DomainEvent) -> Option<EventId> {
        if let Some(f) = self.log.as_mut() {
            let line = serde_json::to_string(&LogLine { unix, ev: ev.clone() }).expect("serialize");
            if let Err(e) = writeln!(f, "{line}") {
                eprintln!("history: log write failed: {e}");
            }
        }
        self.apply(unix, ev)
    }

    fn next_id(&self) -> EventId {
        self.events.len() as EventId + 1
    }

    fn bio(&mut self, who: &str, unix: u64, text: String, event: Option<EventId>) {
        self.bios.entry(who.to_string()).or_default().push(BioEntry { date: galactic_date(unix), text, event });
    }

    fn ship_bio(&mut self, key: String, unix: u64, text: String, event: Option<EventId>) {
        self.ship_bios.entry(key).or_default().push(BioEntry { date: galactic_date(unix), text, event });
    }

    fn add_evidence(&mut self, event: EventId, kind: EvidenceKind, source: &str, text: String, reliability: f32) -> u64 {
        let id = self.evidence.len() as u64 + 1;
        self.evidence.push(Evidence { id, kind, event, source: source.to_string(), text, reliability, discovered_by: None, authenticity: Authenticity::Unverified });
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn push_event(&mut self, unix: u64, system: SystemId, kind: &str, title: String, summary: String, actors: Vec<String>, ships: Vec<String>, importance: Importance, causes: Vec<EventId>) -> EventId {
        let id = self.next_id();
        self.events.push(HistoricalEvent { id, date: galactic_date(unix), unix, system, kind: kind.into(), title, summary, actors, ships, importance, causes, evidence: vec![], research: vec![] });
        id
    }

    fn attach(&mut self, event: EventId, ev_ids: Vec<u64>) {
        if let Some(e) = self.events.iter_mut().find(|e| e.id == event) {
            e.evidence.extend(ev_ids);
        }
    }

    pub fn apply(&mut self, unix: u64, ev: DomainEvent) -> Option<EventId> {
        match ev {
            ev @ (DomainEvent::AllianceFounded { .. } | DomainEvent::WarDeclared { .. } | DomainEvent::PeaceMade { .. }) => self.apply_diplomacy(unix, ev),
            ev @ (DomainEvent::EvidenceForged { .. } | DomainEvent::EvidenceAuthenticated { .. }) => self.apply_forgery(unix, ev),
            DomainEvent::StoryFiled { reporter, event, headline, body, cited } => {
                let title = self.events.iter().find(|e| e.id == event)?.title.clone();
                let rel: Vec<f32> = cited.iter().filter_map(|c| self.evidence.iter().find(|e| e.id == *c && e.event == event)).map(|e| e.reliability).collect();
                let reliability = if rel.is_empty() { 0.5 } else { rel.iter().sum::<f32>() / rel.len() as f32 };
                let id = self.stories.len() as u64 + 1;
                self.stories.push(Story { id, date: galactic_date(unix), unix, event, reporter: reporter.clone(), headline: headline.clone(), body, cited, reliability, retracted: false });
                self.bio(&reporter, unix, format!("「{title}」에 관한 기사 「{headline}」을(를) Herald에 송고했다."), Some(event));
                None
            }
            ev @ (DomainEvent::ArtifactUnearthed { .. }
            | DomainEvent::ArtifactAppraised { .. }
            | DomainEvent::ArtifactLost { .. }
            | DomainEvent::ArtifactRecovered { .. }
            | DomainEvent::ArtifactSold { .. }
            | DomainEvent::ArtifactDonated { .. }) => self.apply_artifact(unix, ev),
            DomainEvent::PlayerFounded { player, species, ship, hull } => {
                self.bio(&player, unix, format!("{species} 출신 함장으로 등록. 평생의 함선 「{ship}」({hull})을 인수했다."), None);
                self.ship_bio(ship_key(&player, &ship), unix, format!("진수. 함장 {player}. 함급 {hull}."), None);
                None
            }
            DomainEvent::ShipDestroyed { victim, victim_ship, victim_rank, killer, killer_ship, system, witnesses, cargo_lost, pvp, victim_bounty } => {
                let sys = galaxy::system(system).name;
                let mut imp = if pvp { Importance::Notable } else { Importance::Minor };
                if victim_bounty > 0 || cargo_lost > 30 || witnesses.len() >= 3 {
                    imp = imp.max(Importance::Notable);
                }
                if pvp && (victim_bounty > 5000 || victim_rank_index(&victim_rank) >= 5) {
                    imp = Importance::Major;
                }
                let causes: Vec<EventId> = self.open_bounties.get(&victim).copied().into_iter().collect();
                let title = format!("{victim_ship} 격침 — {sys}");
                let summary = format!(
                    "{victim}의 {victim_rank}급 함선 「{victim_ship}」이(가) {sys}에서 {killer}{}에 의해 파괴되었다.{}{}",
                    killer_ship.as_ref().map(|s| format!("(「{s}」)")).unwrap_or_default(),
                    if cargo_lost > 0 { format!(" 화물 {cargo_lost}단위가 우주에 흩어졌다.") } else { String::new() },
                    if witnesses.is_empty() { String::new() } else { format!(" 목격자 {}명.", witnesses.len()) }
                );
                let mut actors = vec![victim.clone(), killer.clone()];
                actors.extend(witnesses.iter().cloned());
                let mut ships = vec![victim_ship.clone()];
                ships.extend(killer_ship.clone());
                let id = self.push_event(unix, system, "SHIP_DESTROYED", title, summary, actors, ships, imp, causes);
                // Evidence: the wreck's log (found only by investigating the wreck), killer sensors, witnesses.
                let mut evs = vec![self.add_evidence(
                    id,
                    EvidenceKind::ShipLog,
                    &format!("「{victim_ship}」 블랙박스"),
                    format!("[{}] 마지막 기록: 접촉 — {killer}. 실드 붕괴. 선체 파손. 교신 두절.", galactic_date(unix)),
                    0.95,
                )];
                if let Some(ks) = &killer_ship {
                    evs.push(self.add_evidence(id, EvidenceKind::SensorRecord, &format!("「{ks}」 사격 통제 기록"), format!("표적 「{victim_ship}」 교전 및 격침 확인."), 0.8));
                }
                for w in witnesses.iter().take(3) {
                    evs.push(self.add_evidence(id, EvidenceKind::Testimony, w, format!("{w}의 증언: \"{sys} 근처에서 {killer}이(가) 먼저 발포하는 걸 봤다.\""), 0.55));
                }
                self.attach(id, evs);
                self.bio(&victim, unix, format!("{sys}에서 {killer}에게 격침당했다."), Some(id));
                self.bio(&killer, unix, format!("{sys}에서 {victim}의 「{victim_ship}」을 격침했다."), Some(id));
                self.ship_bio(ship_key(&victim, &victim_ship), unix, format!("{sys}에서 격침. 정거장에서 재건됨."), Some(id));
                if let Some(ks) = killer_ship {
                    self.ship_bio(ship_key(&killer, &ks), unix, format!("{victim}의 「{victim_ship}」 격침."), Some(id));
                }
                Some(id)
            }
            DomainEvent::BossDestroyed { boss, system, killers } => {
                let sys = galaxy::system(system).name;
                let id = self.push_event(unix, system, "BOSS_DESTROYED", format!("{boss} 몰락"), format!("{sys}에서 {}이(가) {boss}을(를) 쓰러뜨렸다.", killers.join(", ")), killers.clone(), vec![], Importance::Major, vec![]);
                let e = self.add_evidence(id, EvidenceKind::SensorRecord, "항로 감시 부표", format!("{boss}의 반응로 붕괴 신호 감지."), 0.9);
                self.attach(id, vec![e]);
                for k in &killers {
                    self.bio(k, unix, format!("{sys}에서 {boss} 격파에 참여했다."), Some(id));
                }
                Some(id)
            }
            DomainEvent::Discovery { player, ship, system, site, kind } => {
                let sys = galaxy::system(system).name;
                let first = self.discovered_sites.insert((system, site.clone()));
                if !first {
                    self.bio(&player, unix, format!("{sys}의 {site}을(를) 조사했다."), None);
                    return None;
                }
                let imp = if kind == "Ruin" || kind == "AncientGate" { Importance::Major } else { Importance::Notable };
                let id = self.push_event(unix, system, "DISCOVERY", format!("{site} 최초 발견"), format!("{player}이(가) 「{ship}」으로 {sys}의 {site}을(를) 최초로 기록했다."), vec![player.clone()], vec![ship.clone()], imp, vec![]);
                let e1 = self.add_evidence(id, EvidenceKind::ArchiveRecord, "항로국 등록부", format!("{site} 좌표 등록. 발견자 {player}."), 0.9);
                let e2 = self.add_evidence(id, EvidenceKind::VeyrMemory, "Veyr 기억의 집", format!("{site}은(는) 이미 세 번째 순환 때 기억되었다고 전해진다."), 0.45);
                self.attach(id, vec![e1, e2]);
                self.bio(&player, unix, format!("{sys}의 {site}을(를) 최초 발견했다."), Some(id));
                self.ship_bio(ship_key(&player, &ship), unix, format!("{site} 최초 발견."), Some(id));
                Some(id)
            }
            DomainEvent::EngagementConcluded { system, sides, winner, seconds } => {
                let sys = galaxy::system(system).name;
                let n = self.battles_in.entry(system).or_insert(0);
                *n += 1;
                let ordinal = *n;
                let participants: Vec<String> = sides.iter().flat_map(|s| s.members.clone()).collect();
                let losses: u32 = sides.iter().map(|s| s.losses).sum();
                let imp = if participants.len() >= 6 || losses >= 4 {
                    Importance::Historic
                } else if participants.len() >= 3 || losses >= 2 {
                    Importance::Major
                } else {
                    Importance::Notable
                };
                let title = if ordinal == 1 { format!("{sys} 전투") } else { format!("제{ordinal}차 {sys} 전투") };
                let side_desc: Vec<String> = sides.iter().map(|s| format!("{}({}척, 손실 {})", s.name, s.members.len(), s.losses)).collect();
                let summary = format!(
                    "{}초간 이어진 교전. {}. {}",
                    seconds,
                    side_desc.join(" vs "),
                    match &winner {
                        Some(w) => format!("{w} 측이 전장을 장악했다."),
                        None => "승자 없이 양측이 물러났다.".into(),
                    }
                );
                // Link to the destructions that happened during the battle window.
                let window_start = unix.saturating_sub(seconds as u64 + 5);
                let causes: Vec<EventId> = self.events.iter().filter(|e| e.kind == "SHIP_DESTROYED" && e.system == system && e.unix >= window_start).map(|e| e.id).collect();
                let id = self.push_event(unix, system, "BATTLE", title.clone(), summary, participants.clone(), vec![], imp, causes);
                let mut evs = Vec::new();
                for s in &sides {
                    let kind = if s.name.contains("Kharz") { EvidenceKind::KharzContract } else { EvidenceKind::ArchiveRecord };
                    let claim = if Some(&s.name) == winner.as_ref() { "우리는 방어전을 승리로 이끌었다." } else { "우리는 기습을 받았고 질서 있게 후퇴했다." };
                    evs.push(self.add_evidence(id, kind, &format!("{} 전투 보고서", s.name), claim.into(), 0.5));
                }
                self.attach(id, evs);
                for p in &participants {
                    self.bio(p, unix, format!("{title}에 참전했다."), Some(id));
                }
                Some(id)
            }
            DomainEvent::ScenarioCompleted { player, ship, chapter, title } => {
                let imp = if chapter >= 4 { Importance::Historic } else if chapter >= 2 { Importance::Major } else { Importance::Notable };
                let id = self.push_event(unix, 0, "SCENARIO", format!("{player}, {title} 완수"), format!("{player}와(과) 「{ship}」이(가) {title}의 위기를 해결했다."), vec![player.clone()], vec![ship.clone()], imp, vec![]);
                self.bio(&player, unix, format!("{title}을(를) 완수했다."), Some(id));
                self.ship_bio(ship_key(&player, &ship), unix, format!("{title} 작전 참여."), Some(id));
                Some(id)
            }
            DomainEvent::ShipAscended { player, ship, designation } => {
                self.bio(&player, unix, format!("「{ship}」이(가) {designation}급으로 개장되었다."), None);
                self.ship_bio(ship_key(&player, &ship), unix, format!("대규모 개장 — {designation}급 판정."), None);
                if victim_rank_index(&designation) >= 6 {
                    return Some(self.push_event(unix, 0, "ASCENSION", format!("「{ship}」 {designation}급 진급"), format!("{player}의 함선이 은하에서 손꼽히는 {designation}급에 올랐다."), vec![player], vec![ship], Importance::Notable, vec![]));
                }
                None
            }
            DomainEvent::BountyPlaced { target, amount, system, reason } => {
                let sys = galaxy::system(system).name;
                let imp = if amount >= 5000 { Importance::Notable } else { Importance::Minor };
                let id = self.push_event(unix, system, "BOUNTY", format!("현상수배: {target}"), format!("{sys}에서 {reason} 혐의. 현상금 {amount} CR."), vec![target.clone()], vec![], imp, vec![]);
                self.open_bounties.insert(target.clone(), id);
                self.bio(&target, unix, format!("{sys}에서 {reason} 혐의로 수배되었다 ({amount} CR)."), Some(id));
                Some(id)
            }
            DomainEvent::BountyClaimed { hunter, target, amount, system } => {
                let sys = galaxy::system(system).name;
                let causes: Vec<EventId> = self.open_bounties.remove(&target).into_iter().collect();
                let id = self.push_event(unix, system, "BOUNTY_CLAIMED", format!("{target} 현상금 회수"), format!("현상금 사냥꾼 {hunter}이(가) {sys}에서 {target}을(를) 처단하고 {amount} CR을 받았다."), vec![hunter.clone(), target.clone()], vec![], Importance::Notable, causes);
                self.bio(&hunter, unix, format!("{target}의 현상금 {amount} CR을 회수했다."), Some(id));
                Some(id)
            }
            DomainEvent::WreckInvestigated { investigator, event, found } => {
                let mut newly = 0;
                for ev in self.evidence.iter_mut().filter(|e| e.event == event && e.discovered_by.is_none()) {
                    ev.discovered_by = Some(investigator.clone());
                    newly += 1;
                }
                let title = self.events.iter().find(|e| e.id == event).map(|e| e.title.clone()).unwrap_or_default();
                self.bio(&investigator, unix, format!("「{title}」 현장을 조사하여 증거 {}건을 확보했다.", found.max(newly)), Some(event));
                None
            }
            DomainEvent::ResearchPublished { author, event, thesis, cited, historian } => {
                let rel: Vec<f32> = cited.iter().filter_map(|c| self.evidence.iter().find(|e| e.id == *c && e.event == event)).map(|e| e.reliability).collect();
                if rel.is_empty() {
                    return None;
                }
                let kinds: HashSet<_> = cited.iter().filter_map(|c| self.evidence.iter().find(|e| e.id == *c)).map(|e| e.kind as u8).collect();
                // Corroboration across independent source kinds raises confidence.
                let mean = rel.iter().sum::<f32>() / rel.len() as f32;
                let confidence = (mean * (0.7 + 0.15 * kinds.len() as f32) + if historian { 0.1 } else { 0.0 }).min(0.99);
                let title = {
                    let e = self.events.iter_mut().find(|e| e.id == event)?;
                    e.research.retain(|r| r.author != author);
                    e.research.push(Research { author: author.clone(), thesis: thesis.clone(), cited, confidence, disputed_by: vec![], discredited: false });
                    // Researched events become more significant over time.
                    if e.importance < Importance::Major {
                        e.importance = match e.importance {
                            Importance::Trivial => Importance::Minor,
                            Importance::Minor => Importance::Notable,
                            _ => Importance::Major,
                        };
                    }
                    e.title.clone()
                };
                self.bio(&author, unix, format!("「{title}」에 관한 연구를 발표했다: \"{thesis}\""), Some(event));
                Some(event)
            }
            DomainEvent::ResearchDisputed { disputer, author, event } => {
                let (title, system, controversy) = {
                    let e = self.events.iter_mut().find(|e| e.id == event)?;
                    let r = e.research.iter_mut().find(|r| r.author == author)?;
                    if disputer == author || r.disputed_by.contains(&disputer) {
                        return None;
                    }
                    r.disputed_by.push(disputer.clone());
                    r.confidence = (r.confidence * 0.8).max(0.05);
                    (e.title.clone(), e.system, r.disputed_by.len() == 2)
                };
                self.bio(&disputer, unix, format!("{author}의 「{title}」 연구를 반박했다."), Some(event));
                self.bio(&author, unix, format!("「{title}」 연구가 {disputer}에게 반박당했다."), Some(event));
                if controversy {
                    let id = self.push_event(unix, system, "CONTROVERSY", format!("「{title}」 역사 논쟁"), format!("{author}의 해석을 두고 학계가 둘로 갈라졌다."), vec![author, disputer], vec![], Importance::Major, vec![event]);
                    return Some(id);
                }
                Some(event)
            }
        }
    }

    /// Who forged a piece of evidence (server side only; clients never see this).
    pub fn forger_of(&self, evidence: u64) -> Option<&str> {
        self.forgeries.get(&evidence).map(|(_, f)| f.as_str())
    }

    /// Forgery (design doc §53–54): a forged copy is indistinguishable from its original until it is
    /// examined; then it is marked, the research built on it is discredited and the exposure is news.
    fn apply_forgery(&mut self, unix: u64, ev: DomainEvent) -> Option<EventId> {
        match ev {
            DomainEvent::EvidenceForged { forger, original, text } => {
                let o = self.evidence.iter().find(|e| e.id == original)?.clone();
                let id = self.evidence.len() as u64 + 1;
                self.evidence.push(Evidence { id, kind: o.kind, event: o.event, source: o.source, text, reliability: o.reliability, discovered_by: Some(forger.clone()), authenticity: Authenticity::Unverified });
                self.forgeries.insert(id, (original, forger));
                None
            }
            DomainEvent::EvidenceAuthenticated { examiner, evidence, discredit } => {
                let idx = self.evidence.iter().position(|e| e.id == evidence)?;
                if self.evidence[idx].authenticity != Authenticity::Unverified {
                    return None;
                }
                let event = self.evidence[idx].event;
                let title = self.events.iter().find(|e| e.id == event).map(|e| e.title.clone()).unwrap_or_default();
                let Some((original, forger)) = self.forgeries.get(&evidence).cloned() else {
                    self.evidence[idx].authenticity = Authenticity::Authentic;
                    self.bio(&examiner, unix, format!("「{title}」의 증거를 감정해 진품임을 확인했다."), Some(event));
                    return None;
                };
                self.evidence[idx].authenticity = Authenticity::Forged;
                let source = self.evidence[idx].source.clone();
                let mut fooled = vec![];
                let system = {
                    let e = self.events.iter_mut().find(|e| e.id == event)?;
                    for r in e.research.iter_mut().filter(|r| r.cited.contains(&evidence) && !r.discredited) {
                        r.discredited = true;
                        r.confidence = (r.confidence * discredit).max(0.01);
                        fooled.push(r.author.clone());
                    }
                    e.system
                };
                let summary = format!(
                    "{examiner}의 감정 결과, 「{title}」에 관한 {source} 사본은 {forger}이(가) 원본(증거 #{original})을 고쳐 만든 위조품으로 드러났다.{}",
                    if fooled.is_empty() { String::new() } else { format!(" 이를 인용한 연구({})는 신뢰를 잃었다.", fooled.join(", ")) }
                );
                let id = self.push_event(unix, system, "FORGERY_EXPOSED", format!("「{title}」 증거 위조 폭로"), summary, vec![forger.clone(), examiner.clone()], vec![], Importance::Major, vec![event]);
                let e = self.add_evidence(id, EvidenceKind::ArchiveRecord, &format!("{examiner}의 감정 보고서"), format!("증거 #{evidence}: 원본 #{original}과 문체·기록 시각 불일치. 위조로 판정."), 0.9);
                self.attach(id, vec![e]);
                self.bio(&examiner, unix, format!("「{title}」의 위조 증거를 밝혀냈다. 위조자는 {forger}."), Some(id));
                self.bio(&forger, unix, format!("「{title}」 증거를 위조한 사실이 {examiner}에게 폭로되었다."), Some(id));
                for a in fooled.iter().filter(|a| **a != forger) {
                    self.bio(a, unix, format!("「{title}」 연구가 위조 증거를 인용한 것으로 드러났다."), Some(id));
                }
                // Articles built on it run as corrections.
                let mut retracted = vec![];
                for st in self.stories.iter_mut().filter(|s| s.cited.contains(&evidence) && !s.retracted) {
                    st.retracted = true;
                    retracted.push((st.reporter.clone(), st.headline.clone()));
                }
                for (reporter, headline) in retracted {
                    self.bio(&reporter, unix, format!("기사 「{headline}」이(가) 위조 증거에 근거한 것으로 드러나 정정되었다."), Some(id));
                }
                Some(id)
            }
            _ => None,
        }
    }

    /// Artifacts: discovery, loss with a ship, recovery from the wreck, sale and donation. Each entry
    /// names the previous one about the same artifact as its cause.
    fn apply_artifact(&mut self, unix: u64, ev: DomainEvent) -> Option<EventId> {
        let prior = |s: &Self, id: u64| s.artifact_last.get(&id).copied();
        let (artifact_id, event) = match ev {
            DomainEvent::ArtifactUnearthed { artifact_id, artifact, culture, player, ship, system, site } => {
                let sys = galaxy::system(system).name;
                let imp = if culture == "선구자" { Importance::Major } else { Importance::Notable };
                let id = self.push_event(unix, system, "ARTIFACT_UNEARTHED", format!("{site}에서 {culture} 유물 발굴"), format!("고고학자 {player}이(가) {sys}의 {site}에서 {artifact}을(를) 발굴했다."), vec![player.clone()], vec![ship.clone()], imp, vec![]);
                let e = self.add_evidence(id, EvidenceKind::ArchiveRecord, "고고학 발굴 대장", format!("{site} 발굴 허가 및 출토 기록: {artifact}. 발굴자 {player}."), 0.9);
                self.attach(id, vec![e]);
                self.bio(&player, unix, format!("{sys}의 {site}에서 {artifact}을(를) 발굴했다."), Some(id));
                self.ship_bio(ship_key(&player, &ship), unix, format!("{site} 발굴 지원."), Some(id));
                (artifact_id, Some(id))
            }
            DomainEvent::ArtifactAppraised { artifact_id, artifact, player, value } => {
                self.bio(&player, unix, format!("유물을 감정했다: {artifact} (감정가 {value} CR)."), prior(self, artifact_id));
                (artifact_id, None)
            }
            DomainEvent::ArtifactLost { artifact_id, artifact, player, system, wreck_event } => {
                let sys = galaxy::system(system).name;
                let causes: Vec<EventId> = [Some(wreck_event), prior(self, artifact_id)].into_iter().flatten().collect();
                let id = self.push_event(unix, system, "ARTIFACT_LOST", format!("{artifact} 실종"), format!("{player}의 함선과 함께 {artifact}이(가) {sys}의 잔해 속으로 사라졌다. 잔해를 조사하는 자가 그것을 찾게 될 것이다."), vec![player.clone()], vec![], Importance::Notable, causes);
                self.bio(&player, unix, format!("격침과 함께 {artifact}을(를) 잃었다."), Some(id));
                (artifact_id, Some(id))
            }
            DomainEvent::ArtifactRecovered { artifact_id, artifact, player, from, system, wreck_event } => {
                let sys = galaxy::system(system).name;
                let causes: Vec<EventId> = [prior(self, artifact_id), Some(wreck_event)].into_iter().flatten().collect();
                let id = self.push_event(unix, system, "ARTIFACT_RECOVERED", format!("잃어버린 {artifact} 회수"), format!("{player}이(가) {sys}에서 {from}의 잔해를 조사하던 중 {artifact}을(를) 되찾았다."), vec![player.clone(), from.clone()], vec![], Importance::Major, causes);
                let e = self.add_evidence(id, EvidenceKind::Testimony, &format!("{player}의 조사 일지"), format!("잔해 화물칸 깊은 곳에서 {artifact} 발견. 봉인 상태 양호."), 0.7);
                self.attach(id, vec![e]);
                self.bio(&player, unix, format!("{from}의 잔해에서 {artifact}을(를) 회수했다."), Some(id));
                (artifact_id, Some(id))
            }
            DomainEvent::ArtifactSold { artifact_id, artifact, player, credits, system } => {
                let sys = galaxy::system(system).name;
                let imp = if credits >= 5000 { Importance::Notable } else { Importance::Minor };
                let causes: Vec<EventId> = prior(self, artifact_id).into_iter().collect();
                let id = self.push_event(unix, system, "ARTIFACT_SOLD", format!("{artifact}, 수집가에게"), format!("{player}이(가) {sys} 정거장에서 {artifact}을(를) {credits} CR에 팔았다. 유물은 개인 소장품이 되어 공공의 눈에서 사라졌다."), vec![player.clone()], vec![], imp, causes);
                self.bio(&player, unix, format!("{artifact}을(를) {credits} CR에 팔았다."), Some(id));
                (artifact_id, Some(id))
            }
            DomainEvent::ArtifactDonated { artifact_id, artifact, player, system } => {
                let sys = galaxy::system(system).name;
                let causes: Vec<EventId> = prior(self, artifact_id).into_iter().collect();
                let id = self.push_event(unix, system, "ARTIFACT_DONATED", format!("{artifact}, {sys} 박물관에"), format!("{player}이(가) {artifact}을(를) {sys} 정거장 박물관에 기증했다. 누구나 그 앞에서 역사를 볼 수 있게 되었다."), vec![player.clone()], vec![], Importance::Major, causes);
                let e = self.add_evidence(id, EvidenceKind::ArchiveRecord, &format!("{sys} 박물관 소장 목록"), format!("{artifact} — 기증자 {player}."), 0.95);
                self.attach(id, vec![e]);
                self.bio(&player, unix, format!("{artifact}을(를) {sys} 박물관에 기증했다."), Some(id));
                (artifact_id, Some(id))
            }
            _ => return None,
        };
        if let Some(e) = event {
            self.artifact_last.insert(artifact_id, e);
        }
        event
    }

    fn apply_diplomacy(&mut self, unix: u64, ev: DomainEvent) -> Option<EventId> {
        match ev {
            DomainEvent::AllianceFounded { alliance, founder } => {
                let id = self.push_event(unix, 0, "ALLIANCE", format!("{alliance} 결성"), format!("{founder}이(가) 동맹 「{alliance}」을(를) 창설했다."), vec![founder.clone()], vec![], Importance::Notable, vec![]);
                let e = self.add_evidence(id, EvidenceKind::KharzContract, &format!("{alliance} 창립 서약"), format!("「{alliance}」의 깃발 아래 서로를 쏘지 않으며, 적을 함께 맞는다."), 0.9);
                self.attach(id, vec![e]);
                self.bio(&founder, unix, format!("동맹 「{alliance}」을(를) 창설했다."), Some(id));
                Some(id)
            }
            DomainEvent::WarDeclared { aggressor, target, by } => {
                let id = self.push_event(unix, 0, "WAR", format!("{aggressor}, {target}에 선전포고"), format!("{by}의 이름으로 「{aggressor}」이(가) 「{target}」에 전쟁을 선포했다. 이제 두 동맹의 함대는 어디서든 서로를 노린다."), vec![by.clone()], vec![], Importance::Major, vec![]);
                let e = self.add_evidence(id, EvidenceKind::ArchiveRecord, "은하 공보", format!("{aggressor} 측 선전포고문 공표."), 0.95);
                self.attach(id, vec![e]);
                self.bio(&by, unix, format!("「{target}」에 선전포고했다."), Some(id));
                Some(id)
            }
            DomainEvent::PeaceMade { a, b, by } => {
                let causes: Vec<EventId> = self.events.iter().filter(|e| e.kind == "WAR" && e.title.contains(&a) && e.title.contains(&b)).map(|e| e.id).collect();
                let id = self.push_event(unix, 0, "PEACE", format!("{a}–{b} 강화"), format!("{by}의 제안으로 「{a}」과(와) 「{b}」 사이의 전쟁이 끝났다."), vec![by.clone()], vec![], Importance::Major, causes);
                self.bio(&by, unix, format!("「{a}」–「{b}」 강화를 성사시켰다."), Some(id));
                Some(id)
            }
            _ => None,
        }
    }

    pub fn chronicle(&self, min: Importance, limit: usize) -> Vec<HistoricalEvent> {
        self.events.iter().rev().filter(|e| e.importance >= min).take(limit).cloned().collect()
    }

    pub fn event(&self, id: EventId) -> Option<&HistoricalEvent> {
        self.events.iter().find(|e| e.id == id)
    }

    /// Evidence that has been discovered (publicly known) or belongs to `viewer`'s archive.
    pub fn visible_evidence(&self, events: &[HistoricalEvent], archive: &[u64]) -> Vec<Evidence> {
        let ids: HashSet<EventId> = events.iter().map(|e| e.id).collect();
        self.evidence
            .iter()
            .filter(|e| ids.contains(&e.event))
            .map(|e| {
                let mut e = e.clone();
                // Undiscovered ship logs stay sealed until someone investigates the wreck (fog of history).
                if e.discovered_by.is_none() && e.kind == EvidenceKind::ShipLog && !archive.contains(&e.id) {
                    e.text = "[봉인됨 — 잔해를 조사해야 열람 가능]".into();
                }
                e
            })
            .collect()
    }

    /// Ids of evidence for `event` that an investigator obtains from its wreck.
    pub fn evidence_from_wreck(&self, event: EventId, double: bool) -> Vec<u64> {
        let mut v: Vec<u64> = self.evidence.iter().filter(|e| e.event == event).map(|e| e.id).collect();
        if !double {
            v.truncate(2);
        }
        v
    }

    pub fn biography(&self, name: &str) -> Vec<BioEntry> {
        self.bios.get(name).cloned().unwrap_or_default()
    }

    pub fn ship_biography(&self, owner: &str, ship: &str) -> Vec<BioEntry> {
        self.ship_bios.get(&ship_key(owner, ship)).cloned().unwrap_or_default()
    }

    /// Galactic Herald: template-generated news from simulation facts (never invents events).
    /// The Galactic Herald: journalists' articles (corrections included) and, for events nobody has
    /// written up, the automatic headline — newest first.
    pub fn herald(&self, limit: usize) -> Vec<Headline> {
        let covered: HashSet<EventId> = self.stories.iter().filter(|s| !s.retracted).map(|s| s.event).collect();
        let mut dated: Vec<(u64, usize, Headline)> = self.auto_headlines(limit, &covered).into_iter().enumerate().map(|(i, (u, h))| (u, i, h)).collect();
        for s in self.stories.iter().rev().take(limit) {
            let headline = if s.retracted { format!("[정정] {}", s.headline) } else { s.headline.clone() };
            let body = if s.retracted {
                format!("이 기사가 근거로 삼은 증거는 위조로 판명되었다. 원문: {}", s.body)
            } else {
                format!("{} (근거 신뢰도 {:.0}%)", s.body, s.reliability * 100.0)
            };
            // Articles sort ahead of automatic headlines from the same moment.
            dated.push((s.unix, usize::MAX - s.id as usize, Headline { date: s.date.clone(), headline, body, by: Some(s.reporter.clone()), story: Some(s.id) }));
        }
        dated.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        dated.into_iter().take(limit).map(|d| d.2).collect()
    }

    fn auto_headlines(&self, limit: usize, covered: &HashSet<EventId>) -> Vec<(u64, Headline)> {
        let mut out = Vec::new();
        for e in self.events.iter().rev().filter(|e| e.importance >= Importance::Notable && !covered.contains(&e.id)).take(limit) {
            let sys = galaxy::system(e.system).name.to_uppercase();
            let headline = match e.kind.as_str() {
                "BATTLE" => format!("{} — 포연이 걷히다", e.title.to_uppercase()),
                "SHIP_DESTROYED" => format!("{sys}에서 또 하나의 함선이 사라지다"),
                "DISCOVERY" => format!("새로운 발견: {}", e.title),
                "BOUNTY" => format!("수배령: {}", e.actors.first().cloned().unwrap_or_default()),
                "BOUNTY_CLAIMED" => format!("정의인가 사냥인가 — {}", e.title),
                "SCENARIO" => format!("영웅의 귀환: {}", e.title),
                "CONTROVERSY" => format!("학계 격돌: {}", e.title),
                "BOSS_DESTROYED" => format!("공포의 종말: {}", e.title),
                "WAR" => format!("전쟁 발발 — {}", e.title),
                "PEACE" => format!("포성이 멎다: {}", e.title),
                "ALLIANCE" => format!("새 깃발: {}", e.title),
                "ARTIFACT_UNEARTHED" => format!("땅속의 역사: {}", e.title),
                "FORGERY_EXPOSED" => format!("위조 스캔들: {}", e.title),
                "ARTIFACT_LOST" => format!("사라진 유물 — {}", e.title),
                "ARTIFACT_RECOVERED" => format!("돌아온 유물: {}", e.title),
                "ARTIFACT_SOLD" => format!("경매장 소식: {}", e.title),
                "ARTIFACT_DONATED" => format!("박물관에 새 전시: {}", e.title),
                _ => e.title.clone(),
            };
            let mut body = e.summary.clone();
            if let Some(r) = e.research.iter().filter(|r| !r.discredited).max_by(|a, b| a.confidence.total_cmp(&b.confidence)) {
                body.push_str(&format!(" 역사가 {}는 \"{}\"라고 주장한다(신뢰도 {:.0}%).", r.author, r.thesis, r.confidence * 100.0));
            }
            out.push((e.unix, Headline { date: e.date.clone(), headline, body, by: None, story: None }));
        }
        out
    }
}

fn victim_rank_index(designation: &str) -> usize {
    shared::ships::RANKS.iter().position(|r| *r == designation).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn destroyed(victim: &str, killer: &str, pvp: bool) -> DomainEvent {
        DomainEvent::ShipDestroyed {
            victim: victim.into(),
            victim_ship: format!("{victim}-ship"),
            victim_rank: "Scout".into(),
            killer: killer.into(),
            killer_ship: Some(format!("{killer}-ship")),
            system: 6,
            witnesses: vec!["W1".into()],
            cargo_lost: 40,
            pvp,
            victim_bounty: 0,
        }
    }

    /// The design doc's key scene: A destroyed by B → chronicle; C investigates the wreck,
    /// publishes research; D disputes it; controversy becomes its own historical event.
    #[test]
    fn key_scene_end_to_end() {
        let mut h = HistoryEngine::new();
        let ev = h.record(100, destroyed("A", "B", true)).unwrap();
        assert!(h.chronicle(Importance::Notable, 10).iter().any(|e| e.id == ev));
        let log_id = h.evidence.iter().find(|e| e.event == ev && e.kind == EvidenceKind::ShipLog).unwrap().id;

        // Sealed until investigated.
        let vis = h.visible_evidence(&h.chronicle(Importance::Minor, 10), &[]);
        assert!(vis.iter().find(|e| e.id == log_id).unwrap().text.contains("봉인"));
        let got = h.evidence_from_wreck(ev, false);
        h.record(200, DomainEvent::WreckInvestigated { investigator: "C".into(), event: ev, found: got.len() as u32 });
        let vis = h.visible_evidence(&h.chronicle(Importance::Minor, 10), &[]);
        assert!(!vis.iter().find(|e| e.id == log_id).unwrap().text.contains("봉인"));

        h.record(300, DomainEvent::ResearchPublished { author: "C".into(), event: ev, thesis: "B가 먼저 쏘았다".into(), cited: got.clone(), historian: true });
        let conf = h.event(ev).unwrap().research[0].confidence;
        assert!(conf > 0.5);
        h.record(400, DomainEvent::ResearchDisputed { disputer: "D".into(), author: "C".into(), event: ev });
        assert!(h.event(ev).unwrap().research[0].confidence < conf);
        let c = h.record(500, DomainEvent::ResearchDisputed { disputer: "E".into(), author: "C".into(), event: ev }).unwrap();
        assert_eq!(h.event(c).unwrap().kind, "CONTROVERSY");
        assert!(h.biography("C").len() >= 2);
        assert!(h.biography("A").iter().any(|b| b.text.contains("격침")));
        assert!(!h.herald(5).is_empty());
    }

    #[test]
    fn first_discovery_only_once() {
        let mut h = HistoryEngine::new();
        let d = |p: &str| DomainEvent::Discovery { player: p.into(), ship: "S".into(), system: 6, site: "고대 유적 Alpha".into(), kind: "Ruin".into() };
        assert!(h.record(1, d("A")).is_some());
        assert!(h.record(2, d("B")).is_none());
        assert_eq!(h.events.len(), 1);
    }

    #[test]
    fn bounty_chain_links_causes() {
        let mut h = HistoryEngine::new();
        let b = h.record(1, DomainEvent::BountyPlaced { target: "X".into(), amount: 6000, system: 5, reason: "무고한 함장 공격".into() }).unwrap();
        let mut d = destroyed("X", "H", true);
        if let DomainEvent::ShipDestroyed { victim_bounty, .. } = &mut d {
            *victim_bounty = 6000;
        }
        let k = h.record(2, d).unwrap();
        assert!(h.event(k).unwrap().causes.contains(&b));
        let c = h.record(3, DomainEvent::BountyClaimed { hunter: "H".into(), target: "X".into(), amount: 6000, system: 5 }).unwrap();
        assert!(h.event(c).unwrap().causes.contains(&b));
    }

    #[test]
    fn battle_links_destructions_and_ordinals() {
        let mut h = HistoryEngine::new();
        h.record(100, destroyed("A", "B", true));
        let side = |n: &str, m: &[&str], l| Side { name: n.into(), members: m.iter().map(|s| s.to_string()).collect(), losses: l };
        let b1 = h.record(110, DomainEvent::EngagementConcluded { system: 6, sides: vec![side("Fleet A", &["A", "A2"], 1), side("Fleet B", &["B"], 0)], winner: Some("Fleet B".into()), seconds: 30 }).unwrap();
        assert_eq!(h.event(b1).unwrap().causes.len(), 1);
        assert_eq!(h.event(b1).unwrap().importance, Importance::Major);
        let b2 = h.record(900, DomainEvent::EngagementConcluded { system: 6, sides: vec![side("x", &["A"], 0), side("y", &["B"], 0)], winner: None, seconds: 10 }).unwrap();
        assert!(h.event(b2).unwrap().title.starts_with("제2차"));
    }

    #[test]
    fn replay_rebuilds_identical_history() {
        let dir = std::env::temp_dir().join(format!("sfd-hist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("history.jsonl");
        let (n_events, n_evidence, bio) = {
            let mut h = HistoryEngine::open(&path).unwrap();
            h.record(1, DomainEvent::PlayerFounded { player: "A".into(), species: "Human".into(), ship: "Aurora".into(), hull: "Wren".into() });
            let ev = h.record(2, destroyed("A", "B", true)).unwrap();
            h.record(3, DomainEvent::WreckInvestigated { investigator: "C".into(), event: ev, found: 2 });
            (h.events.len(), h.evidence.len(), h.biography("A"))
        };
        let h2 = HistoryEngine::open(&path).unwrap();
        assert_eq!(h2.events.len(), n_events);
        assert_eq!(h2.evidence.len(), n_evidence);
        assert_eq!(h2.biography("A"), bio);
        assert!(h2.evidence.iter().any(|e| e.discovered_by.as_deref() == Some("C")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A forgery is secret until examined, survives a replay of the log, and once exposed
    /// discredits the research that cites it.
    #[test]
    fn forgery_is_secret_until_examined_and_replays() {
        let dir = std::env::temp_dir().join(format!("sfd-forge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.jsonl");
        let mut h = HistoryEngine::open(&path).unwrap();
        let ev = h.record(100, destroyed("A", "B", true)).unwrap();
        let log = h.evidence.iter().find(|e| e.event == ev && e.kind == EvidenceKind::ShipLog).unwrap().id;
        let n_events = h.events.len();
        assert!(h.record(110, DomainEvent::EvidenceForged { forger: "M".into(), original: log, text: "A가 먼저 쐈다".into() }).is_none());
        let fake = h.evidence.last().unwrap().id;
        assert_eq!(h.events.len(), n_events, "no chronicle entry for a secret act");
        assert!(h.biography("M").is_empty());
        h.record(120, DomainEvent::ResearchPublished { author: "M".into(), event: ev, thesis: "A의 도발".into(), cited: vec![fake], historian: false });
        // Replayed from the log, the forgery is still there and still secret.
        drop(h);
        let mut h = HistoryEngine::open(&path).unwrap();
        assert_eq!(h.forger_of(fake), Some("M"));
        assert_eq!(h.evidence.iter().find(|e| e.id == fake).unwrap().authenticity, Authenticity::Unverified);
        let exposed = h.record(130, DomainEvent::EvidenceAuthenticated { examiner: "C".into(), evidence: fake, discredit: 0.3 }).unwrap();
        assert_eq!(h.event(exposed).unwrap().kind, "FORGERY_EXPOSED");
        let r = &h.event(ev).unwrap().research[0];
        assert!(r.discredited);
        assert!(h.herald(5).iter().all(|n| !n.body.contains("A의 도발")), "news no longer quotes discredited research");
        assert!(h.record(140, DomainEvent::EvidenceAuthenticated { examiner: "C".into(), evidence: log, discredit: 0.3 }).is_none(), "the original is simply authentic");
        assert_eq!(h.evidence.iter().find(|e| e.id == log).unwrap().authenticity, Authenticity::Authentic);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Journalists' articles replace the automatic headline, survive a restart, and run as a
    /// correction once their evidence is exposed as forged.
    #[test]
    fn stories_lead_the_herald_and_replay() {
        let dir = std::env::temp_dir().join(format!("sfd-story-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.jsonl");
        let mut h = HistoryEngine::open(&path).unwrap();
        let ev = h.record(100, destroyed("A", "B", true)).unwrap();
        let log = h.evidence.iter().find(|e| e.event == ev).unwrap().id;
        assert!(h.herald(5).iter().any(|n| n.by.is_none()), "automatic headline before anyone writes it up");
        h.record(110, DomainEvent::StoryFiled { reporter: "Lois".into(), event: ev, headline: "현장 르포".into(), body: "A가 B를 격침했다.".into(), cited: vec![log] });
        drop(h);
        let mut h = HistoryEngine::open(&path).unwrap();
        let herald = h.herald(5);
        assert_eq!(herald[0].headline, "현장 르포");
        assert_eq!(herald[0].by.as_deref(), Some("Lois"));
        assert!(herald.iter().all(|n| n.by.is_some() || !n.headline.contains("또 하나의 함선")), "{herald:?}");
        assert!(h.stories[0].reliability > 0.9, "rests on the black box");
        h.record(120, DomainEvent::EvidenceForged { forger: "M".into(), original: log, text: "B가 먼저".into() });
        let fake = h.evidence.last().unwrap().id;
        h.record(130, DomainEvent::StoryFiled { reporter: "Clark".into(), event: ev, headline: "B 선제 발포".into(), body: "입수한 사본에 따르면…".into(), cited: vec![fake] });
        h.record(140, DomainEvent::EvidenceAuthenticated { examiner: "C".into(), evidence: fake, discredit: 0.3 });
        assert!(h.stories.iter().find(|s| s.reporter == "Clark").unwrap().retracted);
        assert!(!h.stories.iter().find(|s| s.reporter == "Lois").unwrap().retracted);
        assert!(h.herald(8).iter().any(|n| n.headline == "[정정] B 선제 발포"));
        assert!(h.biography("Clark").iter().any(|b| b.text.contains("정정")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Lost Artifact Hunt: unearthed → lost with the ship → recovered from the wreck, each entry
    /// caused by the one before, and the recovery makes the news.
    #[test]
    fn artifact_story_links_cause_to_effect() {
        let mut h = HistoryEngine::new();
        let art = |s: &str| s.to_string();
        let dug = h.apply(1000, DomainEvent::ArtifactUnearthed { artifact_id: 5, artifact: art("미감정 유물 (고대 유적 Alpha)"), culture: art("Veyr"), player: art("Indy"), ship: art("Seer"), system: 6, site: art("고대 유적 Alpha") }).unwrap();
        assert!(h.apply(1010, DomainEvent::ArtifactAppraised { artifact_id: 5, artifact: art("기억 껍질"), player: art("Indy"), value: 3000 }).is_none(), "appraisal is a biography line, not news");
        let wreck = h.apply(1020, destroyed("Indy", "Corsair", true)).unwrap();
        let lost = h.apply(1020, DomainEvent::ArtifactLost { artifact_id: 5, artifact: art("기억 껍질"), player: art("Indy"), system: 6, wreck_event: wreck }).unwrap();
        let back = h.apply(1100, DomainEvent::ArtifactRecovered { artifact_id: 5, artifact: art("기억 껍질"), player: art("Finder"), from: art("Indy"), system: 6, wreck_event: wreck }).unwrap();
        let ev = |id| h.event(id).unwrap();
        assert!(ev(lost).causes.contains(&wreck) && ev(lost).causes.contains(&dug));
        assert!(ev(back).causes.contains(&lost) && ev(back).causes.contains(&wreck));
        assert_eq!(ev(back).importance, Importance::Major);
        assert!(h.biography("Finder").iter().any(|b| b.text.contains("기억 껍질")));
        assert!(h.biography("Indy").iter().any(|b| b.text.contains("감정")));
        assert!(h.herald(5).iter().any(|n| n.headline.starts_with("돌아온 유물")));
    }
}
