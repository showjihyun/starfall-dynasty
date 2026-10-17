//! Automated play QA (docs/QA_PHASE2_3.md): two headless captains play the Phase 2–3 vertical slice
//! against a real `--dev` server over WebSockets — archaeology, a contract and its betrayal, the
//! destruction and the wreck, forgery and its exposure, the journalist's article and its readers,
//! alliance territory — and every checklist item is judged from what the server actually sent.
//!
//! `cargo run -p bot --release --example qa` (starts its own server) prints one line per item.

use crate::Bot;
use shared::artifacts::Artifact;
use shared::chronicle::{Authenticity, EvidenceKind, HistoricalEvent};
use shared::contracts::{ContractKind, ContractState};
use shared::galaxy::{Ore, Species};
use shared::glam::Vec3;
use shared::protocol::*;
use shared::ships::HullClass;

/// One checklist item: id (as in docs/QA_PHASE2_3.md), what it checks, pass, and what was seen.
#[derive(Clone, Debug)]
pub struct Item {
    pub id: &'static str,
    pub what: &'static str,
    pub ok: bool,
    pub seen: String,
}

#[derive(Default)]
pub struct Report {
    pub items: Vec<Item>,
    /// (captain, login token) — to look at the resulting world with `client --name N --token T`.
    pub tokens: Vec<(String, String)>,
}

impl Report {
    fn check(&mut self, id: &'static str, what: &'static str, ok: bool, seen: impl Into<String>) {
        self.items.push(Item { id, what, ok, seen: seen.into() });
    }

    pub fn passed(&self) -> usize {
        self.items.iter().filter(|i| i.ok).count()
    }

    pub fn ok(&self) -> bool {
        self.items.iter().all(|i| i.ok)
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for i in &self.items {
            s += &format!("{} {:<5} {} — {}\n", if i.ok { "PASS" } else { "FAIL" }, i.id, i.what, i.seen);
        }
        s += &format!("{} / {} passed\n", self.passed(), self.items.len());
        s
    }
}

fn artifacts(b: &Bot) -> Vec<Artifact> {
    b.log.iter().rev().find_map(|m| if let ServerMsg::Artifacts { held } = m { Some(held.clone()) } else { None }).unwrap_or_default()
}

fn toasts(b: &Bot) -> Vec<String> {
    b.log.iter().filter_map(|m| if let ServerMsg::Toast { text } = m { Some(text.clone()) } else { None }).collect()
}

fn credits(b: &Bot) -> u64 {
    b.profile.as_ref().map(|p| p.wallet.credits).unwrap_or(0)
}

async fn chat(b: &mut Bot, text: &str) {
    b.send(ClientMsg::Chat { text: text.into() }).await;
    b.pump(0.3).await;
}

async fn chronicle(b: &mut Bot) -> Option<(Vec<HistoricalEvent>, Vec<shared::chronicle::Evidence>, Vec<shared::chronicle::Headline>)> {
    b.send(ClientMsg::RequestChronicle).await;
    match b.until(5.0, |m| matches!(m, ServerMsg::Chronicle { .. })).await {
        Some(ServerMsg::Chronicle { events, evidence, herald }) => Some((events, evidence, herald)),
        _ => None,
    }
}

/// Dock at the station like a player would: if pirates keep the ship in combat, wait and retry.
async fn dock(b: &mut Bot) -> bool {
    for _ in 0..30 {
        chat(b, "/tp station").await;
        b.send(ClientMsg::Dock).await;
        if b.until(1.0, |m| matches!(m, ServerMsg::Toast { text } if text.starts_with("도킹 완료"))).await.is_some() {
            return true;
        }
        b.pump(1.0).await;
    }
    false
}

/// Scan a site of `kind` nearby and wait for the scan to finish.
async fn scan(b: &mut Bot, kind: &str) {
    chat(b, &format!("/tp poi:{kind}")).await;
    let me = b.my_state().map(|e| Vec3::from(e.pos)).unwrap_or_default();
    let layout = shared::galaxy::system_layout(b.system);
    if let Some(p) = layout.pois.iter().filter(|p| format!("{:?}", p.kind) == kind).min_by(|x, y| x.pos.distance(me).total_cmp(&y.pos.distance(me))) {
        b.send(ClientMsg::Scan { poi: p.id }).await;
    }
    b.until(12.0, |m| matches!(m, ServerMsg::Scanned { .. })).await;
    b.pump(0.5).await;
}

