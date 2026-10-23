//! Client animation as a function of the clock (docs/ART_DIRECTION.md §8): every value here comes
//! from "now" and the moment something happened, never from a per-frame accumulation, so the same
//! clock gives the same picture at any frame rate.

use shared::glam::Quat;

/// A 0..1 level that rises at `up`/s while on and falls at `down`/s while off, from `from` at the
/// last switch `at` (engine boost glow).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ramp {
    pub on: bool,
    pub from: f32,
    pub at: f64,
}

impl Default for Ramp {
    fn default() -> Self {
        Ramp { on: false, from: 0.0, at: 0.0 }
    }
}

impl Ramp {
    pub fn value(&self, now: f64, up: f32, down: f32) -> f32 {
        let dt = (now - self.at).max(0.0) as f32;
        if self.on { (self.from + dt * up).min(1.0) } else { (self.from - dt * down).max(0.0) }
    }

    /// Switch on or off at `now`, continuing from the level reached so far.
    pub fn set(&mut self, on: bool, now: f64, up: f32, down: f32) {
        if on != self.on {
            *self = Ramp { on, from: self.value(now, up, down), at: now };
        }
    }
}

/// 1 at the moment of the event, falling linearly to 0 over `1/rate` seconds (hull hit flash).
pub fn fade(since: f64, rate: f32) -> f32 {
    (1.0 - since.max(0.0) as f32 * rate).max(0.0)
}

/// Seconds after a hit the shield ripple stays visible (shield.wesl: it is down to ~0.2% by then).
pub const SHIELD_SHOWN_AFTER_HIT: f32 = 2.0;

/// Whether a ship's shield bubble draws anything (shield.wesl): only while a hit's ripple is
/// visible, or while a nearly empty shield flickers. Otherwise it is fully transparent, so it is
/// hidden rather than drawn — thousands of idle bubbles cost a whole transparent pass in a battle.
pub fn shield_shown(frac: f32, hit_age: f32) -> bool {
    hit_age < SHIELD_SHOWN_AFTER_HIT || (frac > 0.0 && frac < 0.25)
}

/// Turret swing: from the rotation it had when its target changed toward the current aim, eased
/// over `1/rate` seconds; after that it follows the aim exactly.
pub fn swing(from: Quat, to: Quat, since: f64, rate: f32) -> Quat {
    let k = (since.max(0.0) as f32 * rate).min(1.0);
    from.slerp(to, k * k * (3.0 - 2.0 * k))
}

/// Engine throttle (0.05..1) from recent speeds, newest first: their mean, so the plume settles
/// over the sampled window instead of jumping with every snapshot.
pub fn throttle(speeds: &[f32], max_speed: f32) -> f32 {
    if speeds.is_empty() {
        return 0.05;
    }
    speeds.iter().map(|s| (s / max_speed).clamp(0.05, 1.0)).sum::<f32>() / speeds.len() as f32
}
/// How much of a ship's engine effects is drawn, by its size on screen. Each plume, nozzle glow
/// and nozzle bore is its own transparent draw: thousands of them, a few pixels each, were the
/// largest render cost of a 1000-captain battle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EngineLod {
    /// Plumes, nozzle glows and bores.
    #[default]
    Full,
    /// Plumes only: the nozzles would be a pixel or two.
    Plumes,
    /// Nothing: the ship itself is a dot.
    Off,
}

/// Ship radius on screen (px) from which its nozzle glows and bores are drawn, and its plumes.
pub const NOZZLE_PX: f32 = 24.0;
pub const PLUME_PX: f32 = 1.5;

/// The radius in pixels of a sphere of `radius` at `dist` on a screen `viewport_h` px tall with a
/// vertical field of view of `fov_y` radians.
pub fn screen_radius(radius: f32, dist: f32, viewport_h: f32, fov_y: f32) -> f32 {
    radius * viewport_h * 0.5 / (fov_y * 0.5).tan() / dist.max(1e-3)
}

