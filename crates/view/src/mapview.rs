//! Star map (M) view state: projection, zoom and pan, and the continuous zoom between the two charts.
//! - Galaxy chart: light-years, +z (rimward) up the screen when flat; scale in px per ly. It shares
//!   the system chart's 3D camera (`yaw`, `tilt`): systems sit at their true height above or below
//!   the galactic plane (y = 0), with stems down to it.
//! - System chart: a 3D tactical view (main goal "3D freedom"). An orthographic camera turned by
//!   `yaw` about the vertical and tipped by `tilt` from straight down (0 = the flat top-down chart,
//!   +z south down the screen) looks at a reference plane at `s_height` (my ship's height while
//!   following); scale in px per m. Objects off the plane get a stem down (or up) to it.
//! Zooming in past the galaxy limit dives into the current system; zooming out of a fully framed
//! system returns to the galaxy, centred on that system.

use egui::{Pos2, Rect, Vec2 as EVec2};
use shared::galaxy::{self, Layout, SystemId};
use shared::glam::{Vec2, Vec3};

/// Galaxy chart scale limits (pixels per light-year); zooming in past the max enters the system chart.
pub const G_MIN: f32 = 40.0;
pub const G_MAX: f32 = 700.0;
/// System chart: the farthest-out zoom shows the whole system (its extent plus this margin) before
/// returning to the galaxy; opening the map frames this radius (meters) around my ship.
pub const S_MARGIN: f32 = 1.25;
pub const S_OPEN_RADIUS: f32 = 2_500.0;
pub const S_MAX: f32 = 2.0;
/// Mouse-wheel delta → zoom factor exponent.
const WHEEL: f32 = 0.0025;
/// System chart tilt (radians from straight down): the default 3D view and the most it tips over.
pub const TILT_DEFAULT: f32 = 0.9;
pub const TILT_MAX: f32 = 1.35;
/// Right-drag pixels → radians of yaw and tilt.
const ROTATE: f32 = 0.008;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Level {
    Galaxy,
    #[default]
    System,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapView {
    pub level: Level,
    /// Galaxy chart centre (ly, x/z) and scale (px per ly).
    pub g_center: Vec2,
    pub g_scale: f32,
    /// System chart centre (m, x/z) and scale (px per m).
    pub s_center: Vec2,
    pub s_scale: f32,
    /// System chart camera: turn about the vertical, tip from straight down (radians), and the
    /// height (m) of the reference plane.
    pub yaw: f32,
    pub tilt: f32,
    pub s_height: f32,
    /// Keep my ship centred on the system chart (cleared by dragging).
    pub follow: bool,
    pub was_open: bool,
    pub selected: Option<SystemId>,
}

impl Default for MapView {
    fn default() -> Self {
        MapView { level: Level::System, g_center: Vec2::new(0.4, 2.8), g_scale: 120.0, s_center: Vec2::ZERO, s_scale: 0.03, yaw: 0.0, tilt: TILT_DEFAULT, s_height: 0.0, follow: true, was_open: false, selected: None }
    }
}

/// Radius (m) that contains every body of the system.
pub fn system_extent(layout: Option<&Layout>) -> f32 {
    layout
        .map(|l| {
            let pts = l.planets.iter().map(|p| p.pos.length() + p.radius).chain(l.asteroids.iter().map(|a| a.pos.length())).chain(l.pois.iter().map(|p| p.pos.length())).chain([l.station.length(), l.jump_point.length()]);
            pts.fold(2_000.0, f32::max)
        })
        .unwrap_or(16_000.0)
}

/// A 1/2/5×10^n step whose on-screen size is at least `min_px`.
pub fn nice_step(scale: f32, min_px: f32) -> f32 {
    let raw = min_px / scale.max(1e-9);
    let mag = 10f32.powf(raw.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if mag * m >= raw {
            return mag * m;
        }
    }
    mag * 10.0
}

/// Scale (px per m) at which the whole system fits the map.
pub fn system_fit(map: Rect, extent: f32) -> f32 {
    map.size().min_elem() * 0.5 / (extent * S_MARGIN)
}

impl MapView {
    /// Call every frame with whether the map is open; every opening starts on my ship.
    pub fn set_open(&mut self, open: bool, screen: Rect) {
        if open && !self.was_open {
            self.level = Level::System;
            self.follow = true;
            self.selected = None;
            self.s_scale = screen.size().min_elem() * 0.5 / S_OPEN_RADIUS;
        }
        self.was_open = open;
    }

    /// Camera-space offset (units) → screen offset (units, +y down): turn by `yaw`, tip by `tilt`.
    fn rot_proj(&self, rel: Vec3) -> EVec2 {
        let (sy, cy) = self.yaw.sin_cos();
        let (st, ct) = self.tilt.sin_cos();
        let (xr, zr) = (rel.x * cy - rel.z * sy, rel.x * sy + rel.z * cy);
        EVec2::new(xr, zr * ct - rel.y * st)
    }

    /// Screen offset (units) → the matching (x, z) move on the reference plane.
    fn rot_unproj(&self, off: EVec2) -> Vec2 {
        let (sy, cy) = self.yaw.sin_cos();
        let (xr, zr) = (off.x, off.y / self.tilt.cos().max(0.05));
        Vec2::new(xr * cy + zr * sy, -xr * sy + zr * cy)
    }

    /// Galaxy chart position (ly) → screen. North (+z) is up the screen, so z is flipped before the
    /// shared camera turns and tips it.
    pub fn galaxy_to_screen(&self, map: Rect, mp: [f32; 3]) -> Pos2 {
        let c = self.g_center;
        map.center() + self.rot_proj(Vec3::new(mp[0] - c.x, mp[1], -(mp[2] - c.y))) * self.g_scale
    }

    /// Screen → galaxy chart position (ly) on the galactic plane (y = 0).
    pub fn galaxy_from_screen(&self, map: Rect, sp: Pos2) -> [f32; 3] {
        let d = self.rot_unproj((sp - map.center()) / self.g_scale);
        [self.g_center.x + d.x, 0.0, self.g_center.y - d.y]
    }

    /// Visible galaxy area on the plane: (x0, x1, z0, z1) in light-years (bounding box of the
    /// screen corners, so a tilted chart's far side is included).
    pub fn galaxy_bounds(&self, map: Rect) -> (f32, f32, f32, f32) {
        let corners = [map.min, map.max, Pos2::new(map.min.x, map.max.y), Pos2::new(map.max.x, map.min.y)].map(|c| self.galaxy_from_screen(map, c));
        let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for c in corners {
            (x0, x1, z0, z1) = (x0.min(c[0]), x1.max(c[0]), z0.min(c[2]), z1.max(c[2]));
        }
        (x0, x1, z0, z1)
    }

    /// Drag and wheel on the galaxy chart. Zooming past the limit dives into the current system
    /// (framing all of it) and returns true.
    pub fn galaxy_input(&mut self, map: Rect, drag: Option<EVec2>, scroll: f32, hover: Option<Pos2>, extent: f32) -> bool {
        if let Some(d) = drag {
            let m = self.rot_unproj(d / self.g_scale);
            self.g_center -= Vec2::new(m.x, -m.y);
        }
        if scroll != 0.0 {
            let k = (scroll * WHEEL).exp();
            let new = (self.g_scale * k).clamp(G_MIN, G_MAX);
            // Zoom about the cursor: the point under it stays under it.
            if let Some(h) = hover {
                let before = self.galaxy_from_screen(map, h);
                self.g_scale = new;
                let after = self.galaxy_from_screen(map, h);
                self.g_center += Vec2::new(before[0] - after[0], before[2] - after[2]);
            }
            self.g_scale = new;
            if k > 1.0 && self.g_scale >= G_MAX {
                self.frame_system(map, extent);
                return true;
            }
        }
        false
    }

    /// System chart position (m) → screen. Higher objects appear higher up the screen once tilted.
    pub fn system_to_screen(&self, map: Rect, w: Vec3) -> Pos2 {
        let c = self.s_center;
        map.center() + self.rot_proj(Vec3::new(w.x - c.x, w.y - self.s_height, w.z - c.y)) * self.s_scale
    }

    /// Screen → system chart position (m) on the reference plane.
    pub fn system_from_screen(&self, map: Rect, sp: Pos2) -> Vec3 {
        let d = self.plane_offset(sp - map.center(), self.s_scale);
        Vec3::new(self.s_center.x + d.x, self.s_height, self.s_center.y + d.y)
    }

    /// A screen offset (px) at scale `sc` → the matching move on the reference plane (m, x/z).
    fn plane_offset(&self, off: EVec2, sc: f32) -> Vec2 {
        self.rot_unproj(off / sc)
    }

    /// The point on the reference plane under `w` (where its height stem ends).
    pub fn foot(&self, w: Vec3) -> Vec3 {
        Vec3::new(w.x, self.s_height, w.z)
    }

    /// Whether the chart is tipped enough for height to show.
    pub fn is_3d(&self) -> bool {
        self.tilt > 0.05
    }

    /// Right-drag: turn the camera about the vertical and tip it toward the horizon.
    pub fn rotate(&mut self, drag: EVec2) {
        self.yaw += drag.x * ROTATE;
        self.tilt = (self.tilt + drag.y * ROTATE).clamp(0.0, TILT_MAX);
    }

    /// "평면"/"입체": flat top-down chart ↔ the default 3D view (north up again).
    pub fn toggle_3d(&mut self) {
        if self.is_3d() {
            self.tilt = 0.0;
            self.yaw = 0.0;
        } else {
            self.tilt = TILT_DEFAULT;
        }
    }

    /// Drag and wheel on the system chart, then follow my ship if still following. Zooming out of a
    /// fully framed system returns to the galaxy chart centred on `here` and returns true.
    pub fn system_input(&mut self, map: Rect, drag: Option<EVec2>, scroll: f32, hover: Option<Pos2>, extent: f32, here: SystemId, my_pos: Option<Vec3>) -> bool {
        let fit_min = system_fit(map, extent);
        if let Some(d) = drag {
            self.s_center -= self.plane_offset(d, self.s_scale);
            self.follow = false;
        }
        if scroll != 0.0 {
            let k = (scroll * WHEEL).exp();
            if k < 1.0 && self.s_scale <= fit_min * 1.001 {
                let sys = galaxy::system(here);
                self.level = Level::Galaxy;
                self.g_scale = G_MAX * 0.9;
                self.g_center = Vec2::new(sys.map_pos[0], sys.map_pos[2]);
                return true;
            }
            let new = (self.s_scale * k).clamp(fit_min, S_MAX);
            if let (Some(h), false) = (hover, self.follow) {
                let off = h - map.center();
                self.s_center += self.plane_offset(off, self.s_scale) - self.plane_offset(off, new);
            }
            self.s_scale = new;
        }
        if self.follow
            && let Some(pos) = my_pos
        {
            self.s_center = Vec2::new(pos.x, pos.z);
            self.s_height = pos.y;
        }
        false
    }

    /// Jump the galaxy chart to a system and select it (side-panel click).
    pub fn show_system(&mut self, id: SystemId) {
        let sys = galaxy::system(id);
        self.selected = Some(id);
        self.level = Level::Galaxy;
        self.g_center = Vec2::new(sys.map_pos[0], sys.map_pos[2]);
    }

    /// "은하": the galaxy chart at its default scale around `here`.
    pub fn show_galaxy(&mut self, here: SystemId) {
        let sys = galaxy::system(here);
        self.level = Level::Galaxy;
        self.g_center = Vec2::new(sys.map_pos[0], sys.map_pos[2]);
        self.g_scale = 120.0;
    }

    /// "현재 성계": the whole current system, centred on its star.
    pub fn frame_system(&mut self, map: Rect, extent: f32) {
        self.level = Level::System;
        self.follow = false;
        self.s_center = Vec2::ZERO;
        self.s_scale = system_fit(map, extent);
    }

    /// "내 위치": back to the system chart, following my ship.
    pub fn follow_me(&mut self) {
        self.level = Level::System;
        self.follow = true;
    }

    /// The +/− buttons: zoom the current chart about its centre.
    pub fn zoom_by(&mut self, k: f32) {
        match self.level {
            Level::Galaxy => self.g_scale = (self.g_scale * k).clamp(G_MIN, G_MAX),
            Level::System => self.s_scale = (self.s_scale * k).min(S_MAX),
        }
    }
}

/// Whether the map may name a system: the core always, a generated one once someone has charted it
/// (or while you are in it).
pub fn known(id: SystemId, charted: &std::collections::BTreeSet<SystemId>, here: SystemId) -> bool {
    !galaxy::is_generated(id) || id == here || charted.contains(&id)
}

/// What the map calls a system nobody has reached.
pub const UNKNOWN_SYSTEM: &str = "미확인 성계";

/// The map's word on an asteroid (`minerals.ron` height_bonus): its mineral, then its climb from the
/// station and, where rare deposits can occur, the chance of one at that height next to the flat one.
pub fn asteroid_tip(mineral: &shared::minerals::Mineral, tier: galaxy::RiskTier, climb: f32) -> Vec<String> {
    let t = &shared::tuning::get().minerals;
    let (here, flat) = (shared::minerals::rare_chance(t, tier, climb), shared::minerals::rare_chance(t, tier, 0.0));
    let mut odds = format!("정거장 기준 고도각 {climb:+.0}°");
    if here > 0.0 {
        odds += &format!(" · 희귀 이상 {:.0}%", here * 100.0);
        if here > flat + 0.005 {
            odds += &format!(" (평지 {:.0}%)", flat * 100.0);
        }
    }
    vec![mineral.label(), odds]
}

/// Legend line for the rare-deposit ring, with the height rule when it is on (None: no rare deposits
/// in this tier).
pub fn deposit_legend(tier: galaxy::RiskTier) -> Option<String> {
    let t = &shared::tuning::get().minerals;
    let hb = t.height_bonus;
    let (flat, steep) = (shared::minerals::rare_chance(t, tier, 0.0), shared::minerals::rare_chance(t, tier, hb.full_deg));
    if steep <= 0.0 {
        return None;
    }
    Some(if steep > flat + 0.005 {
        format!("○ 희귀 이상 광맥 · 정거장에서 {:.0}° 넘게 오르내린 소행성일수록 많음 (평지 {:.0}% → {:.0}°에서 {:.0}%)", hb.from_deg, flat * 100.0, hb.full_deg, steep * 100.0)
    } else {
        "○ 희귀 이상 광맥".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 40.0), EVec2::new(950.0, 720.0))
    }

    #[test]
    fn nice_steps_are_1_2_5_decades() {
        assert_eq!(nice_step(1.0, 90.0), 100.0);
        assert_eq!(nice_step(0.03, 90.0), 5000.0);
        assert_eq!(nice_step(0.5, 90.0), 200.0);
        for sc in [0.001, 0.013, 0.2, 1.7] {
            let st = nice_step(sc, 90.0);
            assert!(st * sc >= 90.0 * 0.999 && st * sc < 90.0 * 5.0 + 1e-3, "{sc} → {st}");
        }
    }

    #[test]
    fn system_projection_round_trips_and_keeps_north_up() {
        let v = MapView { s_center: Vec2::new(1200.0, -300.0), s_scale: 0.07, tilt: 0.0, ..Default::default() };
        let m = map();
        assert_eq!(v.system_to_screen(m, Vec3::new(1200.0, 55.0, -300.0)), m.center(), "centre maps to the middle, height ignored");
        let north = v.system_to_screen(m, Vec3::new(1200.0, 0.0, -1300.0));
        assert!(north.y < m.center().y, "-Z (north) is up");
        for w in [Vec3::new(-8000.0, 0.0, 4000.0), Vec3::new(3.5, 0.0, -9.25)] {
            assert!(v.system_from_screen(m, v.system_to_screen(m, w)).distance(w) < 0.05);
        }
    }

    #[test]
    fn tilted_chart_shows_height() {
        let m = map();
        let v = MapView { s_center: Vec2::new(200.0, 100.0), s_scale: 0.05, yaw: 0.7, tilt: TILT_DEFAULT, s_height: 40.0, ..Default::default() };
        let base = Vec3::new(1500.0, 40.0, -900.0);
        let above = base + Vec3::Y * 2000.0;
        let (pb, pa) = (v.system_to_screen(m, base), v.system_to_screen(m, above));
        assert!(pa.y < pb.y - 50.0, "a ship 2 km above the plane is drawn well above its foot: {pb:?} {pa:?}");
        assert!((pa.x - pb.x).abs() < 1e-3, "the stem is vertical on screen");
        assert_eq!(v.foot(above), base);
        // Clicks land on the plane, under the cursor, at any yaw and tilt.
        for w in [base, Vec3::new(-7000.0, 40.0, 3000.0)] {
            assert!(v.system_from_screen(m, v.system_to_screen(m, w)).distance(w) < 0.05);
        }
        // An orthographic view: a sphere of radius r stays inside r × scale on screen (range rings).
        let c = v.system_to_screen(m, base);
        for d in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::new(1.0, 1.0, -1.0).normalize()] {
            assert!(v.system_to_screen(m, base + d * 1000.0).distance(c) <= 1000.0 * 0.05 + 1e-3);
        }
    }

    #[test]
    fn right_drag_rotates_and_toggle_flattens() {
        let mut v = MapView::default();
        assert!(v.is_3d(), "the system chart opens in 3D");
        v.rotate(EVec2::new(100.0, -1000.0));
        assert!((v.yaw - 0.8).abs() < 1e-5);
        assert_eq!(v.tilt, 0.0, "never tips past straight down");
        v.rotate(EVec2::new(0.0, 10_000.0));
        assert_eq!(v.tilt, TILT_MAX, "nor past the horizon limit");
        v.toggle_3d();
        assert_eq!((v.tilt, v.yaw), (0.0, 0.0));
        v.toggle_3d();
        assert_eq!(v.tilt, TILT_DEFAULT);
    }

    #[test]
    fn panning_a_tilted_chart_drags_the_plane() {
        let m = map();
        let mut v = MapView { s_scale: 0.05, yaw: 1.1, tilt: 1.0, follow: false, ..Default::default() };
        let grab = Pos2::new(600.0, 300.0);
        let under = v.system_from_screen(m, grab);
        v.system_input(m, Some(EVec2::new(-35.0, 22.0)), 0.0, None, 12_000.0, 1, None);
        let moved = grab + EVec2::new(-35.0, 22.0);
        assert!(v.system_from_screen(m, moved).distance(under) < 0.05, "the point grabbed stays under the cursor");
    }

    #[test]
    fn galaxy_chart_is_3d_too() {
        let m = map();
        let v = MapView { level: Level::Galaxy, g_center: Vec2::new(1.0, 3.0), g_scale: 150.0, yaw: 0.4, tilt: TILT_DEFAULT, ..Default::default() };
        // A system 0.6 ly above the plane is drawn above its foot; the stem is vertical on screen.
        let (foot, star) = (v.galaxy_to_screen(m, [2.0, 0.0, 4.0]), v.galaxy_to_screen(m, [2.0, 0.6, 4.0]));
        assert!(star.y < foot.y - 30.0 && (star.x - foot.x).abs() < 1e-3, "{foot:?} {star:?}");
        // Clicks land on the plane under the cursor; panning drags the plane.
        for p in [[2.0, 0.0, 4.0], [-1.5, 0.0, 7.25]] {
            let back = v.galaxy_from_screen(m, v.galaxy_to_screen(m, p));
            assert!((back[0] - p[0]).abs() < 1e-3 && (back[2] - p[2]).abs() < 1e-3);
        }
        let mut w = v.clone();
        let grab = Pos2::new(500.0, 300.0);
        let under = w.galaxy_from_screen(m, grab);
        w.galaxy_input(m, Some(EVec2::new(40.0, -25.0)), 0.0, None, 12_000.0);
        let now = w.galaxy_from_screen(m, grab + EVec2::new(40.0, -25.0));
        assert!((now[0] - under[0]).abs() < 1e-3 && (now[2] - under[2]).abs() < 1e-3);
        // Flat (tilt 0): north up, exactly the old chart.
        let flat = MapView { tilt: 0.0, yaw: 0.0, ..v.clone() };
        assert!(flat.galaxy_to_screen(m, [1.0, 0.0, 4.0]).y < flat.galaxy_to_screen(m, [1.0, 0.0, 3.0]).y);
    }

    #[test]
    fn galaxy_zoom_keeps_the_point_under_the_cursor() {
        let mut v = MapView { level: Level::Galaxy, g_scale: 200.0, tilt: 0.0, ..Default::default() };
        let m = map();
        let h = Pos2::new(700.0, 200.0);
        let under = |v: &MapView| {
            let c = m.center();
            [v.g_center.x + (h.x - c.x) / v.g_scale, v.g_center.y - (h.y - c.y) / v.g_scale]
        };
        let before = under(&v);
        assert!(!v.galaxy_input(m, None, 120.0, Some(h), 16_000.0));
        assert!(v.g_scale > 200.0);
        let after = under(&v);
        assert!((before[0] - after[0]).abs() < 1e-4 && (before[1] - after[1]).abs() < 1e-4, "{before:?} {after:?}");
        let p = v.galaxy_to_screen(m, [after[0], 0.0, after[1]]);
        assert!(p.distance(h) < 0.05);
    }

    #[test]
    fn zoom_is_continuous_between_the_two_charts() {
        let m = map();
        let extent = 12_000.0;
        let mut v = MapView { level: Level::Galaxy, g_scale: 650.0, ..Default::default() };
        // Wheel in past the galaxy limit: dive into the system, framing all of it.
        assert!(v.galaxy_input(m, None, 400.0, None, extent));
        assert_eq!(v.level, Level::System);
        assert_eq!(v.s_scale, system_fit(m, extent));
        assert!(!v.follow);
        // Already fully framed: wheel out returns to the galaxy, centred on this system.
        assert!(v.system_input(m, None, -50.0, None, extent, 4, None));
        assert_eq!(v.level, Level::Galaxy);
        let sys = galaxy::system(4);
        assert_eq!(v.g_center, Vec2::new(sys.map_pos[0], sys.map_pos[2]));
        assert!(v.g_scale < G_MAX);
    }

    #[test]
    fn system_chart_follows_my_ship_until_dragged() {
        let m = map();
        let mut v = MapView::default();
        v.set_open(true, m);
        assert!(v.follow && v.level == Level::System);
        v.system_input(m, None, 0.0, None, 12_000.0, 1, Some(Vec3::new(500.0, 9.0, -40.0)));
        assert_eq!((v.s_center, v.s_height), (Vec2::new(500.0, -40.0), 9.0), "the plane follows my height too");
        v.system_input(m, Some(EVec2::new(30.0, 0.0)), 0.0, None, 12_000.0, 1, Some(Vec3::new(900.0, 0.0, 0.0)));
        assert!(!v.follow);
        assert!(v.s_center.x < 500.0, "dragging right moves the view west");
        // Zoom never goes past the limits.
        for _ in 0..50 {
            v.system_input(m, None, 400.0, None, 12_000.0, 1, None);
        }
        assert_eq!(v.s_scale, S_MAX);
    }

    #[test]
    fn reopening_resets_to_my_ship() {
        let m = map();
        let mut v = MapView { level: Level::Galaxy, follow: false, selected: Some(3), was_open: false, ..Default::default() };
        v.set_open(true, m);
        assert_eq!((v.level, v.follow, v.selected), (Level::System, true, None));
        v.level = Level::Galaxy;
        v.set_open(true, m);
        assert_eq!(v.level, Level::Galaxy, "only the opening frame resets");
        v.set_open(false, m);
        v.set_open(true, m);
        assert_eq!(v.level, Level::System);
    }

    #[test]
    fn buttons_jump_between_views() {
        let m = map();
        let mut v = MapView::default();
        v.show_galaxy(2);
        assert_eq!((v.level, v.g_scale), (Level::Galaxy, 120.0));
        v.zoom_by(100.0);
        assert_eq!(v.g_scale, G_MAX, "buttons respect the galaxy limits");
        v.frame_system(m, 10_000.0);
        assert_eq!((v.level, v.follow, v.s_center), (Level::System, false, Vec2::ZERO));
        assert_eq!(v.s_scale, system_fit(m, 10_000.0));
        v.follow_me();
        assert!(v.follow);
        v.show_system(5);
        assert_eq!((v.level, v.selected), (Level::Galaxy, Some(5)));
    }

    #[test]
    fn unknown_until_charted() {
        let mut charted = std::collections::BTreeSet::new();
        assert!(known(9, &charted, 0), "the core is always known");
        assert!(!known(12, &charted, 0));
        assert!(known(12, &charted, 12), "you know where you are");
        charted.insert(12);
        assert!(known(12, &charted, 0));
    }

    #[test]
    fn extent_covers_every_body() {
        for id in 0..galaxy::systems().len() as SystemId {
            let l = galaxy::system_layout(id);
            let e = system_extent(Some(&l));
            assert!(e >= l.station.length() && e >= l.jump_point.length());
            assert!(l.pois.iter().all(|p| p.pos.length() <= e));
        }
        assert_eq!(system_extent(None), 16_000.0);
    }
}