/// Play the checklist with two fresh captains named `a_name` (Archaeologist) and `b_name` (Journalist).
pub async fn run(url: &str, a_name: &str, b_name: &str) -> Result<Report, String> {
    let mut r = Report::default();
    let mut a = Bot::connect(url, a_name).await?;
    let mut b = Bot::connect(url, b_name).await?;

    // 1. Characters.
    a.login_and_create(HullClass::Seer, Species::Veyr, Profession::Archaeologist).await?;
    b.login_and_create(HullClass::Talon, Species::Human, Profession::Journalist).await?;
    let pa = a.profile.as_ref().map(|p| p.profession);
    let pb = b.profile.as_ref().map(|p| p.profession);
    r.check("1-2", "고고학자·기자 진수", pa == Some(Profession::Archaeologist) && pb == Some(Profession::Journalist), format!("{pa:?} / {pb:?}"));

    // 2. Archaeology in Hollow Crown.
    for bot in [&mut a, &mut b] {
        chat(bot, "/cash").await;
        chat(bot, "/grow 5").await;
    }
    r.check("2-1", "/cash·/grow 반영", credits(&a) > 100_000, format!("{} CR", credits(&a)));
    a.warp_to(6).await?;
    scan(&mut a, "Ruin").await;
    let held = artifacts(&a);
    r.check("2-3", "유적 스캔 → 유물 발굴", held.len() == 1, format!("{} 점: {:?}", held.len(), held.iter().map(|x| x.label()).collect::<Vec<_>>()));
    r.check("2-4", "발굴 직후 이름·가치 비공개", held.first().is_some_and(|x| !x.appraised && x.label().starts_with("미감정")), held.first().map(|x| x.label()).unwrap_or_default());
    scan(&mut a, "Ruin").await;
    let held = artifacts(&a);
    let docked = dock(&mut a).await;
    r.check("2-4b", "정거장 도킹(교전 중이면 대기 후 재시도)", docked, toasts(&a).iter().rev().take(2).cloned().collect::<Vec<_>>().join(" / "));
    if let Some(first) = held.first() {
        a.send(ClientMsg::AppraiseArtifact { id: first.id }).await;
        a.pump(0.5).await;
    }
    let after = artifacts(&a);
    let appraised = after.iter().find(|x| x.appraised);
    r.check("2-5", "감정(무료) → 이름·가치·내력", appraised.is_some_and(|x| x.value > 0 && x.provenance.len() >= 2), appraised.map(|x| format!("{} {} CR, 내력 {}줄", x.name, x.value, x.provenance.len())).unwrap_or_else(|| format!("{} 점, 감정된 것 없음 — 최근 알림 {:?}", after.len(), toasts(&a).iter().rev().take(3).collect::<Vec<_>>())));
    let rep0 = a.profile.as_ref().map(|p| p.reputation).unwrap_or(0);
    if let Some(x) = after.iter().find(|x| x.appraised) {
        a.send(ClientMsg::DonateArtifact { id: x.id }).await;
        a.pump(0.5).await;
    }
    let rep1 = a.profile.as_ref().map(|p| p.reputation).unwrap_or(0);
    let keep = artifacts(&a);
    r.check("2-6", "두 점 발굴 → 한 점 기증(평판 +10), 한 점 보유", held.len() == 2 && rep1 == rep0 + 10 && keep.len() == 1, format!("발굴 {}, 평판 {rep0}→{rep1}, 보유 {}", held.len(), keep.len()));

    // 3. Contract.
    b.warp_to(6).await?;
    let before = credits(&a);
    a.send(ClientMsg::PostContract { kind: ContractKind::Delivery { ore: Ore::Xenite, units: 10, system: 4 }, reward: 2000, hours: 24 }).await;
    a.pump(0.5).await;
    let escrowed = credits(&a);
    r.check("3-2", "계약 게시 → 보상 공탁(-2000)", escrowed + 2000 == before, format!("{before} → {escrowed}"));
    b.send(ClientMsg::RequestContracts).await;
    let board = match b.until(5.0, |m| matches!(m, ServerMsg::Contracts { .. })).await {
        Some(ServerMsg::Contracts { list }) => list,
        _ => vec![],
    };
    let cid = board.iter().find(|c| c.state == ContractState::Open && c.employer_name == a_name).map(|c| c.id);
    if let Some(id) = cid {
        b.send(ClientMsg::AcceptContract { id }).await;
    }
    let accepted = b.until(5.0, |m| matches!(m, ServerMsg::Contracts { list } if list.iter().any(|c| Some(c.id) == cid && c.state == ContractState::Accepted))).await.is_some();
    a.pump(0.3).await;
    r.check("3-3", "계약 수락 → 양측 체결 알림", accepted && toasts(&a).iter().any(|t| t.contains("수락했습니다")), format!("게시판 {}건, 수락 {accepted}", board.len()));

    // 4. Betrayal and destruction.
    a.send(ClientMsg::Undock).await;
    chat(&mut a, "/tp station").await;
    chat(&mut b, "/grow 9").await;
    chat(&mut b, "/tp station").await;
    a.pump(0.3).await;
    b.pump(0.5).await;
    let target = a.me;
    b.send(ClientMsg::SetTarget(target)).await;
    // (B's capital turrets open fire as soon as it arrives: compare with the wallet right after posting.)
    let mut died = false;
    for _ in 0..600 {
        let tp = target.and_then(|t| b.ents.get(&t)).map(|e| Vec3::from(e.pos));
        let mut c = tp.map(|p| b.aim_controls(p, 0.3, 250.0)).unwrap_or_default();
        c.fire = true;
        b.send(ClientMsg::Controls(c)).await;
        b.pump(0.05).await;
        a.pump(0.02).await;
        if a.dead {
            died = true;
            break;
        }
    }
    b.send(ClientMsg::Controls(Controls::default())).await;
    a.pump(0.5).await;
    b.pump(0.5).await;
    let ta = toasts(&a);
    let tb = toasts(&b);
    r.check("4-2", "계약 중 공격 → 배신, 보상은 피해자에게", tb.iter().any(|t| t.contains("계약이 깨졌습니다")) && ta.iter().any(|t| t.contains("계약을 깨고")) && credits(&a) >= escrowed + 2000, format!("A 지갑 공탁 후 {escrowed} → {}", credits(&a)));
    r.check("4-3", "격침 → 유물이 잔해와 함께 가라앉음", died && ta.iter().any(|t| t.contains("가라앉았습니다")) && artifacts(&a).is_empty(), format!("격침 {died}, A 유물 {}", artifacts(&a).len()));

    // 5. Wreck, forgery, reporting.
    chat(&mut b, "/tp wreck").await;
    b.pump(0.5).await;
    let wreck = b.ents.values().filter(|e| e.kind == EntityKind::Wreck).map(|e| e.id).next();
    if let Some(w) = wreck {
        b.send(ClientMsg::InvestigateWreck { wreck: w }).await;
    }
    b.pump(0.8).await;
    let got = artifacts(&b);
    r.check("5-1", "잔해 조사 → 증거·유물 회수", got.len() == 1 && toasts(&b).iter().any(|t| t.contains("유물 1점 회수")), format!("잔해 {wreck:?}, B 유물 {}", got.len()));
    let (events, evidence, _) = chronicle(&mut b).await.ok_or("no chronicle")?;
    let destroyed = events.iter().find(|e| e.kind == "SHIP_DESTROYED" && e.actors.first().is_some_and(|v| v == a_name)).map(|e| e.id);
    let archive = b.profile.as_ref().map(|p| p.archive.clone()).unwrap_or_default();
    let log = evidence.iter().find(|e| Some(e.event) == destroyed && e.kind == EvidenceKind::ShipLog && archive.contains(&e.id)).map(|e| e.id);
    if let Some(id) = log {
        b.send(ClientMsg::ForgeEvidence { evidence: id, text: "마지막 기록: 상대가 먼저 발포. 응사함.".into() }).await;
    }
    b.pump(0.5).await;
    let (_, evidence2, _) = chronicle(&mut b).await.ok_or("no chronicle")?;
    let fake = evidence2.iter().find(|e| Some(e.event) == destroyed && e.kind == EvidenceKind::ShipLog && Some(e.id) != log && e.text.contains("먼저 발포")).map(|e| e.id);
    r.check("5-2", "위조 사본 → 같은 출처의 증거가 하나 더", fake.is_some(), format!("원본 {log:?}, 사본 {fake:?}"));
    if let (Some(ev), Some(f)) = (destroyed, fake) {
        b.send(ClientMsg::FileStory { event: ev, headline: "Hollow Crown의 배신 — 블랙박스가 말하다".into(), body: "입수한 블랙박스에 따르면 먼저 쏜 쪽은 상대였다.".into(), cite: vec![f] }).await;
    }
    let (_, _, herald) = chronicle(&mut b).await.ok_or("no chronicle")?;
    r.check("5-3", "기사 송고 → Herald 맨 위, 바이라인", herald.first().is_some_and(|h| h.by.as_deref() == Some(b_name)), herald.first().map(|h| format!("{} — {:?}", h.headline, h.by)).unwrap_or_default());
    a.send(ClientMsg::Respawn).await;
    a.pump(0.5).await;
    let b_before = credits(&b);
    chronicle(&mut a).await;
    b.pump(0.5).await;
    r.check("5-4", "다른 함장이 읽음 → 기자 원고료", credits(&b) > b_before, format!("{b_before} → {}", credits(&b)));
    let b_rep = b.profile.as_ref().map(|p| p.reputation).unwrap_or(0);
    if let Some(f) = fake {
        a.send(ClientMsg::AuthenticateEvidence { evidence: f }).await;
    }
    a.pump(0.5).await;
    b.pump(0.5).await;
    let (events3, evidence3, herald3) = chronicle(&mut a).await.ok_or("no chronicle")?;
    let marked = evidence3.iter().find(|e| Some(e.id) == fake).map(|e| e.authenticity);
    r.check(
        "5-5",
        "감정 → 위조 판명, 폭로 사건, 기사 [정정], 기자 평판 하락",
        marked == Some(Authenticity::Forged) && events3.iter().any(|e| e.kind == "FORGERY_EXPOSED") && herald3.iter().any(|h| h.headline.starts_with("[정정]")) && b.profile.as_ref().is_some_and(|p| p.reputation < b_rep),
        format!("표시 {marked:?}, 정정 {}, 기자 평판 {b_rep}→{}", herald3.iter().any(|h| h.headline.starts_with("[정정]")), b.profile.as_ref().map(|p| p.reputation).unwrap_or(0)),
    );

    // 6. Alliance and territory.
    a.send(ClientMsg::AllianceCreate { name: "QA Pact".into(), tag: "QAP".into() }).await;
    a.pump(0.5).await;
    chat(&mut a, "/influence 100 3").await;
    chat(&mut a, "/influence 100 4").await;
    let standings = a.log.iter().rev().find_map(|m| if let ServerMsg::Influence { systems, .. } = m { Some(systems.clone()) } else { None }).unwrap_or_default();
    let held: Vec<u16> = standings.iter().filter(|s| s.controller.as_ref().is_some_and(|c| c.1 == "QAP")).map(|s| s.system).collect();
    let pts: Vec<f32> = standings.iter().filter(|s| held.contains(&s.system)).filter_map(|s| s.standings.first().map(|x| x.2)).collect();
    r.check("6-2", "영향력 → 두 성계 지배(100, 80)", held == vec![3, 4] && pts.iter().any(|p| (*p - 80.0).abs() < 0.5), format!("지배 {held:?}, 영향력 {pts:?}"));
    r.tokens = [(&a, a_name), (&b, b_name)].into_iter().filter_map(|(bot, n)| Some((n.to_string(), bot.token.clone()?))).collect();
    Ok(r)
}