/// The engine detail for a ship `radius_px` on screen that had `was`: a level drops only 15%
/// below its threshold, so a ship hovering at the boundary doesn't flicker.
pub fn engine_lod(radius_px: f32, was: EngineLod) -> EngineLod {
    let reaches = |px: f32, had: bool| radius_px >= if had { px * 0.85 } else { px };
    if reaches(NOZZLE_PX, was == EngineLod::Full) {
        EngineLod::Full
    } else if reaches(PLUME_PX, was != EngineLod::Off) {
        EngineLod::Plumes
    } else {
        EngineLod::Off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_is_a_function_of_the_clock_not_the_frame_rate() {
        // Same switches, sampled at 30 and 60 fps: identical values at the same moments.
        let run = |fps: f64| {
            let mut r = Ramp::default();
            let mut out = vec![];
            for f in 0..=(fps as usize * 2) {
                let now = f as f64 / fps;
                r.set((0.2..1.0).contains(&now), now, 3.0, 2.0);
                if f % (fps as usize / 10) == 0 {
                    out.push((r.value(now, 3.0, 2.0) * 1000.0).round());
                }
            }
            out
        };
        assert_eq!(run(30.0), run(60.0));
    }

    #[test]
    fn ramp_continues_from_where_it_was() {
        let mut r = Ramp::default();
        r.set(true, 1.0, 3.0, 2.0);
        assert!((r.value(1.2, 3.0, 2.0) - 0.6).abs() < 1e-5);
        r.set(false, 1.2, 3.0, 2.0);
        assert!((r.value(1.3, 3.0, 2.0) - 0.4).abs() < 1e-5);
        assert_eq!(r.value(9.0, 3.0, 2.0), 0.0);
        r.set(true, 9.0, 3.0, 2.0);
        assert_eq!(r.value(20.0, 3.0, 2.0), 1.0);
    }

    #[test]
    fn a_shield_shows_only_after_a_hit_or_when_nearly_empty() {
        assert!(shield_shown(1.0, 0.0));
        assert!(shield_shown(1.0, 1.9));
        assert!(!shield_shown(1.0, 2.0));
        assert!(!shield_shown(1.0, 3.0));
        assert!(shield_shown(0.2, 3.0));
        assert!(!shield_shown(0.0, 3.0), "a collapsed shield is gone");
    }

    #[test]
    fn engine_detail_follows_size_on_screen_without_flicker() {
        use EngineLod::*;
        let fov = 62f32.to_radians();
        // A fighter (radius 10) on a 900 px screen: full up close, plumes at mid range, off far away.
        let px = |d: f32| screen_radius(10.0, d, 900.0, fov);
        assert_eq!(engine_lod(px(150.0), Full), Full);
        assert_eq!(engine_lod(px(1500.0), Full), Plumes);
        assert_eq!(engine_lod(px(9000.0), Plumes), Off);
        // Just under a threshold keeps what it had; coming back needs the full threshold.
        assert_eq!(engine_lod(NOZZLE_PX * 0.9, Full), Full);
        assert_eq!(engine_lod(NOZZLE_PX * 0.9, Plumes), Plumes);
        assert_eq!(engine_lod(PLUME_PX * 0.9, Plumes), Plumes);
        assert_eq!(engine_lod(PLUME_PX * 0.9, Off), Off);
        assert_eq!(engine_lod(NOZZLE_PX, Off), Full);
    }

    #[test]
    fn fade_and_swing_end_where_they_should() {
        assert_eq!(fade(0.0, 4.0), 1.0);
        assert!((fade(0.125, 4.0) - 0.5).abs() < 1e-6);
        assert_eq!(fade(1.0, 4.0), 0.0);
        let to = Quat::from_rotation_y(1.0);
        assert!(swing(Quat::IDENTITY, to, 0.0, 1.5).abs_diff_eq(Quat::IDENTITY, 1e-6));
        assert!(swing(Quat::IDENTITY, to, 0.7, 1.5).abs_diff_eq(to, 1e-6));
    }

    #[test]
    fn throttle_is_the_mean_of_clamped_speeds() {
        assert_eq!(throttle(&[], 140.0), 0.05);
        assert_eq!(throttle(&[280.0, 0.0], 140.0), (1.0 + 0.05) / 2.0);
    }
}
