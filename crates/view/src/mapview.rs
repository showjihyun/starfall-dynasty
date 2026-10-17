//! Star map (M) view state: projection, zoom and pan, and the continuous zoom between the two charts.
//! - Galaxy chart: light-years on the x/z plane, +z up the screen; scale in px per ly.
//! - System chart: metres on the x/z plane, +z (south) down the screen; scale in px per m.
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
    /// Keep my ship centred on the system chart (cleared by dragging).
    pub follow: bool,
    pub was_open: bool,
    pub selected: Option<SystemId>,
}

impl Default for MapView {
    fn default() -> Self {
        MapView { level: Level::System, g_center: Vec2::new(0.4, 2.8), g_scale: 120.0, s_center: Vec2::ZERO, s_scale: 0.03, follow: true, was_open: false, selected: None }
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

    /// Galaxy chart position (ly) → screen.
    pub fn galaxy_to_screen(&self, map: Rect, mp: [f32; 3]) -> Pos2 {
        let (c, sc) = (self.g_center, self.g_scale);
        Pos2::new(map.center().x + (mp[0] - c.x) * sc, map.center().y - (mp[2] - c.y) * sc)
    }

    /// Visible galaxy area: (x0, x1, z0, z1) in light-years.
    pub fn galaxy_bounds(&self, map: Rect) -> (f32, f32, f32, f32) {
        let (c, sc) = (self.g_center, self.g_scale);
        (c.x - map.width() / 2.0 / sc, c.x + map.width() / 2.0 / sc, c.y - map.height() / 2.0 / sc, c.y + map.height() / 2.0 / sc)
    }

    /// Drag and wheel on the galaxy chart. Zooming past the limit dives into the current system
    /// (framing all of it) and returns true.
    pub fn galaxy_input(&mut self, map: Rect, drag: Option<EVec2>, scroll: f32, hover: Option<Pos2>, extent: f32) -> bool {
        if let Some(d) = drag {
            self.g_center -= Vec2::new(d.x, -d.y) / self.g_scale;
        }
        if scroll != 0.0 {
            let k = (scroll * WHEEL).exp();
            let new = (self.g_scale * k).clamp(G_MIN, G_MAX);
            // Zoom about the cursor.
            if let Some(h) = hover {
                let before = Vec2::new(h.x - map.center().x, -(h.y - map.center().y)) / self.g_scale;
                let after = Vec2::new(h.x - map.center().x, -(h.y - map.center().y)) / new;
                self.g_center += before - after;
            }
            self.g_scale = new;
            if k > 1.0 && self.g_scale >= G_MAX {
                self.frame_system(map, extent);
                return true;
            }
        }
        false
    }

    /// System chart position (m) → screen (height is ignored: the chart looks straight down).
    pub fn system_to_screen(&self, map: Rect, w: Vec3) -> Pos2 {
        let (c, sc) = (self.s_center, self.s_scale);
        Pos2::new(map.center().x + (w.x - c.x) * sc, map.center().y + (w.z - c.y) * sc)
    }

    /// Screen → system chart position (m, on the y = 0 plane).
    pub fn system_from_screen(&self, map: Rect, sp: Pos2) -> Vec3 {
        let (c, sc) = (self.s_center, self.s_scale);
        Vec3::new(c.x + (sp.x - map.center().x) / sc, 0.0, c.y + (sp.y - map.center().y) / sc)
    }

    /// Drag and wheel on the system chart, then follow my ship if still following. Zooming out of a
    /// fully framed system returns to the galaxy chart centred on `here` and returns true.
    pub fn system_input(&mut self, map: Rect, drag: Option<EVec2>, scroll: f32, hover: Option<Pos2>, extent: f32, here: SystemId, my_pos: Option<Vec3>) -> bool {
        let fit_min = system_fit(map, extent);
        if let Some(d) = drag {
            self.s_center -= Vec2::new(d.x, d.y) / self.s_scale;
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
                let off = Vec2::new(h.x - map.center().x, h.y - map.center().y);
                self.s_center += off / self.s_scale - off / new;
            }
            self.s_scale = new;
        }
        if self.follow
            && let Some(pos) = my_pos
        {
            self.s_center = Vec2::new(pos.x, pos.z);
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
        let v = MapView { s_center: Vec2::new(1200.0, -300.0), s_scale: 0.07, ..Default::default() };
        let m = map();
        assert_eq!(v.system_to_screen(m, Vec3::new(1200.0, 55.0, -300.0)), m.center(), "centre maps to the middle, height ignored");
        let north = v.system_to_screen(m, Vec3::new(1200.0, 0.0, -1300.0));
        assert!(north.y < m.center().y, "-Z (north) is up");
        for w in [Vec3::new(-8000.0, 0.0, 4000.0), Vec3::new(3.5, 0.0, -9.25)] {
            assert!(v.system_from_screen(m, v.system_to_screen(m, w)).distance(w) < 0.05);
        }
    }

    #[test]
    fn galaxy_zoom_keeps_the_point_under_the_cursor() {
        let mut v = MapView { level: Level::Galaxy, g_scale: 200.0, ..Default::default() };
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
        assert_eq!(v.s_center, Vec2::new(500.0, -40.0));
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
