//! The HUD checked as text and geometry: run the egui drawing in a headless context (no window,
//! no GPU, no Bevy) and inspect the shapes it emits.

use egui::epaint::{ColorMode, Shape};
use egui::{Color32, Pos2, RawInput, Rect, Vec2};
use shared::protocol::{EntityKind, EntityState};
use shared::ships::{HullClass, Module, Ship};
use view::hud::{self, CROSSHAIR_R, STATUS_BAR_W};
use view::palette::LOSS;

const SCREEN: Vec2 = Vec2::new(1280.0, 720.0);

/// What one frame drew: every text with its screen rect, filled rects, and polylines with their colour.
#[derive(Default)]
struct Drawn {
    texts: Vec<(String, Rect)>,
    rects: Vec<(Rect, Color32)>,
    lines: Vec<(Vec<Pos2>, Color32)>,
}

impl Drawn {
    fn text(&self, needle: &str) -> Option<&(String, Rect)> {
        self.texts.iter().find(|(t, _)| t.contains(needle))
    }
    fn lines_of(&self, c: Color32) -> Vec<&Vec<Pos2>> {
        self.lines.iter().filter(|(_, col)| *col == c).map(|(p, _)| p).collect()
    }
}

fn collect(shape: &Shape, out: &mut Drawn) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        Shape::Text(t) => out.texts.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Rect(r) => out.rects.push((r.rect, r.fill)),
        Shape::Path(p) => {
            if let ColorMode::Solid(c) = p.stroke.color {
                out.lines.push((p.points.clone(), c));
            }
        }
        _ => {}
    }
}

/// Runs `draw` for a few frames (areas size themselves on the first) and returns the last frame's shapes.
fn frame(mut draw: impl FnMut(&egui::Context)) -> Drawn {
    let ctx = egui::Context::default();
    // No fade-ins: colours come out exactly as drawn.
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let mut out = Drawn::default();
    for i in 0..3 {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(i as f64 * 0.1), ..Default::default() };
        ctx.begin_pass(input);
        draw(&ctx);
        let full = ctx.end_pass();
        out = Drawn::default();
        for c in &full.shapes {
            collect(&c.shape, &mut out);
        }
    }
    out
}

fn me(shield: f32, hull: f32) -> EntityState {
    EntityState {
        id: 1,
        kind: EntityKind::Player,
        name: "Aegis".into(),
        hull_class: Some(HullClass::Bastion),
        rank: 0,
        livery: 0,
        pos: [0.0; 3],
        rot: [0.0, 0.0, 0.0, 1.0],
        vel: [30.0, 0.0, 40.0],
        hull,
        shield,
        radius: 16.0,
        fleet: None,
        wanted: false,
        boosting: false,
        mining: None,
        docked: false,
        owner: Some(7),
        alliance: None,
        class: None,
        species: None,
    }
}

#[test]
fn crosshair_gauges_show_values_and_the_lost_part() {
    let c = Pos2::new(SCREEN.x / 2.0, SCREEN.y / 2.0);
    let d = frame(|ctx| {
        let p = ctx.layer_painter(egui::LayerId::background());
        hud::crosshair_gauges(&p, c, 0.5, 0.8, [0.75, 0.8], 0.0);
    });
    let (_, shield) = d.text("실드\n50%").expect("shield readout");
    let (_, hull) = d.text("선체\n80%").expect("hull readout");
    assert!(shield.right() < c.x - CROSSHAIR_R && hull.left() > c.x + CROSSHAIR_R, "readouts sit outside the arcs: {shield:?} {hull:?}");
    assert!((shield.center().y - c.y).abs() < 2.0 && (hull.center().y - c.y).abs() < 2.0, "level with the crosshair");
    // Only the shield lost something (lag 0.75 > 0.5): one orange arc, on the left, on the gauge radius.
    let loss = d.lines_of(LOSS);
    assert_eq!(loss.len(), 1);
    assert!(loss[0].iter().all(|p| p.x < c.x && (p.distance(c) - CROSSHAIR_R).abs() < 0.01));
    // Shield arc spans a quarter of the gauge: from 50% up to 75% of its height range.
    let (y0, y1) = loss[0].iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
    assert!(y1 - y0 > 15.0 && y1 - y0 < 60.0, "{y0}..{y1}");
}

#[test]
fn low_hull_turns_red_and_full_gauges_show_no_loss() {
    let c = Pos2::new(640.0, 360.0);
    let d = frame(|ctx| {
        let p = ctx.layer_painter(egui::LayerId::background());
        hud::crosshair_gauges(&p, c, 1.0, 0.2, [1.0, 0.2], 0.0);
    });
    assert!(d.lines_of(LOSS).is_empty());
    let red = hud::hull_color(0.2, 0.55 + 0.45 * 0.5);
    assert_eq!(red.r(), 255);
    assert!(red.g() < 80, "pulsing red");
    assert!(!d.lines_of(red).is_empty(), "the hull arc is drawn in it");
}

