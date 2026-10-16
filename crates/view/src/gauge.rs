//! HUD gauges: the shield/hull fractions trail behind the real ones (showing what was just lost),
//! and a red edge flash marks hull damage.

/// Lost gauge drains toward the real value at this many full gauges per second.
pub const DRAIN_PER_SEC: f32 = 0.35;
/// Hull-damage flash fades from 1 to 0 at this rate per second.
pub const FLASH_FADE_PER_SEC: f32 = 1.8;
/// Longest step one update advances (a stalled frame must not drain everything at once).
pub const MAX_STEP: f64 = 0.25;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GaugeTrail {
    /// Trailing [shield, hull] fractions, never below the real ones.
    pub lag: [f32; 2],
    /// 1 right after hull damage, fading to 0.
    pub flash: f32,
    last_hull: f32,
    last_t: f64,
}

impl GaugeTrail {
    /// Advance to `now` with the current shield and hull fractions.
    pub fn update(&mut self, shield: f32, hull: f32, now: f64) {
        let dt = (now - self.last_t).clamp(0.0, MAX_STEP) as f32;
        if hull < self.last_hull - 1e-3 {
            self.flash = 1.0;
        }
        self.last_hull = hull;
        self.last_t = now;
        for (lag, cur) in self.lag.iter_mut().zip([shield, hull]) {
            // Drain toward the real value; jump up immediately on regen/repair.
            *lag = if cur >= *lag { cur } else { (*lag - dt * DRAIN_PER_SEC).max(cur) };
        }
        self.flash = (self.flash - dt * FLASH_FADE_PER_SEC).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut GaugeTrail, from: f64, to: f64, shield: f32, hull: f32) {
        let mut t = from;
        while t < to - 1e-9 {
            t = (t + 1.0 / 60.0).min(to);
            g.update(shield, hull, t);
        }
    }

    #[test]
    fn lost_part_lingers_then_drains_at_a_fixed_rate() {
        let mut g = GaugeTrail::default();
        run(&mut g, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(g.lag, [1.0, 1.0]);
        assert_eq!(g.flash, 0.0, "no damage, no flash");
        // Take a hit: shield to 40%, hull to 70%.
        g.update(0.4, 0.7, 1.0 + 1.0 / 60.0);
        assert!(g.flash > 0.95, "hull damage flashes");
        assert!(g.lag[0] > 0.99 && g.lag[1] > 0.99, "the lost part still shows");
        run(&mut g, 1.0 + 1.0 / 60.0, 2.0, 0.4, 0.7);
        // One second after the hit (the hit frame itself drained too): 0.35 of a gauge gone.
        assert!((g.lag[0] - (1.0 - DRAIN_PER_SEC)).abs() < 1e-3, "{:?}", g.lag);
        assert!(g.flash == 0.0, "flash gone within a second");
        run(&mut g, 2.0, 5.0, 0.4, 0.7);
        assert_eq!(g.lag, [0.4, 0.7], "drains down to the real value, never below");
    }

    #[test]
    fn regen_jumps_up_and_shield_loss_does_not_flash() {
        let mut g = GaugeTrail::default();
        run(&mut g, 0.0, 0.5, 0.5, 0.5);
        g.update(0.8, 0.9, 0.52);
        assert_eq!(g.lag, [0.8, 0.9]);
        g.update(0.2, 0.9, 0.54);
        assert_eq!(g.flash, 0.0, "only hull damage flashes");
    }

    #[test]
    fn a_stalled_frame_drains_at_most_one_step() {
        let mut g = GaugeTrail::default();
        g.update(1.0, 1.0, 10.0);
        g.update(0.0, 1.0, 10.01);
        g.update(0.0, 1.0, 40.0);
        assert!((g.lag[0] - (1.0 - 0.01 * DRAIN_PER_SEC - MAX_STEP as f32 * DRAIN_PER_SEC)).abs() < 1e-4, "{:?}", g.lag);
    }
}
