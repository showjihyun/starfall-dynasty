//! Client animation as a function of the clock (docs/ART_DIRECTION.md §8): every value here comes
//! from "now" and the moment something happened, never from a per-frame accumulation, so the same
//! clock gives the same picture at any frame rate.

use shared::glam::{Quat, Vec3};

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

/// Where battery `k` of `n` (`beam` of them flank mounts, `shared::ships::battery_covers`) fires
/// from on a model without turret nodes, in the ship's own frame for a hull of `radius`: dorsal on
/// top, ventral under the belly, beam mounts on alternate flanks at the hull's plane, spread along
/// the length — so a leviathan's shots come from the side of the hull that may fire, not its bow.
/// `target_x`: the target's x in the ship's frame — a beam mount fires from the flank facing it.
pub fn battery_mount(k: usize, n: usize, beam: u8, radius: f32, target_x: f32) -> Vec3 {
    let along = ((k / 2) as f32 + 0.5) / (n.div_ceil(2).max(1)) as f32 - 0.5;
    let z = along * radius * 1.2;
    if k.is_multiple_of(2) {
        Vec3::new(0.0, radius * 0.14, z)
    } else if k / 2 + beam as usize >= n / 2 {
        let side = if target_x != 0.0 { target_x.signum() } else if (k / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
        Vec3::new(side * radius * 0.3, 0.0, z)
    } else {
        Vec3::new(0.0, -radius * 0.14, z)
    }
}

/// Seconds a turret bolt takes from muzzle to target (it is drawn flying; the hit itself is the server's).
pub const BOLT_SECS: f32 = 0.22;
/// Length of a turret bolt's bright tail (m).
pub const BOLT_TAIL: f32 = 110.0;

/// A turret bolt's tail as a fraction of its path of `len` metres (the beam shader's unit): short
/// against a long shot, never the whole of a point-blank one.
pub fn bolt_tail(len: f32) -> f32 {
    (BOLT_TAIL / len.max(1.0)).clamp(0.04, 0.6)
}

/// Where a shot arriving from `from` (the shooter, in the target's own frame) lands on a hull of
/// `radius`: on the side facing the shooter, scattered by `seed` — a capital is hit along its
/// length and across its flank, not through one point at its centre. In the target's frame; the
/// hull is taken as an ellipsoid as long as the model (2.2 radii) and a good deal slimmer.
pub fn impact_point(from: Vec3, radius: f32, seed: u32) -> Vec3 {
    let unit = |k: u32| {
        let h = (seed ^ k.wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B);
        ((h ^ (h >> 15)).wrapping_mul(0xC2B2_AE35) >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    };
    let half = Vec3::new(0.42, 0.24, 1.0) * radius;
    // A direction somewhere on the hemisphere facing the shooter.
    let toward = from.normalize_or(Vec3::NEG_Z);
    let d = (toward + Vec3::new(unit(1), unit(2), unit(3)) * 0.85).normalize_or(toward);
    let d = if d.dot(toward) < 0.0 { d - toward * 2.0 * d.dot(toward) } else { d };
    d / (d / half).length()
}

/// Seconds a ship dropping out of warp takes to settle from its streak into its hull.
pub const WARP_IN_SECS: f32 = 0.7;
/// How long a ship looks along its way the moment it drops out of warp (times its length).
pub const WARP_IN_STRETCH: f32 = 8.0;

/// A ship dropping out of warp, `since` seconds ago: how stretched along its way it is drawn —
/// `WARP_IN_STRETCH` at once, easing out to its own length (1) by `WARP_IN_SECS`.
pub fn warp_stretch(since: f64) -> f32 {
    let x = since as f32 / WARP_IN_SECS;
    if !(0.0..1.0).contains(&x) {
        return 1.0;
    }
    1.0 + (WARP_IN_STRETCH - 1.0) * (1.0 - x).powi(3)
}

/// A warp drive charging for `charge` seconds, `since` seconds in: the glow around the ship (0..1),
/// building with the square of the charge and beating faster towards the jump (2 → 8 beats a second).
pub fn warp_spool(since: f64, charge: f32) -> f32 {
    let t = since.max(0.0) as f32;
    let x = (t / charge.max(0.1)).min(1.0);
    let beat = 0.5 + 0.5 * (std::f32::consts::TAU * (2.0 + 6.0 * x) * t).sin();
    x * x * (0.7 + 0.3 * beat)
}

/// Camera for the ship on the character creation screen: (eye, point looked at). The look point sits
/// above the ship, so it lands in the screen's empty lower middle, under the text columns. The eye is
/// on the star's side (stars sit at the origin): seen from the far side the ship was a black silhouette.
pub fn preview_camera(ship: Vec3, radius: f32) -> (Vec3, Vec3) {
    let to_star = (-ship).normalize_or(Vec3::NEG_Z);
    (ship + to_star * radius * 5.0 + Vec3::Y * radius * 0.5, ship + Vec3::Y * radius * 1.5)
}

/// Rigged ship parts (`Rig_<Kind>_N` nodes, origin at the hinge; tools/blender/shiplib.py).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigKind {
    Dish,
    Gear,
    Vane,
}

impl RigKind {
    /// From a node's name: `Rig_Dish_0` → Dish.
    pub fn of(node: &str) -> Option<RigKind> {
        match node.strip_prefix("Rig_")?.split('_').next()? {
            "Dish" => Some(RigKind::Dish),
            "Gear" => Some(RigKind::Gear),
            "Vane" => Some(RigKind::Vane),
            _ => None,
        }
    }
}

/// A rigged part's rotation from its rest pose, as a function of the clock. `gear` 0 is folded, 1 down;
/// `boost` 0–1; `pulse_age`: seconds since this ship's scanner pulse (the dish whirls, then settles).
pub fn rig_rotation(kind: RigKind, now: f64, seed: f32, gear: f32, boost: f32, pulse_age: Option<f64>) -> Quat {
    match kind {
        RigKind::Dish => {
            let idle = (now * 0.8 + seed as f64).rem_euclid(std::f64::consts::TAU) as f32;
            let whirl = pulse_age.filter(|a| *a >= 0.0).map_or(0.0, |a| 14.0 * (1.0 - (-a as f32 / 0.8).exp()));
            Quat::from_rotation_y(idle + whirl)
        }
        RigKind::Gear => Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * (1.0 - gear.clamp(0.0, 1.0))),
        RigKind::Vane => Quat::from_rotation_x(-0.45 * boost.clamp(0.0, 1.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_streak_settles_into_the_hull_and_the_spool_builds_to_the_jump() {
        assert_eq!(warp_stretch(0.0), WARP_IN_STRETCH);
        assert_eq!(warp_stretch(WARP_IN_SECS as f64), 1.0);
        assert_eq!(warp_stretch(-1.0), 1.0, "before the event: the hull as it is");
        let s: Vec<f32> = (0..=7).map(|k| warp_stretch(k as f64 * 0.1)).collect();
        assert!(s.windows(2).all(|w| w[1] <= w[0]), "eases out without bouncing: {s:?}");
        assert_eq!(warp_spool(0.0, 4.0), 0.0);
        for k in 0..=50 {
            let v = warp_spool(k as f64 * 0.1, 4.0);
            assert!((0.0..=1.0).contains(&v), "{v}");
        }
        // Brighter towards the jump, whatever the beat.
        assert!(warp_spool(1.0, 4.0) < 0.07 && warp_spool(3.9, 4.0) > 0.65);
    }

    #[test]
    fn a_bolt_is_a_short_streak_on_a_long_shot_and_never_the_whole_of_a_short_one() {
        assert!((bolt_tail(1100.0) - 0.1).abs() < 1e-5);
        assert_eq!(bolt_tail(10_000.0), 0.04);
        assert_eq!(bolt_tail(50.0), 0.6);
        assert_eq!(bolt_tail(0.0), 0.6);
    }

    #[test]
    fn shots_land_on_the_side_of_the_hull_facing_the_shooter_and_spread_over_it() {
        let r = 200.0;
        for from in [Vec3::new(900.0, 100.0, -300.0), Vec3::new(-50.0, -800.0, 0.0), Vec3::new(0.0, 0.0, 2000.0)] {
            let hits: Vec<Vec3> = (0..200).map(|k| impact_point(from, r, k)).collect();
            for p in &hits {
                assert!(p.dot(from) >= -1e-3, "the far side is never hit: {p:?} from {from:?}");
                // On the hull: within its length and beam, and off its centre.
                assert!(p.z.abs() <= r * 1.001 && p.x.abs() <= r * 0.43 && p.y.abs() <= r * 0.25, "{p:?}");
                assert!(p.length() > r * 0.2, "{p:?}");
            }
            // Spread: a broadside is struck along a good part of the length, not at one point.
            let (lo, hi) = hits.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.z), hi.max(p.z)));
            assert!(hi - lo > r * 0.5, "spread {} from {from:?}", hi - lo);
        }
        // The same shot lands in the same place.
        assert_eq!(impact_point(Vec3::X, r, 7), impact_point(Vec3::X, r, 7));
        // A shooter at the very centre (never, but no NaN).
        assert!(impact_point(Vec3::ZERO, r, 1).is_finite());
    }

    #[test]
    fn battery_mounts_sit_on_the_side_of_the_hull_that_fires() {
        // Sentinel: 12 batteries, 5 of the 6 ventral ones on the flanks (npcs.ron ventral_turrets 1).
        let (n, beam, r) = (12, 5, 100.0);
        for k in 0..n {
            let m = battery_mount(k, n, beam, r, 0.0);
            for up in [-1.0f32, 0.0, 1.0] {
                // A battery that reaches straight up sits on top, straight down under the belly.
                if shared::ships::battery_covers(k, n, beam, up, 0.25) && up.abs() > 0.5 {
                    assert!(m.y * up > 0.0, "battery {k} reaches up={up} but sits at {m}");
                }
            }
            if !shared::ships::battery_covers(k, n, beam, 1.0, 0.25) && !shared::ships::battery_covers(k, n, beam, -1.0, 0.25) {
                assert!(m.y == 0.0 && m.x.abs() > r * 0.2, "beam battery {k} on a flank, at {m}");
            }
            assert!(m.length() < r, "battery {k} on the hull, at {m}");
        }
        let flanks: Vec<f32> = (0..n).map(|k| battery_mount(k, n, beam, r, 0.0)).filter(|m| m.y == 0.0).map(|m| m.x.signum()).collect();
        assert_eq!(flanks.len(), 5);
        assert!(flanks.contains(&1.0) && flanks.contains(&-1.0), "both flanks fire");
        // At a target on its left, every beam battery fires from the left flank (not through the hull).
        assert!((0..n).map(|k| battery_mount(k, n, beam, r, -300.0)).filter(|m| m.y == 0.0).all(|m| m.x < 0.0));
        // An ordinary ship: dorsal and ventral alternate, none on the flanks.
        assert!((0..6).all(|k| battery_mount(k, 6, 0, r, 0.0).y != 0.0));
    }

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
