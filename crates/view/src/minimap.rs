//! The minimap: the system around the ship, bottom left, heading up — the station and the warp point
//! always on it (pinned to the rim when out of range), sites, rare rocks, every ship by relation with
//! its heading and whether it is above or below, the target ringed. Round or square, its size and
//! whether it shows at all are the player's (settings; M toggles it). The mouse wheel zooms it, a
//! click opens the full star map. Drawn from plain data and checked headless (`tests/minimap.rs`).

use crate::palette::{ACCENT, DIM, GOLD, GREEN, RED, alpha, map as pm};
use egui::{Align2, Color32, FontId, Rect, Sense, Shape as EShape, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};
use shared::galaxy::coords;
use shared::glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Circle,
    Square,
}

impl Shape {
    pub const ALL: [Shape; 2] = [Shape::Circle, Shape::Square];

    pub fn name(self) -> &'static str {
        match self {
            Shape::Circle => "원형",
            Shape::Square => "사각형",
        }
    }
}

/// Zoom steps: radius shown, in world units (metres).
pub const RANGES: [f32; 6] = [1500.0, 3000.0, 6000.0, 12_000.0, 24_000.0, 48_000.0];
pub const DEFAULT_RANGE: usize = 2;

/// A ship (or wreck) on the minimap.
#[derive(Clone, Debug)]
pub struct Blip {
    pub pos: Vec3,
    pub fwd: Vec3,
    pub color: Color32,
    pub wreck: bool,
    /// Capital-sized: drawn larger.
    pub big: bool,
    pub target: bool,
}

/// What the minimap shows.
pub struct MinimapView<'a> {
    pub system_name: &'a str,
    pub me: Vec3,
    /// My nose: the map turns so it points up.
    pub fwd: Vec3,
    pub station: Vec3,
    pub jump_point: Vec3,
    /// Sites found: position, scanned already.
    pub sites: &'a [(Vec3, bool)],
    /// Rocks worth a look (rare and up): position, colour of their rarity.
    pub rare: &'a [(Vec3, Color32)],
    /// Places a held job sends the captain to in this system (a wreck to salvage, …).
    pub marks: &'a [Vec3],
    pub blips: &'a [Blip],
    /// Index into `RANGES`.
    pub zoom: usize,
    /// The key that opens the full star map.
    pub map_key: &'a str,
}

/// What the player did with the minimap this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MinimapEvent {
    /// Clicked: open the full star map.
    Open,
    /// Wheel: the new zoom index.
    Zoom(usize),
}

/// Heading-up projection of a world point onto the map, in points from the centre.
pub fn project(me: Vec3, fwd: Vec3, p: Vec3, scale: f32) -> Vec2 {
    let f = Vec2::new(fwd.x, fwd.z);
    let f = if f.length() > 1e-3 { f.normalized() } else { Vec2::new(0.0, -1.0) };
    let right = Vec2::new(-f.y, f.x);
    let d = Vec2::new(p.x - me.x, p.z - me.z);
    Vec2::new(d.dot(right), -d.dot(f)) * scale
}

/// Keep `v` inside the map (radius `r`): the point itself, or where it leaves the map and true.
fn pin(v: Vec2, r: f32, shape: Shape) -> (Vec2, bool) {
    match shape {
        Shape::Circle => {
            if v.length() <= r { (v, false) } else { (v.normalized() * r, true) }
        }
        Shape::Square => {
            let m = v.x.abs().max(v.y.abs());
            if m <= r { (v, false) } else { (v * (r / m), true) }
        }
    }
}

