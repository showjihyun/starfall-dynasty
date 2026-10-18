//! Window panels checked headless: what they show, and which request a click returns.

use egui::epaint::Shape;
use egui::{Pos2, RawInput, Rect, Vec2};
use shared::artifacts::{Artifact, Culture, Holder};
use shared::chronicle::BioEntry;
use shared::contracts::{Contract, ContractKind, ContractState};
use view::panels::{self, ArtifactPrices, ArtifactRequest, ContractForm, ContractRequest};

fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Draws a panel for a few frames; `click` (a text prefix) is clicked on the last frame.
/// Returns the texts of the last frame and the request the panel returned, if any.
fn frame<R>(mut draw: impl FnMut(&mut egui::Ui) -> Option<R>, click: Option<&str>) -> (Vec<String>, Option<R>) {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let (mut seen, mut got) = (vec![], None);
    let mut at: Option<Pos2> = None;
    for i in 0..4 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), time: Some(i as f64 * 0.1), ..Default::default() };
        // Press and release on the text found in the previous frame.
        if let Some(p) = at.take() {
            input.events = vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
            ];
        }
        ctx.begin_pass(input);
        let mut r = None;
        egui::Area::new("panel".into()).fixed_pos(Pos2::new(10.0, 10.0)).show(&ctx, |ui| r = draw(ui));
        let full = ctx.end_pass();
        if r.is_some() {
            got = r;
        }
        let mut t = vec![];
        for c in &full.shapes {
            texts(&c.shape, &mut t);
        }
        if i == 2
            && let Some(label) = click
        {
            at = t.iter().find(|(s, _)| s.starts_with(label)).map(|(_, r)| r.center());
        }
        seen = t.into_iter().map(|(s, _)| s).collect();
    }
    (seen, got)
}

// ---- artifacts

fn artifact(id: u64, appraised: bool) -> Artifact {
    Artifact {
        id,
        name: "Hollow Crown의 잃어버린 왕관".into(),
        culture: Culture::Human,
        origin_system: 6,
        origin_site: "고대 유적 Alpha".into(),
        value: 3000,
        appraised,
        holder: Holder::Player(1),
        provenance: vec![BioEntry { date: "3827.001".into(), text: "발굴되었다.".into(), event: None }],
    }
}

const PRICES: ArtifactPrices = ArtifactPrices { appraisal: 400, unappraised_sale: 0.4, donation_share: 0.25, donation_reputation: 10 };

fn art_frame(held: &[Artifact], docked: bool, click: Option<&str>) -> (Vec<String>, Option<ArtifactRequest>) {
    frame(|ui| panels::artifacts(ui, held, docked, &PRICES), click)
}

#[test]
fn unappraised_find_hides_its_name_and_value() {
    let (t, _) = art_frame(&[artifact(7, false)], true, None);
    assert!(t.iter().any(|s| s == "미감정 유물 (고대 유적 Alpha)"), "{t:?}");
    assert!(!t.iter().any(|s| s.contains("왕관") || s.contains("3000")), "name and value stay hidden: {t:?}");
    assert!(t.iter().any(|s| s == "감정 (400 CR)"));
    assert!(t.iter().any(|s| s == "판매 (미감정 40%)"));
}

#[test]
fn appraised_artifact_shows_name_value_and_prices() {
    let (t, _) = art_frame(&[artifact(7, true)], true, None);
    assert!(t.iter().any(|s| s == "Hollow Crown의 잃어버린 왕관"));
    assert!(t.iter().any(|s| s.contains("인류 유물 · 감정가 3000 CR")), "{t:?}");
    assert!(!t.iter().any(|s| s.starts_with("감정 (")), "no appraisal button once appraised");
    assert!(t.iter().any(|s| s == "판매 (3000 CR)"));
    assert!(t.iter().any(|s| s == "박물관 기증 (평판 +10, 사례금 750 CR)"));
    assert!(t.iter().any(|s| s == "내력 (1)"));
}

#[test]
fn artifact_buttons_return_requests_only_when_docked() {
    assert_eq!(art_frame(&[artifact(7, true)], true, Some("판매")).1, Some(ArtifactRequest::Sell(7)));
    assert_eq!(art_frame(&[artifact(7, false)], true, Some("감정 (")).1, Some(ArtifactRequest::Appraise(7)));
    assert_eq!(art_frame(&[artifact(9, true)], true, Some("박물관 기증")).1, Some(ArtifactRequest::Donate(9)));
    assert_eq!(art_frame(&[artifact(7, true)], false, Some("판매")).1, None, "disabled away from the station");
    let (t, _) = art_frame(&[], true, None);
    assert!(t.iter().any(|s| s.starts_with("유물 없음")));
}

// ---- contracts

fn contract(id: u64, employer: u64, kind: ContractKind, state: ContractState, contractor: Option<u64>) -> Contract {
    Contract { id, employer, employer_name: format!("P{employer}"), kind, reward: 1500, posted_unix: 0, expires_unix: 3600, state, contractor: contractor.map(|c| (c, format!("P{c}"))), signed_event: None }
}

