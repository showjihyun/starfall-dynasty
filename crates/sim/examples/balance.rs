//! Headless balance sheet (docs/HARNESS.md): every hull at module levels 1/5/9 fights a set of NPC
//! encounters with scripted aim. Deterministic, so a rule or number change shows up as a text diff.
//!
//! `cargo run -p sim --release --example balance [-- --write docs/BALANCE.md]`
//!
//! Cell: `W 12s 84%` = won after 12 s with 84% hull left · `L 31s` = flagship lost after 31 s ·
//! `- 70%` = nobody won within the time limit (hull left).
//! A second table gives the fight in numbers (`crate::metrics`): damage per second dealt and taken
//! over the fight, and the mean time-to-kill of the captain's kills.

use sim::scenario::{Captain, NpcGroup, Scenario, Step, run};
use shared::galaxy::Species;
use shared::missions::NpcArchetype;
use shared::protocol::{ClientMsg, Controls, Profession};
use shared::ships::HullClass;

const LIMIT: f32 = 90.0;

fn encounters() -> Vec<(&'static str, NpcArchetype, u8, u32)> {
    vec![
        ("Raider L1", NpcArchetype::Raider, 1, 1),
        ("Gunship L2", NpcArchetype::Gunship, 2, 1),
        ("Raider L3 x4", NpcArchetype::Raider, 3, 4),
        ("Warden L3", NpcArchetype::Warden, 3, 1),
        ("Warlord L4", NpcArchetype::Warlord, 4, 1),
    ]
}