/// The minimap, `size` points across, where the caller placed `ui`. Returns what the player did.
pub fn minimap(ui: &mut egui::Ui, v: &MinimapView, shape: Shape, size: f32) -> Option<MinimapEvent> {
    let head = 18.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(size, size + head), Sense::click());
    let p = ui.painter_at(rect.expand(2.0));
    let area = Rect::from_min_size(rect.min + Vec2::new(0.0, head), Vec2::splat(size));
    let c = area.center();
    let r = size / 2.0 - 2.0;
    let range = RANGES[v.zoom.min(RANGES.len() - 1)];
    let scale = r / range;

    // Ground, rim, range rings.
    match shape {
        Shape::Circle => {
            p.circle_filled(c, r, alpha(pm::BG, 225));
            p.circle_stroke(c, r * 0.5, Stroke::new(1.0, pm::GRID));
            p.circle_stroke(c, r, Stroke::new(1.5, pm::TOOLTIP_BORDER));
        }
        Shape::Square => {
            let sq = Rect::from_center_size(c, Vec2::splat(r * 2.0));
            p.rect_filled(sq, 6.0, alpha(pm::BG, 225));
            p.circle_stroke(c, r * 0.5, Stroke::new(1.0, pm::GRID));
            p.circle_stroke(c, r, Stroke::new(1.0, pm::GRID));
            p.rect_stroke(sq, 6.0, Stroke::new(1.5, pm::TOOLTIP_BORDER), StrokeKind::Middle);
        }
    }
    p.line_segment([c - Vec2::new(r, 0.0), c + Vec2::new(r, 0.0)], Stroke::new(1.0, alpha(pm::GRID, 140)));
    p.line_segment([c - Vec2::new(0.0, r), c + Vec2::new(0.0, r)], Stroke::new(1.0, alpha(pm::GRID, 140)));
    let to = |w: Vec3| project(v.me, v.fwd, w, scale);
    let inside = |w: Vec3| pin(to(w), r - 4.0, shape);

    // Rare rocks and found sites (only in range).
    for (w, col) in v.rare {
        let (o, out) = inside(*w);
        if !out {
            p.circle_stroke(c + o, 3.0, Stroke::new(1.2, *col));
        }
    }
    for (w, scanned) in v.sites {
        let (o, out) = inside(*w);
        if !out {
            p.text(c + o, Align2::CENTER_CENTER, "◇", FontId::proportional(12.0), if *scanned { DIM } else { crate::palette::POI });
        }
    }
    // The station and the warp point: always there, on the rim when far.
    let landmark = |w: Vec3, col: Color32, label: &str, square: bool| {
        let (o, out) = inside(w);
        let at = c + o;
        if square {
            p.rect_stroke(Rect::from_center_size(at, Vec2::splat(9.0)), 1.5, Stroke::new(2.0, col), StrokeKind::Middle);
        } else {
            p.circle_stroke(at, 5.0, Stroke::new(2.0, col));
        }
        if out {
            // Far: an arrow pointing out, and the distance.
            let dir = o.normalized();
            p.add(EShape::convex_polygon(vec![at + dir * 9.0, at + dir * 3.0 + Vec2::new(-dir.y, dir.x) * 4.0, at + dir * 3.0 - Vec2::new(-dir.y, dir.x) * 4.0], col, Stroke::NONE));
            p.text(at - dir * 16.0, Align2::CENTER_CENTER, coords::dist_short(v.me.distance(w)), FontId::proportional(10.0), col);
        } else {
            p.text(at + Vec2::new(8.0, -2.0), Align2::LEFT_BOTTOM, label, FontId::proportional(11.0), col);
        }
    };
    landmark(v.station, GREEN, "정거장", true);
    landmark(v.jump_point, ACCENT, "워프", false);
    for w in v.marks {
        let (o, out) = inside(*w);
        let at = c + o;
        let d = 6.0;
        p.add(EShape::convex_polygon(vec![at + Vec2::new(0.0, -d), at + Vec2::new(d, 0.0), at + Vec2::new(0.0, d), at + Vec2::new(-d, 0.0)], GOLD, Stroke::NONE));
        if out {
            p.text(at - o.normalized() * 16.0, Align2::CENTER_CENTER, coords::dist_short(v.me.distance(*w)), FontId::proportional(10.0), GOLD);
        }
    }

    // Ships: a chevron along their heading, ▲/▼ when well above or below.
    let up = |f: Vec3| {
        let s = to(v.me + f * 100.0) - to(v.me);
        if s.length() > 1e-3 { s.normalized() } else { Vec2::new(0.0, -1.0) }
    };
    for b in v.blips {
        let (o, out) = inside(b.pos);
        if out && !b.target {
            continue;
        }
        let at = c + o;
        if b.wreck {
            p.text(at, Align2::CENTER_CENTER, "×", FontId::proportional(11.0), b.color);
            continue;
        }
        let s = if b.big { 7.0 } else { 4.5 };
        let d = up(b.fwd);
        let side = Vec2::new(-d.y, d.x);
        p.add(EShape::convex_polygon(vec![at + d * s, at - d * s * 0.6 + side * s * 0.7, at - d * s * 0.6 - side * s * 0.7], b.color, Stroke::NONE));
        let dy = b.pos.y - v.me.y;
        if dy.abs() > 300.0 {
            p.text(at + Vec2::new(s + 2.0, 0.0), Align2::LEFT_CENTER, if dy > 0.0 { "▲" } else { "▼" }, FontId::proportional(8.0), alpha(b.color, 200));
        }
        if b.target {
            p.circle_stroke(at, s + 4.0, Stroke::new(1.5, RED));
        }
    }
    // Me, in the middle, nose up.
    p.add(EShape::convex_polygon(vec![c + Vec2::new(0.0, -8.0), c + Vec2::new(5.5, 6.0), c + Vec2::new(0.0, 3.0), c + Vec2::new(-5.5, 6.0)], GOLD, Stroke::NONE));

    // Header: where, how far it reaches, how to open the chart.
    p.text(rect.left_top() + Vec2::new(2.0, 0.0), Align2::LEFT_TOP, v.system_name, FontId::proportional(13.0), crate::palette::exchange::TEXT);
    p.text(rect.right_top() + Vec2::new(-2.0, 1.0), Align2::RIGHT_TOP, format!("반경 {} · 성도 {}", coords::dist_short(range), v.map_key), FontId::proportional(11.0), DIM);

    if resp.clicked() {
        return Some(MinimapEvent::Open);
    }
    if resp.hovered() {
        // One step per wheel notch (the raw events, not the smoothed scroll).
        let wheel: f32 = ui.input(|i| i.events.iter().filter_map(|e| if let egui::Event::MouseWheel { delta, .. } = e { Some(delta.y) } else { None }).sum());
        if wheel > 0.0 && v.zoom > 0 {
            return Some(MinimapEvent::Zoom(v.zoom - 1));
        }
        if wheel < 0.0 && v.zoom + 1 < RANGES.len() {
            return Some(MinimapEvent::Zoom(v.zoom + 1));
        }
    }
    None
}
