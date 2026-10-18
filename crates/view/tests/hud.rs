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
