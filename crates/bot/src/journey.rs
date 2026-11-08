//! Player journey QA: what a new player goes through from the landing screen to the end of the
//! campaign — connect, create a captain, be told what to do, watch the opening, play the chapters,
//! hear each chapter's closing call and be pointed at the next — checked stage by stage against the
//! real rules (`sim::Game`) and the client's own presentation logic (`view`), without a window.
//!
//! The screens follow the client's state machine (`client/src/state.rs`): Connecting → Login on
//! connect, Welcome without a profile → Create, Profile / EnterSystem → Playing. The chapters are
//! played by the campaign pilot (`sim::playthrough`), which flies like a player (no dev commands).
//!
//! `cargo run -p bot --release --example journey [-- --hours 10 --no-campaign]` (results in
//! docs/QA_JOURNEY.md).

use shared::galaxy::Species;
use shared::missions::scenarios;
use shared::protocol::{ClientMsg, PROTOCOL_VERSION, Profession, Profile, ServerMsg, TICK_HZ};
use shared::ships::HullClass;
use sim::game::Game;
use sim::store::Store;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Options {
    pub species: Species,
    pub profession: Profession,
    pub hull: HullClass,
    /// Play the chapters too (minutes of wall clock); false checks the first session's screens only.
    pub campaign: bool,
    pub max_hours: f32,
    pub unix: u64,
}

impl Default for Options {
    fn default() -> Self {
        Options { species: Species::Human, profession: Profession::CombatPilot, hull: HullClass::Talon, campaign: true, max_hours: 10.0, unix: 1_760_000_000 }
    }
}

/// One stage of the journey: did the player get what they need there.
#[derive(Clone, Debug)]
pub struct Stage {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub stages: Vec<Stage>,
    /// Game seconds from sending the captain's name to seeing their own ship in space.
    pub to_first_ship: f32,
    /// The opening cinematic, as this captain's species sees it.
    pub opening_secs: f32,
    pub opening_captions: usize,
    /// (chapter title, game minutes, deaths) per finished chapter.
    pub chapters: Vec<(String, f32, u32)>,
    pub campaign: Option<sim::playthrough::Report>,
}

impl Report {
    pub fn failed(&self) -> usize {
        self.stages.iter().filter(|s| !s.ok).count()
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for st in &self.stages {
            s += &format!("{} {} — {}\n", if st.ok { "PASS" } else { "FAIL" }, st.name, st.detail);
        }
        let mut line = format!("first session: name → own ship {:.1} s · opening {:.0} s ({} captions)", self.to_first_ship, self.opening_secs, self.opening_captions);
        for (t, m, d) in &self.chapters {
            line += &format!(" · {t} {m:.0} min ({d} deaths)");
        }
        s += &line;
        s += "\n";
        if let Some(c) = &self.campaign {
            s += &format!("campaign: {}/{} chapters in {:.0} min of game time{}\n", c.completed(), scenarios().len(), c.secs / 60.0, c.stuck.as_ref().map_or(String::new(), |w| format!(" — STUCK: {w}")));
        }
        s += &format!("{} stage(s) failed\n", self.failed());
        s
    }
}

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../client/assets/cinematics")
}

/// The screen the client shows after a server message (`client/src/state.rs::apply`), or None if
/// the message does not change it.
pub fn screen_after(m: &ServerMsg) -> Option<&'static str> {
    match m {
        ServerMsg::Welcome { profile: None, .. } => Some("Create"),
        ServerMsg::Welcome { profile: Some(_), .. } | ServerMsg::Profile(_) | ServerMsg::EnterSystem { .. } => Some("Playing"),
        _ => None,
    }
}