#[test]
fn loss_bar_layers_background_loss_and_fill() {
    let d = frame(|ctx| {
        egui::Area::new("bar".into()).fixed_pos(Pos2::new(100.0, 100.0)).show(ctx, |ui| {
            hud::bar_with_loss(ui, 0.3, 0.6, Color32::BLUE, "x".into(), 200.0, 20.0);
        });
    });
    let width_of = |c: Color32| d.rects.iter().find(|(_, f)| *f == c).map(|(r, _)| r.width());
    assert_eq!(width_of(LOSS), Some(120.0));
    assert_eq!(width_of(Color32::BLUE), Some(60.0));
    assert!(d.rects.iter().any(|(r, _)| r.width() == 200.0), "background track");
}

#[test]
fn ship_status_panel_reads_the_real_numbers_and_fits() {
    let mut ship = Ship::new("Aegis".into(), HullClass::Bastion, 1);
    let fresh = ship.stats();
    let m = me(0.5, 0.25);
    let d = frame(|ctx| {
        egui::Area::new("ship".into()).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -36.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| hud::ship_status(ui, &ship, 12, Some(&m), [0.9, 0.25]));
        });
    });
    let shield = format!("에너지 실드 {:.0}/{:.0}", 0.5 * fresh.max_shield, fresh.max_shield);
    let hull = format!("선체 내구도 {:.0}/{:.0}", 0.25 * fresh.max_hull, fresh.max_hull);
    let (_, sr) = d.text(&shield).unwrap_or_else(|| panic!("missing {shield}: {:?}", d.texts));
    let (_, hr) = d.text(&hull).expect("hull bar label");
    assert!(sr.bottom() <= hr.top(), "shield bar above hull bar");
    let (_, line) = d.text("속도 50 u/s · 화물 12/").expect("speed and cargo line");
    assert!(line.top() > hr.bottom());
    assert!(d.text("Rank 3에서 순양함으로").is_some(), "a fresh ship shows its next growth step");
    // Everything stays on screen and the bars keep their width.
    for (t, r) in &d.texts {
        assert!(Rect::from_min_size(Pos2::ZERO, SCREEN).contains_rect(*r), "{t} off screen at {r:?}");
    }
    assert!(d.rects.iter().any(|(r, _)| (r.width() - STATUS_BAR_W).abs() < 0.01));
    // Low hull bar is red, and the lost shield trails in orange.
    assert!(d.rects.iter().any(|(_, f)| *f == view::palette::RED));
    assert!(d.rects.iter().any(|(r, f)| *f == LOSS && (r.width() - STATUS_BAR_W * 0.9).abs() < 0.01));

    // A capital ship drops the growth hint and lists its batteries.
    for mo in Module::ALL {
        ship.modules.insert(mo, 10);
    }
    let cap = ship.stats();
    let d = frame(|ctx| {
        egui::Area::new("ship".into()).show(ctx, |ui| hud::ship_status(ui, &ship, 0, None, [1.0, 1.0]));
    });
    let want = format!("주력함 · 전장 {:.0}m · 포탑 {}문 · 함수 주포 · 호위함 {}척", cap.radius * 2.2, cap.turrets, cap.escorts);
    assert!(d.text(&want).is_some(), "missing {want}: {:?}", d.texts);
    assert!(d.text("Rank").is_none());
    assert!(d.text("에너지 실드").is_none(), "no bars without a ship in space");
}

// ---- 3D cues: edge arrows and elevation

#[test]
fn offscreen_marker_points_the_way() {
    use shared::glam::Vec3;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0));
    let mid = screen.center();
    // On screen, ahead: no arrow.
    assert!(view::hud::offscreen_marker(screen, Vec3::new(0.0, 0.0, -100.0), Some(mid), 40.0).is_none());
    // Ahead but projected off the right edge: arrow on the right edge, pointing right.
    let (at, dir) = view::hud::offscreen_marker(screen, Vec3::new(500.0, 0.0, -100.0), Some(egui::pos2(2400.0, 450.0)), 40.0).unwrap();
    assert!((at.x - (1600.0 - 40.0)).abs() < 0.5 && dir.x > 0.99);
    // Straight above and behind: arrow at the top edge (up the screen is −y).
    let (at, dir) = view::hud::offscreen_marker(screen, Vec3::new(0.0, 300.0, 50.0), None, 40.0).unwrap();
    assert!((at.y - 40.0).abs() < 0.5 && dir.y < -0.99, "{at:?} {dir:?}");
    // Every arrow stays on the inset ring.
    for l in [Vec3::new(-3.0, -1.0, 2.0), Vec3::new(1.0, 5.0, -0.1), Vec3::new(0.0, 0.0, 9.0)] {
        let (at, _) = view::hud::offscreen_marker(screen, l, None, 40.0).unwrap();
        assert!(screen.shrink(39.0).contains(at), "{at:?}");
    }
}

