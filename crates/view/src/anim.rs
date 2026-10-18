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