/// (outcome cell, fight-numbers cell)
fn cell(hull: HullClass, level: u8, arch: NpcArchetype, npc_level: u8, count: u32) -> (String, String) {
    let sc = Scenario {
        name: format!("{hull:?} L{level} vs {arch:?}"),
        description: String::new(),
        unix: 1_800_000_000,
        dev: false,
        ambient_pirates: false,
        captains: vec![Captain { name: "Cap".into(), hull, species: Species::Human, profession: Profession::Explorer, level, system: 7, pos: Some([0.0, 0.0, 6000.0]), near_poi: None, offset: None, heading: 0.0, undocked: true, credits: 0, cargo: Default::default() }],
        npcs: vec![NpcGroup { archetype: arch, level: npc_level, system: 7, pos: [0.0, 0.0, 5450.0], count, spacing: 80.0 }],
        script: vec![Step { at: 0.2, captain: "Cap".into(), msg: ClientMsg::Controls(Controls { thrust: 0.2, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true, fill_nearest: None, target_captain: None }],
        run_secs: LIMIT,
        expect: vec![],
        digest: None,
    };
    let r = run(&sc, None);
    let hull_left = r.final_hull.first().map(|h| h.1).unwrap_or(0.0);
    let lost = r.final_hull_lost.first().map(|h| h.1).unwrap_or(0.0);
    let (outcome, end) = match (r.down_at.first(), r.npcs_cleared_at) {
        (Some((_, t)), cleared) if cleared.is_none_or(|c| c > *t) => (format!("L {t:.0}s"), *t),
        (_, Some(t)) => (format!("W {t:.0}s {:.1}% −{lost:.0}", hull_left * 100.0), t),
        _ => (format!("- {:.1}% −{lost:.0}", hull_left * 100.0), LIMIT),
    };
    // Rates over the whole fight (start to its end), so they compare across cells.
    let tot = r.totals.first().map(|t| t.1.clone()).unwrap_or_default();
    let secs = end.max(1.0 / shared::protocol::TICK_HZ);
    let ttk = tot.mean_ttk().map(|v| format!(" · {v:.1}s")).unwrap_or_default();
    (outcome, format!("{:.0}/{:.0}{ttk}", tot.dealt / secs, tot.taken / secs))
}

fn main() {
    let write = std::env::args().skip_while(|a| a != "--write").nth(1);
    let enc = encounters();
    let mut out = String::new();
    out += "# 밸런스 시트\n\n";
    out += "`cargo run -p sim --release --example balance -- --write docs/BALANCE.md`로 생성한다(결정적이므로 규칙이나 수치를 바꾸면 이 표의 diff로 영향이 보인다).\n\n";
    out += &format!(
        "각 칸은 함장 1명(모든 모듈 Lv)과 해당 NPC가 Vesta Front에서 550 m 거리로 시작해, 조준 사격하며 {LIMIT:.0}초까지 싸운 결과다. `W 12s 84.0% −96` = 12초에 승리, 선체 84.0% 남음(96포인트 손실, −0이면 피격 전에 격파) · `L 31s` = 31초에 격침 · `- 70.0%` = 시간 내 승부 없음.\n\n"
    );
    out += "| 함선 | Lv |";
    for (name, ..) in &enc {
        out += &format!(" {name} |");
    }
    out += "\n|---|---|";
    for _ in &enc {
        out += "---|";
    }
    out += "\n";
    let mut numbers = String::new();
    let t0 = std::time::Instant::now();
    eprintln!("tuning {}", shared::tuning::Tuning::locate().map(|d| d.display().to_string()).unwrap_or_default());
    for hull in HullClass::ALL {
        for level in [1u8, 5, 9] {
            out += &format!("| {hull:?} | {level} |");
            numbers += &format!("| {hull:?} | {level} |");
            for (_, arch, nl, n) in &enc {
                let (o, m) = cell(hull, level, *arch, *nl, *n);
                out += &format!(" {o} |");
                numbers += &format!(" {m} |");
            }
            out += "\n";
            numbers += "\n";
        }
    }
    out += "\n## 교전 수치\n\n";
    out += "같은 교전을 수치로 본 표다(`sim::metrics`). `85/18 · 7.5s` = 교전 내내 초당 85 피해를 주고 18 피해를 받았으며, 함장이 격파한 적은 첫 피격부터 격침까지 평균 7.5초 걸렸다. 피해는 실드와 선체를 합친 값이고, 교전 시간은 승패가 난 시점(없으면 제한 시간)까지다.\n\n";
    out += "| 함선 | Lv |";
    for (name, ..) in &enc {
        out += &format!(" {name} |");
    }
    out += "\n|---|---|";
    for _ in &enc {
        out += "---|";
    }
    out += "\n";
    out += &numbers;
    out += &territory();
    out += &social_economy();
    print!("{out}");
    eprintln!("{} duels simulated in {:.2} s", HullClass::ALL.len() * 3 * enc.len(), t0.elapsed().as_secs_f64());
    if let Some(path) = write {
        std::fs::write(&path, &out).expect("write balance sheet");
        eprintln!("wrote {path}");
    }
}

/// Territory economy (tuning/social.ron): what holding n systems costs and how hard the next one is.
fn territory() -> String {
    let g = sim::game::Game::new(sim::store::Store::memory(), history::HistoryEngine::new(), 1_800_000_000);
    let t = g.tuning.social;
    let mut out = String::new();
    out += "
## 영토 경제

";
    out += &format!(
        "동맹이 성계 n개를 지배할 때의 시간당 유지비와, 다음 성계를 얻는 난이도다. 새 성계의 영향력은 1/(1 + {} × n)배로만 쌓인다(과잉 확장). 유지비를 세금만으로 대려면 비동맹 함장들이 그 동맹 영토에서 시간당 거래·현상금으로 `유지비 ÷ 세율({:.0}%)`만큼 벌어야 한다. 금고가 유지비를 못 내면 가장 약한 성계부터 철수한다.

",
        t.overreach,
        t.tax_rate * 100.0
    );
    out += "| 지배 성계 n | 시간당 유지비 | 다음 성계 영향력 배율 | 다음 성계 장악에 필요한 원래 활동량 | 유지비를 세금으로 대는 데 필요한 비동맹 거래/시간 |
|---|---|---|---|---|
";
    for n in 1..=6usize {
        let upkeep = g.upkeep(n);
        let mult = 1.0 / (1.0 + t.overreach * n as f32);
        out += &format!("| {n} | {upkeep} CR | ×{mult:.2} | {:.0} | {} CR |
", t.control_threshold / mult, (upkeep as f32 / t.tax_rate).round());
    }
    out
}

/// Phase 2–3 economy (docs/HARNESS.md): what each way of earning pays, measured in the simulation
/// where the game decides it (hunting, mining) and from the rules where it is a formula.
fn social_economy() -> String {
    use shared::galaxy::{self, PoiKind, RiskTier};
    let t = shared::tuning::get();
    let mut out = String::new();
    out += "\n## 사회 경제\n\n";
    out += "Phase 2·3 수입원을 한 표로 모았다. 사냥·채굴은 시뮬레이션으로 측정한 값이고(아래 조건), 나머지는 `tuning/*.ron` 규칙으로 계산한 값이다. 비교 기준: 모듈 Lv4→5 업그레이드 한 번에 약 3,500 CR, Lv8→9에 약 11,000 CR.\n\n";

    // Hunting: 15 minutes in a pirate system with ambient spawning, flying at and shooting the nearest pirate.
    out += "### 해적 사냥 (15분 시뮬레이션)\n\n성계 기본 해적 출현(18초마다 보충)을 켠 채, 가장 가까운 해적에게 다가가며 조준 사격한다. 격침되면 그 시점까지로 계산한다.\n\n| 함선 | Lv | 성계 | 격파 | 현상금 합계 | 시간당 CR | 생존 |\n|---|---|---|---|---|---|---|\n";
    for (sys, label) in [(3u16, "Orion Drift (Frontier)"), (6, "Hollow Crown (Wild)"), (7, "Vesta Front (War Zone)")] {
        for level in [1u8, 5, 9] {
            let sc = Scenario {
                name: format!("hunt {sys} {level}"),
                description: String::new(),
                unix: 1_800_000_000,
                dev: false,
                ambient_pirates: true,
                captains: vec![Captain { name: "Hunter".into(), hull: HullClass::Talon, species: Species::Human, profession: Profession::CombatPilot, level, system: sys, pos: None, near_poi: None, offset: None, heading: 0.0, undocked: true, credits: 0, cargo: Default::default() }],
                npcs: vec![],
                script: vec![Step { at: 0.2, captain: "Hunter".into(), msg: ClientMsg::Controls(Controls { thrust: 0.7, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true, fill_nearest: None, target_captain: None }],
                run_secs: 900.0,
                expect: vec![],
                digest: None,
            };
            let r = run(&sc, None);
            let tot = r.totals.first().map(|x| x.1.clone()).unwrap_or_default();
            let alive_for = r.down_at.first().map(|d| d.1).unwrap_or(900.0).max(1.0);
            out += &format!(
                "| Talon | {level} | {label} | {} | {} CR | {:.0} | {} |\n",
                tot.kills,
                tot.credits_earned,
                tot.credits_earned as f32 / alive_for * 3600.0,
                r.down_at.first().map(|d| format!("{:.0}초에 격침", d.1)).unwrap_or_else(|| "끝까지".into())
            );
        }
    }

    // Mining: fill the hold at the asteroid nearest the station, fly back, sell.
    out += "\n### 채굴·판매 (시뮬레이션)\n\n광부(+40%)가 정거장에서 가장 가까운 소행성에서 화물칸을 채운 뒤 왕복해 판다. 시간당 CR = 판매액 ÷ (채굴 시간 + 왕복 비행 시간).\n\n| 함선 | Lv | 성계 | 광석 | 화물 | 채굴 시간 | 판매액 | 시간당 CR |\n|---|---|---|---|---|---|---|---|\n";
    for (sys, label) in [(0u16, "Sol Haven (Core)"), (3, "Orion Drift (Frontier)"), (6, "Hollow Crown (Wild)")] {
        for level in [1u8, 5] {
            let (ore, units, secs, value, cycle) = mining(sys, level);
            out += &format!("| Mule | {level} | {label} | {ore} | {units} | {secs:.0}초 | {value} CR | {:.0} |\n", value as f32 / cycle * 3600.0);
        }
    }

    // Archaeology: every ruin and gate, 60 days of finds, appraised value.
    out += "\n### 고고학 (60일 표본)\n\n유적·고대 게이트마다 하루 한 점. 평균 감정가(미감정 판매는 그 40%), 성계 위험도 배율 반영. 「하루 최대」는 그 등급 성계의 모든 유적을 하루에 도는 경우.\n\n| 위험도 | 유적·게이트 수 | 한 점 평균 감정가 | 하루 최대 |\n|---|---|---|---|\n";
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        let mut values = vec![];
        let mut sites = 0;
        for s in galaxy::SYSTEMS.iter().filter(|s| s.tier == tier) {
            for p in galaxy::system_layout(s.id).pois.iter().filter(|p| matches!(p.kind, PoiKind::Ruin | PoiKind::AncientGate)) {
                sites += 1;
                for day in 0..60 {
                    values.push(shared::artifacts::excavate(1, s.id, &p.name, p.kind, p.id, day, 1, 0).value);
                }
            }
        }
        if sites == 0 {
            continue;
        }
        let avg = values.iter().sum::<u64>() as f64 / values.len() as f64;
        out += &format!("| {} | {sites} | {avg:.0} CR | {:.0} CR |\n", tier.label(), avg * sites as f64);
    }

    // Rule-based sources.
    let h = &t.history;
    let so = &t.social;
    let a = &t.artifacts;
    out += "\n### 규칙으로 정해지는 수입·비용\n\n| 활동 | 수입 | 비용·위험 |\n|---|---|---|\n";
    out += &format!(
        "| 기자: 기사 한 편 | 송고 {} + 특종 {} + 독자 {}명 × {} = 최대 {} CR | 근거가 위조로 드러나면 평판 {} |\n",
        h.story_fee,
        h.scoop_bonus,
        h.story_reader_cap,
        h.story_per_reader,
        h.story_fee + h.scoop_bonus + h.story_reader_cap as u64 * h.story_per_reader,
        h.retraction_reputation
    );
    for n in [1u64, 3, 5] {
        let per = so.visit_base + so.visit_per_exhibit * n;
        out += &format!("| 기관(전시 {n}점): 관람 1회 | {per} CR | 설립 {} CR → 손익분기 관람 {}회 |\n", so.institute_cost, so.institute_cost.div_ceil(per));
    }
    out += &format!("| 유물 기증(정거장 박물관) | 감정가의 {:.0}% + 평판 {} | 유물을 판매(감정가 100%)하지 않는 기회비용 |\n", a.donation_share * 100.0, a.donation_reputation);
    out += &format!("| 위조 감정(폭로 성공) | {} CR + 평판 {} | 감정비 {} CR(역사학자 무료) |\n", h.examiner_reward, h.examiner_reputation, h.authenticate_cost);
    out += &format!("| 위조 | (연구·기사 신뢰) | {} CR, 폭로 시 평판 {} |\n", h.forge_cost, h.forger_reputation);
    out += &format!("| 계약 | 의뢰 보상(이전), 완수 평판 +{} | 배신 시 평판 {} |\n", so.fulfil_reputation, so.betrayal_reputation);
    out
}

/// One mining cycle: (ore, units, seconds to fill, sale value, cycle seconds incl. the round trip).
fn mining(sys: u16, level: u8) -> (String, u32, f32, u64, f32) {
    use shared::ships::Module;
    let mut g = sim::game::Game::new(sim::store::Store::memory(), history::HistoryEngine::new(), 1_800_000_000);
    g.handle(1, ClientMsg::Hello { version: shared::protocol::PROTOCOL_VERSION, name: "Miner".into(), token: None });
    g.handle(1, ClientMsg::CreateCharacter { species: Species::Human, profession: Profession::Miner, hull: HullClass::Mule, ship_name: "Digger".into(), livery: 0 });
    let pid = g.sessions[&1].player.unwrap();
    for m in Module::ALL {
        g.store.profile_mut(pid).unwrap().ship.modules.insert(m, level);
    }
    for s in g.systems.values_mut() {
        s.pirate_cd = f32::INFINITY;
    }
    g.spawn_player(pid, sys, false);
    let layout = shared::galaxy::system_layout(sys);
    let station = layout.station;
    let a = layout.asteroids.iter().min_by(|x, y| x.pos.distance(station).total_cmp(&y.pos.distance(station))).unwrap().clone();
    let (s, e) = g.player_entity(pid).unwrap();
    let ent = g.ent_mut(s, e).unwrap();
    ent.pos = a.pos + shared::glam::Vec3::new(0.0, 0.0, a.radius + 60.0);
    let (speed, cap) = (ent.stats.speed, ent.stats.cargo as u32);
    g.handle(1, ClientMsg::Mine { asteroid: Some(a.id) });
    let mut ticks = 0u32;
    while g.store.profile(pid).unwrap().cargo_used() < cap && ticks < 20 * 3600 {
        g.tick(0.05, 1_800_000_000);
        ticks += 1;
    }
    let units = g.store.profile(pid).unwrap().cargo_used();
    let secs = ticks as f32 / 20.0;
    g.spawn_player(pid, sys, true);
    let before = g.store.profile(pid).unwrap().wallet.credits;
    g.handle(1, ClientMsg::Sell { ore: a.ore, units });
    let value = g.store.profile(pid).unwrap().wallet.credits - before;
    let trip = 2.0 * a.pos.distance(station) / speed.max(1.0);
    (a.ore.name().to_string(), units, secs, value, secs + trip)
}