#[test]
fn elevation_text_reads_above_and_below() {
    use shared::glam::Vec3;
    assert_eq!(view::hud::elevation_text(Vec3::ZERO, Vec3::new(1000.0, 1000.0, 0.0)), "▲ 1.00 km · +45°");
    assert_eq!(view::hud::elevation_text(Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 200.0, 300.0)), "▼ 300 m · -45°");
}

#[test]
fn target_panel_tells_how_many_turrets_reach_me() {
    // 3D combat: diving above or below a capital ship halves the batteries that can answer.
    assert_eq!(view::hud::battery_text(3, 0), None, "no turrets, no line");
    assert_eq!(view::hud::battery_text(8, 8).unwrap(), "포탑 8문 모두 나를 겨눔 — 위·아래로 파고들면 절반");
    assert_eq!(view::hud::battery_text(4, 8).unwrap(), "포탑 4/8문만 닿음 — 사격호 밖");
}

#[test]
fn a_turret_ship_is_told_to_roll() {
    // The defender's answer to a diving attacker (docs/3D_전투_설계.md): roll it level with the hull.
    assert_eq!(view::hud::roll_hint(8, 8), None, "every battery reaches: nothing to do");
    assert_eq!(view::hud::roll_hint(0, 0), None, "no turrets (fighter): rolling does not help");
    assert_eq!(view::hud::roll_hint(4, 8).unwrap(), "내 포탑 4/8문만 닿음\nQ/E로 굴러 선체 옆에 두면 8/8문", "two lines: the 320 px target panel would orphan the count");
}

#[test]
fn turret_ship_is_warned_about_captains_outside_its_fleet() {
    use shared::galaxy::RiskTier;
    use view::relation::Viewer;
    // Two captains on one pirate in open-PvP space: holding the trigger, my spare batteries would
    // shoot the other captain unless we fly as a fleet (docs/3D_전투_설계.md, 함대 필수).
    let at = |id, pos: [f32; 3]| EntityState { id, owner: Some(id as u64), name: format!("Cap{id}"), pos, ..me(1.0, 1.0) };
    let mine = at(1, [0.0; 3]);
    let pirate = EntityState { kind: EntityKind::Npc(shared::missions::NpcArchetype::Raider), owner: None, ..at(5, [0.0, 0.0, -800.0]) };
    let mate = at(2, [400.0, 0.0, 0.0]);
    let far = at(3, [5000.0, 0.0, 0.0]);
    let wild = Viewer { me: Some(1), player: Some(1), fleet: None, alliance: None, alliances: &[], tier: RiskTier::Wild };
    let warn = |v: &Viewer, turrets, ents: &[&EntityState]| hud::stray_fire_warning(v, &mine, &pirate, turrets, 2000.0, ents.iter().copied());
    assert_eq!(warn(&wild, 14, &[&mine, &pirate, &mate, &far]).unwrap(), "함대 아님: Cap2\n사격하면 내 포탑이 맞힐 수 있다 — K로 함대");
    assert_eq!(warn(&wild, 14, &[&mine, &pirate, &far]), None, "out of turret reach");
    assert_eq!(warn(&wild, 0, &[&mine, &pirate, &mate]), None, "no turrets: forward guns only hit what I aim at");
    let fleet = Viewer { fleet: Some(9), ..wild };
    assert_eq!(warn(&fleet, 14, &[&EntityState { fleet: Some(9), ..mate.clone() }]), None, "fleet mates are never picked");
    let core = Viewer { tier: RiskTier::Core, ..wild };
    assert_eq!(warn(&core, 14, &[&mate]), None, "no PvP here: turrets cannot hit captains");
    let both = at(4, [0.0, 300.0, 0.0]);
    assert!(warn(&wild, 14, &[&mate, &both]).unwrap().starts_with("함대 아님: Cap2 외 1척"));
}

// ---- bottom of the screen: chat, ship status, key hints

/// Like `frame`, at `size` and with the Korean font the client uses (text widths as in the game).
fn frame_ko(size: Vec2, mut draw: impl FnMut(&egui::Context)) -> Drawn {
    let ctx = egui::Context::default();
    ctx.set_fonts(view::fonts::with_korean().0);
    let mut out = Drawn::default();
    for i in 0..3 {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(i as f64 * 0.1), ..Default::default() };
        ctx.begin_pass(input);
        draw(&ctx);
        let full = ctx.end_pass();
        out = Drawn::default();
        for c in &full.shapes {
            collect(&c.shape, &mut out);
        }
    }
    out
}

