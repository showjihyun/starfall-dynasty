//! Headless balance sheet (docs/HARNESS.md): every hull at module levels 1/5/9 fights a set of NPC
//! encounters with scripted aim. Deterministic, so a rule or number change shows up as a text diff.
//!
//! `cargo run -p server --release --example balance [-- --write docs/BALANCE.md]`
//!
//! Cell: `W 12s 84%` = won after 12 s with 84% hull left · `L 31s` = flagship lost after 31 s ·
//! `- 70%` = nobody won within the time limit (hull left).
//! A second table gives the fight in numbers (`crate::metrics`): damage per second dealt and taken
//! over the fight, and the mean time-to-kill of the captain's kills.

use server::scenario::{Captain, NpcGroup, Scenario, Step, run};
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
        captains: vec![Captain { name: "Cap".into(), hull, species: Species::Human, profession: Profession::Explorer, level, system: 7, pos: Some([0.0, 0.0, 6000.0]), heading: 0.0, undocked: true, credits: 0 }],
        npcs: vec![NpcGroup { archetype: arch, level: npc_level, system: 7, pos: [0.0, 0.0, 5450.0], count, spacing: 80.0 }],
        script: vec![Step { at: 0.2, captain: "Cap".into(), msg: ClientMsg::Controls(Controls { thrust: 0.2, fire: true, ..Default::default() }), repeat_every: Some(0.1), until: None, target_nearest: Some("npc".into()), aim: true }],
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
    out += "`cargo run -p server --release --example balance -- --write docs/BALANCE.md`로 생성한다(결정적이므로 규칙이나 수치를 바꾸면 이 표의 diff로 영향이 보인다).\n\n";
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
    out += "같은 교전을 수치로 본 표다(`server::metrics`). `85/18 · 7.5s` = 교전 내내 초당 85 피해를 주고 18 피해를 받았으며, 함장이 격파한 적은 첫 피격부터 격침까지 평균 7.5초 걸렸다. 피해는 실드와 선체를 합친 값이고, 교전 시간은 승패가 난 시점(없으면 제한 시간)까지다.\n\n";
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
    print!("{out}");
    eprintln!("{} duels simulated in {:.2} s", HullClass::ALL.len() * 3 * enc.len(), t0.elapsed().as_secs_f64());
    if let Some(path) = write {
        std::fs::write(&path, &out).expect("write balance sheet");
        eprintln!("wrote {path}");
    }
}
