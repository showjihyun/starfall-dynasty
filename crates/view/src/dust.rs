//! Space dust: faint motes fixed in space in a box around the camera that wraps as the camera
//! moves and scales with the flagship (a fighter's dust is a few hundred metres across, a
//! capital's kilometres). Motes streak along the ship's velocity.

use shared::glam::Vec3;

/// The dust box for a ship of `radius` moving at `speed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    /// Edge length of the box around the camera (m).
    pub size: f32,
    pub half: f32,
    /// Mote thickness and streak length (m) before the edge fade.
    pub width: f32,
    pub len: f32,
}

impl Field {
    pub fn new(radius: f32, speed: f32) -> Field {
        let size = (radius * 45.0).clamp(380.0, 9_000.0);
        let width = size * 0.0005;
        // Streak length grows with speed relative to the ship's own size.
        let len = width * (1.0 + (speed / (radius * 0.6)).min(30.0));
        Field { size, half: size * 0.5, width, len }
    }

    /// Wrap an offset from the camera back into the box, so motes left behind reappear ahead.
    pub fn wrap(&self, rel: Vec3) -> Vec3 {
        let (s, h) = (self.size, self.half);
        Vec3::new((rel.x + h).rem_euclid(s) - h, (rel.y + h).rem_euclid(s) - h, (rel.z + h).rem_euclid(s) - h)
    }

    /// Size factor 0..1: motes shrink toward the box edges (so wrapping never pops) and right at the camera.
    pub fn fade(&self, rel: Vec3) -> f32 {
        let r = rel.length() / self.half;
        (1.0 - r * r).clamp(0.0, 1.0) * ((r - 0.06) * 4.0).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_scales_with_the_ship_within_limits() {
        assert_eq!(Field::new(9.0, 0.0).size, 405.0);
        assert_eq!(Field::new(2.0, 0.0).size, 380.0);
        assert_eq!(Field::new(400.0, 0.0).size, 9_000.0);
        let still = Field::new(10.0, 0.0);
        assert_eq!(still.len, still.width, "no streak at rest");
        let fast = Field::new(10.0, 1e6);
        assert_eq!(fast.len, fast.width * 31.0, "streaks are capped");
    }

    #[test]
    fn wrapping_keeps_motes_inside_the_box() {
        let f = Field::new(10.0, 0.0);
        for rel in [Vec3::new(1000.0, -3.0, 0.0), Vec3::new(-f.half - 1.0, f.half, 7.0), Vec3::new(0.2, -0.2, 0.1)] {
            let w = f.wrap(rel);
            assert!(w.abs().max_element() <= f.half, "{rel} → {w}");
            let d = (w - rel) / f.size;
            assert!((d - d.round()).abs().max_element() < 1e-4, "moved by whole boxes only");
        }
        assert_eq!(f.wrap(Vec3::new(3.0, -4.0, 5.0)), Vec3::new(3.0, -4.0, 5.0), "inside stays put");
    }

    #[test]
    fn motes_fade_at_the_edges_and_the_camera() {
        let f = Field::new(10.0, 0.0);
        assert_eq!(f.fade(Vec3::ZERO), 0.0);
        assert_eq!(f.fade(Vec3::new(f.half, 0.0, 0.0)), 0.0);
        assert!(f.fade(Vec3::new(f.half * 0.4, 0.0, 0.0)) > 0.8);
    }
}
