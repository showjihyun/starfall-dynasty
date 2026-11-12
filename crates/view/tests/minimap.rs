//! The minimap checked headless: heading up, landmarks pinned to the rim, a click opens the chart,
//! the wheel zooms, both shapes draw.

use egui::epaint::Shape;
use egui::{Pos2, RawInput, Rect, Vec2};
use shared::glam::Vec3;
use view::minimap::{self, Blip, MinimapEvent, MinimapView, Shape as MapShape};

fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Draws the minimap for a few frames with `input` events on the last; texts and the event.
fn frame(v: &MinimapView, shape: MapShape, events: Vec<egui::Event>) -> (Vec<(String, Rect)>, Option<MinimapEvent>) {
    let ctx = egui::Context::default();
    let (mut seen, mut got) = (vec![], None);
    for i in 0..3 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 600.0))), time: Some(i as f64 * 0.1), ..Default::default() };
        if i == 2 {
            input.events = events.clone();
        }
        ctx.begin_pass(input);
        let mut r = None;
        egui::Area::new("m".into()).fixed_pos(Pos2::new(10.0, 10.0)).show(&ctx, |ui| r = minimap::minimap(ui, v, shape, 240.0));
        let out = ctx.end_pass();
        got = got.or(r);
        let mut t = vec![];
        for c in &out.shapes {
            texts(&c.shape, &mut t);
        }
        seen = t;
    }
    (seen, got)
}

fn view<'a>(blips: &'a [Blip]) -> MinimapView<'a> {
    MinimapView { system_name: "Sol Haven", me: Vec3::ZERO, fwd: Vec3::NEG_Z, station: Vec3::new(0.0, 0.0, -1500.0), jump_point: Vec3::new(40_000.0, 0.0, 0.0), sites: &[], rare: &[], marks: &[], blips, zoom: minimap::DEFAULT_RANGE, map_key: "F5" }
}

#[test]
fn ahead_is_up_and_far_landmarks_sit_on_the_rim_with_their_distance() {
    // Heading up: whatever the ship's heading, a point straight ahead is drawn above the centre.
    for fwd in [Vec3::NEG_Z, Vec3::X, Vec3::new(-1.0, 0.0, 1.0).normalize()] {
        let o = minimap::project(Vec3::ZERO, fwd, fwd * 1000.0, 0.1);
        assert!(o.y < -99.0 && o.x.abs() < 1e-3, "{fwd:?} → {o:?}");
        let right = minimap::project(Vec3::ZERO, fwd, Vec3::Y.cross(fwd).normalize() * -1000.0, 0.1);
        assert!(right.x > 99.0, "the ship's right is to the right: {right:?}");
    }
    for shape in MapShape::ALL {
        let (seen, ev) = frame(&view(&[]), shape, vec![]);
        assert_eq!(ev, None);
        let t: Vec<&str> = seen.iter().map(|s| s.0.as_str()).collect();
        assert!(t.contains(&"Sol Haven") && t.contains(&"정거장"), "{shape:?}: {t:?}");
        assert!(t.iter().any(|s| s.starts_with("반경 6.0k u · 성도 F5")), "{t:?}");
        // The warp point, 40,000 u off, is pinned to the rim with how far it is.
        assert!(t.contains(&"40k u"), "{t:?}");
    }
}

#[test]
fn a_click_opens_the_chart_and_the_wheel_zooms() {
    let blips = [Blip { pos: Vec3::new(300.0, 900.0, -800.0), fwd: Vec3::X, color: egui::Color32::RED, wreck: false, big: false, target: true }];
    let v = view(&blips);
    let (seen, _) = frame(&v, MapShape::Circle, vec![]);
    assert!(seen.iter().any(|s| s.0 == "▲"), "a ship well above shows ▲");
    let c = Pos2::new(10.0 + 120.0, 10.0 + 18.0 + 120.0);
    let click = vec![
        egui::Event::PointerMoved(c),
        egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ];
    assert_eq!(frame(&v, MapShape::Square, click).1, Some(MinimapEvent::Open));
    let wheel = |y: f32| vec![egui::Event::PointerMoved(c), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: Vec2::new(0.0, y), phase: egui::TouchPhase::Move, modifiers: Default::default() }];
    assert_eq!(frame(&v, MapShape::Circle, wheel(1.0)).1, Some(MinimapEvent::Zoom(minimap::DEFAULT_RANGE - 1)), "wheel up: closer");
    assert_eq!(frame(&v, MapShape::Circle, wheel(-1.0)).1, Some(MinimapEvent::Zoom(minimap::DEFAULT_RANGE + 1)), "wheel down: wider");
}

#[test]
fn the_settings_keep_the_minimap_and_a_bigger_ui_by_default() {
    let s = view::settings::Settings::default();
    assert!(s.minimap && s.minimap_shape == MapShape::Circle);
    assert!(s.ui_scale >= 1.25, "the default UI reads bigger than 100%");
    let old = view::settings::Settings::parse("(minimap: false, minimap_shape: Square, minimap_size: 9999.0)");
    assert!(!old.minimap && old.minimap_shape == MapShape::Square && old.minimap_size == view::settings::MINIMAP_SIZE.1);
}