pub fn run(o: &Options) -> Report {
    let mut rep = Report::default();
    let stage = |rep: &mut Report, name: &str, ok: bool, detail: String| rep.stages.push(Stage { name: name.into(), ok, detail });
    let dt = 1.0 / TICK_HZ;
    let mut g = Game::new(Store::memory(), history::HistoryEngine::new(), o.unix);
    let conn = 1;
    let out = |g: &mut Game| -> Vec<ServerMsg> { g.out.drain(..).filter(|(c, _)| *c == conn).map(|(_, m)| m).collect() };

    // Landing: the name goes out, the server answers with a token and no captain yet → the creation screen.
    g.handle(conn, ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Newcomer".into(), token: None });
    let msgs = out(&mut g);
    let welcome = msgs.iter().find_map(|m| if let ServerMsg::Welcome { token, profile, .. } = m { Some((token.clone(), profile.is_none())) } else { None });
    let screen = msgs.iter().filter_map(screen_after).next_back();
    stage(&mut rep, "랜딩 → 접속", welcome.as_ref().is_some_and(|(t, fresh)| !t.is_empty() && *fresh) && screen == Some("Create"), format!("welcome {} · token {} · screen {:?}", welcome.is_some(), welcome.as_ref().is_some_and(|(t, _)| !t.is_empty()), screen.unwrap_or("-")));

    // Creation: the captain exists, the client switches to the game and the ship shows up.
    g.handle(conn, ClientMsg::CreateCharacter { species: o.species, profession: o.profession, hull: o.hull, ship_name: "SNS Newcomer".into(), livery: 0 });
    let mut msgs = out(&mut g);
    let mut ticks = 0u32;
    let me = msgs.iter().find_map(|m| if let ServerMsg::EnterSystem { you, .. } = m { Some(*you) } else { None });
    let seen = |msgs: &[ServerMsg]| msgs.iter().any(|m| matches!(m, ServerMsg::Snapshot { infos, .. } if infos.iter().any(|i| Some(i.id) == me)));
    while !seen(&msgs) && ticks < 10 * TICK_HZ as u32 {
        g.tick(dt, o.unix);
        msgs.extend(out(&mut g));
        ticks += 1;
    }
    rep.to_first_ship = ticks as f32 * dt;
    let profile: Option<Profile> = msgs.iter().rev().find_map(|m| if let ServerMsg::Profile(p) = m { Some(p.clone()) } else { None });
    let screen = msgs.iter().filter_map(screen_after).next_back();
    let detail = format!("profile {} · screen {:?} · own ship on screen after {:.1} s", profile.is_some(), screen.unwrap_or("-"), rep.to_first_ship);
    stage(&mut rep, "캐릭터 생성 → 첫 화면", profile.is_some() && screen == Some("Playing") && seen(&msgs), detail);
    let Some(profile) = profile else { return rep };

    // The first thing on screen: what to do now (the tracker).
    let goal = view::hud::next_goal(&profile);
    stage(&mut rep, "첫 안내", goal.as_ref().is_some_and(|g| g.contains(scenarios()[0].title)), goal.clone().unwrap_or_else(|| "nothing tells the captain what to do".into()));

    // The opening (played once before chapter 1 starts): sound, short enough, every line on screen long enough.
    match std::fs::read_to_string(assets().join("opening.ron")).map_err(|e| e.to_string()).and_then(|t| view::cinematic::Cinematic::parse(&t)) {
        Ok(cin) => {
            rep.opening_secs = cin.sequence(o.species).iter().map(|s| s.secs).sum();
            rep.opening_captions = view::cinematic::caption_cues(&cin, o.species, 0.0, rep.opening_secs + 1.0).len();
            let problems = view::cinematic::check(&cin);
            // A first session should get the player flying within a few minutes; the opening can be skipped.
            let ok = problems.is_empty() && rep.opening_secs > 0.0 && rep.opening_secs <= 180.0 && rep.opening_captions > 0;
            let detail = format!("{:.0} s, {} captions{}", rep.opening_secs, rep.opening_captions, if problems.is_empty() { String::new() } else { format!(" · {}", problems.join("; ")) });
            stage(&mut rep, "오프닝 영상", ok, detail);
        }
        Err(e) => stage(&mut rep, "오프닝 영상", false, format!("opening.ron: {e}")),
    }

    // Chapter 1 starts when the opening ends: the tracker shows its first step.
    g.handle(conn, ClientMsg::StartScenario { id: 0 });
    let started: Option<Profile> = out(&mut g).into_iter().rev().find_map(|m| if let ServerMsg::Profile(p) = m { Some(p) } else { None });
    let all = scenarios();
    let step = started.as_ref().and_then(|p| p.scenario.as_ref()).and_then(|sp| all[sp.id as usize].steps.get(sp.step)).map(|s| s.describe());
    stage(&mut rep, "1장 시작 → 첫 목표", step.as_ref().is_some_and(|s| !s.is_empty()) && started.as_ref().is_some_and(|p| view::hud::next_goal(p).is_none()), step.unwrap_or_else(|| "no step on the tracker".into()));

    // Every step of every chapter says what to do; every chapter ends with its call and points at the next.
    let comms = std::fs::read_to_string(assets().join("comms.ron")).map_err(|e| e.to_string()).and_then(|t| view::comms::Comms::parse(&t));
    let mut silent = vec![];
    let mut vague = vec![];
    for sc in scenarios() {
        if comms.as_ref().map_or(true, |c| c.lines(sc.id, o.species).is_empty()) && (sc.id as usize) + 1 < scenarios().len() {
            silent.push(sc.title);
        }
        for st in &sc.steps {
            let d = st.describe();
            if d.trim().chars().count() < 4 {
                vague.push(format!("{}: {d:?}", sc.title));
            }
        }
    }
    stage(&mut rep, "장 끝 통신", comms.is_ok() && silent.is_empty(), if silent.is_empty() { format!("{} chapters close with a call", scenarios().len() - 1) } else { format!("no call after: {}", silent.join(", ")) });
    // …and which keys do it (a new player knows the goal, not yet the controls).
    let first = all[0].steps.first().map(|s| view::hud::step_hint(s, true, &Default::default())).unwrap_or_default();
    let keyless: Vec<String> = all.iter().flat_map(|sc| sc.steps.iter()).map(|s| view::hud::step_hint(s, false, &Default::default())).filter(|h| !h.chars().any(|c| c.is_ascii_uppercase())).collect();
    stage(&mut rep, "단계 조작 힌트", keyless.is_empty() && first.starts_with("G 출항"), format!("first: {first}{}", if keyless.is_empty() { String::new() } else { format!(" · no keys: {}", keyless.join("; ")) }));
    stage(&mut rep, "단계 문구", vague.is_empty(), if vague.is_empty() { format!("{} steps, each says what to do", scenarios().iter().map(|s| s.steps.len()).sum::<usize>()) } else { vague.join("; ") });
    let mut done = profile.clone();
    let mut pointed = vec![];
    for k in 1..scenarios().len() as u32 {
        done.scenarios_completed = k;
        done.scenario = None;
        if !view::hud::next_goal(&done).is_some_and(|g| g.contains(scenarios()[k as usize].title)) {
            pointed.push(k);
        }
    }
    stage(&mut rep, "다음 장 안내", pointed.is_empty(), if pointed.is_empty() { "after each chapter the tracker names the next".into() } else { format!("no pointer after chapters {pointed:?}") });

    if o.campaign {
        let c = sim::playthrough::play(&sim::playthrough::Options { species: o.species, profession: o.profession, hull: o.hull, max_secs: o.max_hours * 3600.0, unix: o.unix, ..Default::default() });
        for ch in &c.chapters {
            if let Some(f) = ch.finished {
                rep.chapters.push((ch.title.clone(), (f - ch.started) / 60.0, ch.deaths));
            }
        }
        stage(&mut rep, "캠페인 완주", c.completed() == scenarios().len(), format!("{}/{} chapters{}", c.completed(), scenarios().len(), c.stuck.as_ref().map_or(String::new(), |w| format!(", stuck: {w}"))));
        rep.campaign = Some(c);
    }
    rep
}