fn hunt(target: &str) -> ContractKind {
    ContractKind::Hunt { target: target.into() }
}

#[test]
fn board_offers_the_right_action_per_contract() {
    let delivery = ContractKind::Delivery { ore: shared::galaxy::Ore::Xenite, units: 5, system: 1 };
    let list = vec![
        contract(1, 2, hunt("Mark"), ContractState::Open, None),
        contract(2, 1, hunt("Mark"), ContractState::Open, None),
        contract(3, 2, delivery, ContractState::Accepted, Some(1)),
        contract(4, 2, hunt("Mark"), ContractState::Breached { by: "P1".into() }, Some(1)),
    ];
    let board = |click| {
        let mut form = ContractForm::default();
        frame(|ui| panels::contracts(ui, &list, 1, 5000, &Default::default(), 0, &mut form), click)
    };
    let (t, _) = board(None);
    assert!(t.iter().any(|s| s == "#1 Mark의 함선 격침"), "{t:?}");
    assert!(t.iter().any(|s| s == "배신으로 파기 — P1"));
    assert!(t.iter().any(|s| s == "진행 중 — P1"));
    // Someone else's open contract: accept. Mine: withdraw. My running delivery: hand over.
    assert_eq!(t.iter().filter(|s| *s == "수락").count(), 1);
    assert_eq!(t.iter().filter(|s| *s == "철회").count(), 1);
    assert_eq!(t.iter().filter(|s| *s == "납품").count(), 1);
    assert_eq!(board(Some("수락")).1, Some(ContractRequest::Accept(1)));
    assert_eq!(board(Some("철회")).1, Some(ContractRequest::Cancel(2)));
    assert_eq!(board(Some("납품")).1, Some(ContractRequest::Deliver(3)));
}

#[test]
fn posting_needs_a_target_and_the_reward_in_hand() {
    let post = |form: ContractForm, credits| {
        let mut form = form;
        frame(|ui| panels::contracts(ui, &[], 1, credits, &Default::default(), 0, &mut form), Some("게시 (")).1
    };
    let with_target = ContractForm { target: "Mark".into(), reward: 2000, ..Default::default() };
    assert_eq!(post(with_target.clone(), 5000), Some(ContractRequest::Post { kind: hunt("Mark"), reward: 2000, hours: 24 }));
    assert_eq!(post(with_target, 1000), None, "cannot escrow what you lack");
    assert_eq!(post(ContractForm { reward: 500, ..Default::default() }, 5000), None, "a hunt needs a target");
}

#[test]
fn delivery_destinations_hide_uncharted_systems() {
    let core = shared::galaxy::CORE.len();
    let mut charted = std::collections::BTreeSet::new();
    let ids = |c: &std::collections::BTreeSet<u16>, here| panels::delivery_destinations(c, here).iter().map(|s| s.id as usize).collect::<Vec<_>>();
    assert_eq!(ids(&charted, 0), (0..core).collect::<Vec<_>>(), "only the core before anyone explores");
    charted.insert(core as u16 + 3);
    assert_eq!(ids(&charted, core as u16 + 7).len(), core + 2, "plus charted systems and the one you are in");
}

#[test]
fn specimens_list_and_sell() {
    let t = shared::tuning::get();
    let id = shared::minerals::bucket(&t.minerals, shared::galaxy::Ore::Xenite, shared::minerals::Rarity::Rare).start;
    let m = shared::minerals::mineral(id).unwrap();
    let held = std::collections::BTreeMap::from([(id, 3u32)]);
    let price = m.specimen_value(&t.minerals) * 3;
    let (seen, sold) = frame(|ui| panels::specimens(ui, &held, true).then_some(()), Some("표본 전량 판매"));
    assert!(seen.iter().any(|s| s == &format!("{} × 3", m.label())), "{seen:?}");
    assert!(seen.iter().any(|s| s == &format!("표본 전량 판매 (+{price} CR)")), "{seen:?}");
    assert_eq!(sold, Some(()));
    assert_eq!(frame(|ui| panels::specimens(ui, &held, false).then_some(()), Some("표본 전량 판매")).1, None, "docked only");
}