/// The status panel as the client places it, and its bounding box.
fn status_panel(ctx: &egui::Context, ship: &Ship, m: &EntityState) -> Rect {
    egui::Area::new("ship".into()).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -36.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| hud::ship_status(ui, ship, 12, Some(m), [1.0, 1.0]));
    })
    .response
    .rect
}

#[test]
fn key_hints_stay_clear_of_the_chat_and_the_ship_panel() {
    let ship = Ship::new("Aegis".into(), HullClass::Bastion, 1);
    let m = me(1.0, 1.0);
    for size in [Vec2::new(1280.0, 720.0), Vec2::new(1600.0, 900.0), Vec2::new(1920.0, 1080.0)] {
        let chat = Rect::from_min_max(Pos2::new(hud::CHAT_MARGIN, size.y - 200.0), Pos2::new(hud::CHAT_MARGIN + hud::CHAT_W, size.y - hud::CHAT_MARGIN));
        let mut panel = Rect::NOTHING;
        let d = frame_ko(size, |ctx| {
            panel = status_panel(ctx, &ship, &m);
            hud::key_hints(ctx);
        });
        let hints: Vec<Rect> = hud::KEY_HINTS.iter().map(|l| d.text(l).unwrap_or_else(|| panic!("missing hint {l}")).1).collect();
        for r in &hints {
            assert!(Rect::from_min_size(Pos2::ZERO, size).contains_rect(*r), "hint off screen at {size:?}: {r:?}");
            assert!(!r.intersects(panel), "hint over the ship panel at {size:?}: {r:?} vs {panel:?}");
            assert!(!r.intersects(chat), "hint over the chat at {size:?}");
        }
        assert!(hints[0].bottom() <= hints[1].top(), "two lines, one under the other");
    }

    // The single 150-character line it replaces ran under the ship panel and into the chat.
    const OLD: &str = "W/S 추진 · A/D 선회 · R/F 상하 · Q/E 롤 · Z/Ctrl 상승·하강 · Alt+A/D 좌우 평행이동 · 우클릭 조종 · Shift 부스트 · Space 사격/주포 · T 목표 · X 미사일 · B 호위공격 · Y 호위복귀 · C 1/3인칭 · I 함교 화면 · G 도킹 · N 채굴 · V 스캔 · M 지도 · U 정거장 · J 미션 · O 계약 · H 연대기 · K 함대 · L 동맹 · F1";
    let size = Vec2::new(1280.0, 720.0);
    let chat = Rect::from_min_max(Pos2::new(hud::CHAT_MARGIN, size.y - 200.0), Pos2::new(hud::CHAT_MARGIN + hud::CHAT_W, size.y - hud::CHAT_MARGIN));
    let mut panel = Rect::NOTHING;
    let d = frame_ko(size, |ctx| {
        panel = status_panel(ctx, &ship, &m);
        egui::Area::new("keys".into()).anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -6.0)).show(ctx, |ui| {
            ui.label(egui::RichText::new(OLD).size(11.0));
        });
    });
    let old: Vec<Rect> = d.texts.iter().filter(|(t, _)| t.contains("추진") && t.contains("동맹")).map(|(_, r)| *r).collect();
    assert!(old.iter().any(|r| r.intersects(chat) || r.intersects(panel)), "the test can see the old overlap: {old:?} panel {panel:?}");
}

#[test]
fn the_help_window_lists_every_control() {
    let d = frame_ko(Vec2::new(1600.0, 900.0), |ctx| {
        egui::Window::new("도움말 (F1)").show(ctx, hud::controls);
    });
    let right = d.texts.iter().map(|(_, r)| r.right()).fold(0.0, f32::max);
    let left = d.texts.iter().map(|(_, r)| r.left()).fold(f32::MAX, f32::min);
    assert!(right - left <= hud::CONTROLS_W + 1.0, "the rows wrap within {} px (spans {})", hud::CONTROLS_W, right - left);
    for (group, keys) in hud::CONTROLS {
        assert!(d.text(group).is_some(), "group {group}");
        for (k, what) in *keys {
            assert!(d.text(k).is_some() && d.text(what).is_some(), "{k} {what}");
        }
    }
    // Every key the short hint names is explained in full there.
    for k in ["W/S", "A/D", "R/F", "Q/E", "Shift", "Space", "T", "X", "M", "F1"] {
        let spaced = k.replace('/', " / ");
        assert!(hud::CONTROLS.iter().flat_map(|(_, ks)| ks.iter()).any(|(key, _)| key.starts_with(&spaced) || *key == k), "{k} missing from the help");
    }
}
