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
        captains: vec![Captain { name: "Cap".into(), hull, species: Species::Human, profession: Profession::Explorer, level, system: 7, pos: Some([0.0, 0.0, 6000.0]), near_poi: None, near_deposit: None, offset: None, heading: 0.0, undocked: true, credits: 0, cargo: Default::default() }],
        npcs: vec![NpcGroup { archetype: arch, level: npc_level, system: 7, pos: [0.0, 0.0, 5450.0], count, spacing: 80.0 }],
        script: vec![Step { at: 0.2, captain: "Cap".into(), msg: ClientMsg::Controls(Controls { thrust: 0.2, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true, broadside: false, hold_over: false, fill_nearest: None, target_captain: None }],
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
    out += &galaxy_sheet();
    out += &mineral_sheet();
    out += &vertical_sheet();
    out += &relief_sheet();
    print!("{out}");
    eprintln!("{} duels simulated in {:.2} s", HullClass::ALL.len() * 3 * enc.len(), t0.elapsed().as_secs_f64());
    if let Some(path) = write {
        std::fs::write(&path, &out).expect("write balance sheet");
        eprintln!("wrote {path}");
    }
}

/// Main goal "3D freedom": how much fights use height. A hit counts as from above/below when the
/// shot came at least `HIGH_ANGLE` degrees off the system plane; a flat fight scores near 0%.
fn vertical_sheet() -> String {
    let mut out = String::from("\n## 3D 활용도\n\n");
    out += &format!(
        "목표: 평면이 아닌 3차원 전투. 명중 한 발마다 쏜 쪽이 표적보다 얼마나 위·아래에 있었는지(성계 평면 기준 고도각)를 잰다. 「위·아래 명중」 = |고도각| ≥ {:.0}°인 명중 비율, 「평균 고도각」 = |고도각| 평균. 함장 쪽(호위함 포함)이 준 명중만 센다.\n\n| 전투 | 함장 | 명중 | 위·아래 명중 | 평균 고도각 |\n|---|---|---|---|---|\n",
        sim::metrics::HIGH_ANGLE
    );
    let mut row = |label: &str, r: &sim::scenario::Report| {
        for (name, t) in &r.totals {
            if t.hits_dealt == 0 {
                continue;
            }
            out += &format!("| {label} | {name} | {} | {:.0}% | {:.1}° |\n", t.hits_dealt, t.high_angle_share().unwrap_or(0.0) * 100.0, t.mean_elev().unwrap_or(0.0));
        }
    };
    for (sys, label) in [(3u16, "해적 사냥 15분 (Frontier)"), (6, "해적 사냥 15분 (Wild)")] {
        let sc = Scenario {
            name: format!("vertical hunt {sys}"),
            description: String::new(),
            unix: 1_800_000_000,
            dev: false,
            ambient_pirates: true,
            captains: vec![Captain { name: "Hunter".into(), hull: HullClass::Talon, species: Species::Human, profession: Profession::CombatPilot, level: 5, system: sys, pos: None, near_poi: None, near_deposit: None, offset: None, heading: 0.0, undocked: true, credits: 0, cargo: Default::default() }],
            npcs: vec![],
            script: vec![Step { at: 0.2, captain: "Hunter".into(), msg: ClientMsg::Controls(Controls { thrust: 0.7, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true, broadside: false, hold_over: false, fill_nearest: None, target_captain: None }],
            run_secs: 900.0,
            expect: vec![],
            digest: None,
        };
        row(label, &run(&sc, None));
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    let load = |file: &str| -> Scenario { serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{file}.json"))).unwrap()).unwrap() };
    for file in ["capital_vs_raiders", "fighter_duel_gunship", "war_shifts_control", "wild_crime_bounty"] {
        row(&format!("시나리오 {file}"), &run(&load(file), None));
    }
    // PvP: the same attack from level and from straight above a capital ship (turret arcs, combat.ron).
    let r = run(&load("pvp_dive_halves_return_fire"), None);
    out += "
대인전 위·아래 공격(시나리오 pvp_dive_halves_return_fire): Lv9 Talon이 Lv9 Bastion 주력함을 10초 공격. 주력함은 공격자를 목표로 잡고 포탑으로만 반격한다(`turret_arc_overlap`). 「구름」= 주력함이 공격자를 선체 옆으로 오게 구른다(broadside), 「따라붙음」= 공격자가 주력함의 위 축을 따라 움직인다(hold_over).

| 공격 위치 | 위·아래 명중 | 공격자 선체 (10초) | 받은 피해 (호위함 포함) | 주력함 반격 DPS |
|---|---|---|---|---|
";
    for (label, who, admiral) in [("수평", "Level", "AdmiralLevel"), ("바로 위", "Diver", "AdmiralDive"), ("바로 위 · 방어자 구름", "DiverVsRoll", "AdmiralRoll"), ("바로 위 · 방어자 구름 · 공격자 따라붙음", "Shadow", "AdmiralDuel")] {
        let of = |n: &str| &r.totals.iter().find(|(name, _)| name == n).unwrap().1;
        let (t, a) = (of(who), of(admiral));
        let hull = r.final_hull.iter().find(|(n, _)| n == who).map(|(_, h)| *h).unwrap_or(0.0);
        out += &format!("| {label} | {:.0}% | {:.0}% | {:.0} | {:.0} |
", t.high_angle_share().unwrap_or(0.0) * 100.0, hull * 100.0, t.taken, a.dps());
    }
    out
}

/// Main goal "3D freedom", the map side: how steeply the routes from each station climb or dive to
/// the places captains fly to (`galaxy::route_elevations`, heights from `galaxy.ron` relief).
fn relief_sheet() -> String {
    use shared::galaxy::{self, RiskTier};
    let mut out = String::from("
## 성계 지형

목표: 일하러 가는 길이 평면이 아니다. 정거장에서 워프 지점·탐사 지점·소행성까지 경로마다 성계 평면 기준 고도각을 잰다. 「가파른 경로」 = |고도각| ≥ 20°.

| 위험도 | 성계 | 경로 | 평균 고도각 | 가파른 경로 | 위 / 아래 |
|---|---|---|---|---|---|
");
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        let sys: Vec<_> = galaxy::systems().iter().filter(|s| s.tier == tier).collect();
        let e: Vec<f32> = sys.iter().flat_map(|s| galaxy::route_elevations(&galaxy::system_layout(s.id))).collect();
        if e.is_empty() {
            continue;
        }
        let n = e.len() as f32;
        let (up, down) = (e.iter().filter(|a| **a >= 20.0).count(), e.iter().filter(|a| **a <= -20.0).count());
        out += &format!("| {:?} | {} | {} | {:.1}° | {:.0}% | {} / {} |
", tier, sys.len(), e.len(), e.iter().map(|a| a.abs()).sum::<f32>() / n, (up + down) as f32 / n * 100.0, up, down);
    }
    out
}

/// Phase 4-2: the mineral catalogue, what the galaxy's asteroids hold, and what specimens add to mining.
fn mineral_sheet() -> String {
    use shared::galaxy::{self, Ore, RiskTier};
    use shared::minerals::{self, Rarity};
    let t = shared::tuning::get();
    let m = &t.minerals;
    let mut out = String::from("\n## 광물\n\n");
    out += &format!("`tuning/minerals.ron`(seed {})의 광물 {}종. 광석 계열 × 등급마다 수가 정해져 있고 이름·속성 9개는 id로 정해진다. 덩어리 광석은 계열 시세로 팔리고, 희귀 이상 광맥은 캘 때마다 그 광물의 표본이 확률로 나온다.\n\n", m.seed, minerals::catalogue_size(m));
    out += "| 등급 | 계열당 종 수 | 속성 평균 | 표본 확률(단위당) | 표본 평균 시세 | 첫 발견 평판 |\n|---|---|---|---|---|---|\n";
    let avg_value = |r: Rarity| {
        let ids: Vec<u32> = Ore::ALL.iter().flat_map(|o| minerals::bucket(m, *o, r)).collect();
        ids.iter().map(|id| minerals::mineral_in(m, *id).unwrap().specimen_value(m)).sum::<u64>() as f64 / ids.len().max(1) as f64
    };
    for r in Rarity::ALL {
        let chance = m.specimen_chance.get(&r).copied().unwrap_or(0.0);
        out += &format!(
            "| {} | {} | {} | {} | {:.0} CR | {} |\n",
            r.label(),
            m.counts.get(&r).copied().unwrap_or(0),
            m.property_mean.get(&r).copied().unwrap_or(0),
            if chance > 0.0 { format!("{:.1}%", chance * 100.0) } else { "—".into() },
            avg_value(r),
            m.discovery_reputation.get(&r).map(|v| format!("+{v}")).unwrap_or_else(|| "—".into())
        );
    }
    out += "\n은하 전체 소행성의 광맥 등급(실제 배치)과, 무작위 소행성 1,000단위를 캘 때 표본으로 더 버는 기대값:\n\n| 위험도 | 소행성 | 일반 | 고급 | 희귀 | 이색 | 전설 | 1,000단위당 표본 기대 수입 |\n|---|---|---|---|---|---|---|---|\n";
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        let mut n = [0u32; 5];
        let mut expect = 0.0;
        for s in galaxy::systems().iter().filter(|s| s.tier == tier) {
            for a in galaxy::system_layout(s.id).asteroids {
                let min = minerals::mineral_in(m, a.mineral).unwrap();
                n[min.rarity as usize] += 1;
                expect += m.specimen_chance.get(&min.rarity).copied().unwrap_or(0.0) as f64 * min.specimen_value(m) as f64;
            }
        }
        let total: u32 = n.iter().sum();
        out += &format!("| {} | {total} | {} | {} | {} | {} | {} | {:.0} CR |\n", tier.label(), n[0], n[1], n[2], n[3], n[4], expect / total.max(1) as f64 * 1000.0);
    }
    let hb = m.height_bonus;
    out += &format!("\n높이 보너스(`height_bonus`): 정거장에서 {:.0}° 넘게 오르내리는 소행성은 희귀 이상 가중치가 최대 ×{:.1}({:.0}°에서). 평평한 소행성(< {:.0}°)과 가파른 소행성 비교:\n\n| 위험도 | 평평 / 가파름 | 희귀 이상 (평평) | 희귀 이상 (가파름) | 1,000단위당 표본 기대 수입 (평평 → 가파름) |\n|---|---|---|---|---|\n", hb.from_deg, 1.0 + hb.weight, hb.full_deg, hb.from_deg);
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        // (asteroids, rare and above, specimen income) for flat [0] and steep [1]
        let mut c = [(0u32, 0u32, 0.0f64); 2];
        for s in galaxy::systems().iter().filter(|s| s.tier == tier) {
            let l = galaxy::system_layout(s.id);
            let skip = 1 + l.pois.len();
            for (a, e) in l.asteroids.iter().zip(galaxy::route_elevations(&l).into_iter().skip(skip)) {
                let min = minerals::mineral_in(m, a.mineral).unwrap();
                let k = &mut c[(e.abs() >= hb.from_deg) as usize];
                k.0 += 1;
                k.1 += (min.rarity >= Rarity::Rare) as u32;
                k.2 += m.specimen_chance.get(&min.rarity).copied().unwrap_or(0.0) as f64 * min.specimen_value(m) as f64;
            }
        }
        let pct = |k: (u32, u32, f64)| k.1 as f64 / k.0.max(1) as f64 * 100.0;
        let inc = |k: (u32, u32, f64)| k.2 / k.0.max(1) as f64 * 1000.0;
        out += &format!("| {} | {} / {} | {:.0}% | {:.0}% | {:.0} → {:.0} CR |\n", tier.label(), c[0].0, c[1].0, pct(c[0]), pct(c[1]), inc(c[0]), inc(c[1]));
    }
    out
}

/// Phase 4-1: the procedural galaxy around the core, by tier, and how far a warp core gets you.
fn galaxy_sheet() -> String {
    use shared::galaxy::{self, PoiKind, RiskTier};
    let t = shared::tuning::get();
    let all = galaxy::systems();
    let mut out = String::from("
## 은하

");
    out += &format!(
        "수작업 핵심 성계 {}개에 `tuning/galaxy.ron`(seed {})으로 {}개를 더 생성한다. 생성 성계에 처음 도달한 함장은 최초 도달 사건과 등록 보상 {} CR × 위험도 배율을 받는다.

| 위험도 | 핵심 | 생성 | 유적·고대 게이트 | 최초 도달 보상 |
|---|---|---|---|---|
",
        galaxy::CORE.len(),
        t.galaxy.seed,
        all.len() - galaxy::CORE.len(),
        t.galaxy.chart_reward
    );
    for tier in [RiskTier::Core, RiskTier::Frontier, RiskTier::Wild, RiskTier::WarZone, RiskTier::Deep] {
        let of: Vec<_> = all.iter().filter(|s| s.tier == tier).collect();
        let core = of.iter().filter(|s| !galaxy::is_generated(s.id)).count();
        let ruins: usize = of.iter().map(|s| galaxy::system_layout(s.id).pois.iter().filter(|p| matches!(p.kind, PoiKind::Ruin | PoiKind::AncientGate)).count()).sum();
        let reward = if of.iter().any(|s| galaxy::is_generated(s.id)) { format!("{:.0} CR", t.galaxy.chart_reward as f32 * t.economy.risk_reward[&tier]) } else { "—".into() };
        out += &format!("| {} | {core} | {} | {ruins} | {reward} |
", tier.label(), of.len() - core);
    }
    out += "
워프 사거리별로 Sol Haven에서 (중간 성계를 거쳐) 닿는 성계 수:

| 워프 사거리 | 닿는 성계 | 못 닿는 성계 |
|---|---|---|
";
    for range in [1.0f32, 1.5, 2.0, 2.5] {
        let mut seen = vec![false; all.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(i) = stack.pop() {
            for j in 0..all.len() {
                if !seen[j] && galaxy::distance_ly(i as u16, j as u16) <= range {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        let missing: Vec<&str> = all.iter().filter(|s| !seen[s.id as usize]).map(|s| s.name).collect();
        let shown = if missing.len() > 4 { format!("{} 외 {}개", missing[..4].join(", "), missing.len() - 4) } else { missing.join(", ") };
        out += &format!("| {range:.1} ly | {} / {} | {shown} |
", seen.iter().filter(|s| **s).count(), all.len());
    }
    out
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
                captains: vec![Captain { name: "Hunter".into(), hull: HullClass::Talon, species: Species::Human, profession: Profession::CombatPilot, level, system: sys, pos: None, near_poi: None, near_deposit: None, offset: None, heading: 0.0, undocked: true, credits: 0, cargo: Default::default() }],
                npcs: vec![],
                script: vec![Step { at: 0.2, captain: "Hunter".into(), msg: ClientMsg::Controls(Controls { thrust: 0.7, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true, broadside: false, hold_over: false, fill_nearest: None, target_captain: None }],
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
        for s in galaxy::systems().iter().filter(|s| s.tier == tier) {
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