#[test]
fn prospectors_are_told_height_pays() {
    // The height bonus (minerals.ron) is only worth something if players can find it: the station,
    // the map legend and each asteroid's tooltip say so, with the real odds.
    use shared::galaxy::RiskTier;
    let empty = std::collections::BTreeMap::new();
    let (seen, _) = frame(|ui| panels::specimens(ui, &empty, true).then_some(()), None);
    assert!(seen.iter().any(|s| s.contains("위아래로 멀리 오르내린 소행성")), "{seen:?}");

    assert_eq!(view::mapview::deposit_legend(RiskTier::Core), None, "no rare deposits in Core: no ring to explain");
    let legend = view::mapview::deposit_legend(RiskTier::Wild).unwrap();
    assert!(legend.contains("20° 넘게 오르내린") && legend.contains("평지 18%"), "{legend}");

    let t = shared::tuning::get();
    let m = shared::minerals::mineral(shared::minerals::bucket(&t.minerals, shared::galaxy::Ore::Lumen, shared::minerals::Rarity::Common).start).unwrap();
    let flat = view::mapview::asteroid_tip(&m, RiskTier::Wild, -4.0);
    assert_eq!(flat, vec![m.label(), "정거장 기준 고도각 -4° · 희귀 이상 18%".to_string()]);
    let steep = view::mapview::asteroid_tip(&m, RiskTier::Wild, 40.0);
    assert!(steep[1].starts_with("정거장 기준 고도각 +40° · 희귀 이상 ") && steep[1].ends_with("(평지 18%)"), "{steep:?}");
    let core = view::mapview::asteroid_tip(&m, RiskTier::Core, 40.0);
    assert_eq!(core[1], "정거장 기준 고도각 +40°", "no odds where rare never occurs");
}

// ---- influence

#[test]
fn influence_table_lists_contested_systems_only() {
    use shared::protocol::SystemStanding;
    let systems = vec![
        SystemStanding { system: 0, controller: None, standings: vec![] },
        SystemStanding { system: 3, controller: Some((1, "FPA".into())), standings: vec![(1, "FPA".into(), 72.4), (2, "EMP".into(), 40.0)] },
        SystemStanding { system: 4, controller: None, standings: vec![(2, "EMP".into(), 12.0)] },
    ];
    let (t, _) = frame(|ui| -> Option<()> {
        panels::influence(ui, &systems, Some("FPA"), 60.0);
        None
    }, None);
    assert!(t.iter().any(|s| s == "Orion Drift") && t.iter().any(|s| s == "Vesta Corridor"), "{t:?}");
    assert!(!t.iter().any(|s| s == "Sol Haven"), "no influence, not listed");
    assert!(t.iter().any(|s| s == "지배 [FPA]"));
    assert!(t.iter().any(|s| s == "[FPA] 72 · [EMP] 40"));
    assert!(t.iter().any(|s| s == "무주지"));
    let (t, _) = frame(|ui| -> Option<()> {
        panels::influence(ui, &[], None, 60.0);
        None
    }, None);
    assert!(t.iter().any(|s| s.starts_with("아직 어느 성계에도")));
}

// ---- institutes

#[test]
fn institutes_show_exhibits_and_offer_donation_and_founding() {
    use shared::institutes::{Institute, InstituteView};
    use view::panels::InstituteRequest;
    let inst = |id: u64, founder: u64, system: u16, exhibits: Vec<Artifact>| InstituteView {
        institute: Institute { id, name: format!("기관{id}"), founder, founder_name: format!("P{founder}"), system, founded_unix: 0, exhibits: exhibits.iter().map(|a| a.id).collect(), visitors: 3 },
        exhibits,
    };
    let list = vec![inst(1, 2, 6, vec![artifact(9, true)]), inst(2, 3, 4, vec![])];
    let held = vec![artifact(7, false)];
    let draw = |click, docked, credits| {
        let mut name = "새 연구소".to_string();
        frame(|ui| panels::institutes(ui, &list, 6, &held, 1, credits, docked, 5000, &mut name), click)
    };
    let (t, _) = draw(None, true, 9000);
    assert!(t.iter().any(|s| s == "연구소·박물관 1곳"), "{t:?}");
    assert!(t.iter().any(|s| s == "설립 P2 · 전시 1점 · 관람 3회"));
    assert!(t.iter().any(|s| s == "Hollow Crown의 잃어버린 왕관 — 인류"), "exhibits with their names");
    assert!(t.iter().any(|s| s == "다른 성계의 기관 1곳"));
    assert_eq!(draw(Some("미감정 유물"), true, 9000).1, Some(InstituteRequest::Donate { artifact: 7, institute: 1 }));
    assert_eq!(draw(Some("설립 ("), true, 9000).1, Some(InstituteRequest::Found("새 연구소".into())));
    assert_eq!(draw(Some("설립 ("), true, 100).1, None, "cannot afford");
    assert_eq!(draw(Some("미감정 유물"), false, 9000).1, None, "docked only");
    // A founder sees no founding form.
    let mine = vec![inst(1, 1, 6, vec![])];
    let mut name = String::new();
    let (t, _) = frame(|ui| panels::institutes(ui, &mine, 6, &[], 1, 9000, true, 5000, &mut name), None);
    assert!(!t.iter().any(|s| s.starts_with("설립 (")));
}

#[test]
fn fleet_window_teaches_split_angles() {
    // Rolling beats a lone diver; a fleet that splits its angles beats rolling (pvp_two_angles_beat_roll).
    let (seen, _) = frame(|ui| {
        panels::fleet_tips(ui);
        None::<()>
    }, None);
    assert!(seen.iter().any(|s| s.contains("공격 각도를 나누면") && s.contains("구르기")), "{seen:?}");
    assert!(seen.iter().any(|s| s.contains("위·아래로 파고들어라")), "{seen:?}");
}
