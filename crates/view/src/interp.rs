//! Snapshot interpolation: other ships are drawn `INTERP_DELAY` in the past, between the two
//! snapshots that bracket that moment; the own ship is extrapolated from the newest one so input
//! feels immediate.

use shared::glam::{Quat, Vec3};
use shared::protocol::{EntityId, EntityState};
use std::collections::{HashMap, VecDeque};

/// How far behind the newest snapshot other entities are rendered (seconds).
pub const INTERP_DELAY: f64 = 0.1;
/// Own-ship extrapolation never runs further ahead than this (seconds).
pub const MAX_EXTRAPOLATION: f64 = 0.12;

pub struct Snap {
    pub at: f64,
    pub ents: HashMap<EntityId, EntityState>,
}

/// Pose of `id` at render time `t` (same clock as `Snap::at`) and its newest state.
pub fn sample(snaps: &VecDeque<Snap>, me: Option<EntityId>, id: EntityId, t: f64) -> Option<(Vec3, Quat, &EntityState)> {
    let n = snaps.len();
    if n == 0 {
        return None;
    }
    let last = &snaps[n - 1];
    let cur = last.ents.get(&id)?;
    // Own ship: extrapolate from the newest snapshot for responsiveness.
    if Some(id) == me || n < 2 {
        let dt = ((t + INTERP_DELAY - last.at).clamp(0.0, MAX_EXTRAPOLATION)) as f32;
        return Some((Vec3::from(cur.pos) + Vec3::from(cur.vel) * dt, Quat::from_array(cur.rot), cur));
    }
    for i in (0..n - 1).rev() {
        let (a, b) = (&snaps[i], &snaps[i + 1]);
        if a.at <= t && t <= b.at {
            let (Some(ea), Some(eb)) = (a.ents.get(&id), b.ents.get(&id)) else { break };
            let k = ((t - a.at) / (b.at - a.at).max(1e-4)) as f32;
            let p = Vec3::from(ea.pos).lerp(Vec3::from(eb.pos), k);
            let r = Quat::from_array(ea.rot).slerp(Quat::from_array(eb.rot), k);
            return Some((p, r, eb));
        }
    }
    Some((Vec3::from(cur.pos), Quat::from_array(cur.rot), cur))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use shared::protocol::EntityKind;

    pub fn ent(id: EntityId, pos: [f32; 3], vel: [f32; 3]) -> EntityState {
        EntityState {
            id,
            kind: EntityKind::Player,
            name: format!("e{id}"),
            hull_class: None,
            rank: 0,
            livery: 0,
            pos,
            rot: Quat::IDENTITY.to_array(),
            vel,
            hull: 1.0,
            shield: 1.0,
            radius: 10.0,
            fleet: None,
            wanted: false,
            boosting: false,
            mining: None,
            docked: false,
            owner: None,
            alliance: None,
            class: None,
            species: None,
        }
    }

    fn snaps(frames: &[(f64, Vec<EntityState>)]) -> VecDeque<Snap> {
        frames.iter().map(|(at, es)| Snap { at: *at, ents: es.iter().map(|e| (e.id, e.clone())).collect() }).collect()
    }

    #[test]
    fn others_interpolate_between_bracketing_snapshots() {
        let rot_b = Quat::from_rotation_y(1.0);
        let mut b = ent(2, [10.0, 0.0, 0.0], [0.0; 3]);
        b.rot = rot_b.to_array();
        b.hull = 0.5;
        let s = snaps(&[(0.0, vec![ent(2, [0.0; 3], [0.0; 3])]), (0.05, vec![b])]);
        let (p, r, st) = sample(&s, Some(1), 2, 0.025).unwrap();
        assert!((p.x - 5.0).abs() < 1e-4, "{p}");
        assert!(r.angle_between(Quat::from_rotation_y(0.5)) < 1e-4);
        assert_eq!(st.hull, 0.5, "state comes from the newer snapshot");
    }

    #[test]
    fn own_ship_extrapolates_but_never_far() {
        let s = snaps(&[(0.0, vec![ent(1, [0.0; 3], [100.0, 0.0, 0.0])]), (0.05, vec![ent(1, [5.0, 0.0, 0.0], [100.0, 0.0, 0.0])])]);
        // Render time is INTERP_DELAY behind the newest snapshot: 0.02 s ahead of it here.
        let (p, ..) = sample(&s, Some(1), 1, 0.05 - INTERP_DELAY + 0.02).unwrap();
        assert!((p.x - 7.0).abs() < 1e-3, "{p}");
        let (far, ..) = sample(&s, Some(1), 1, 10.0).unwrap();
        assert!((far.x - (5.0 + 100.0 * MAX_EXTRAPOLATION as f32)).abs() < 1e-3, "clamped: {far}");
        let (past, ..) = sample(&s, Some(1), 1, -5.0).unwrap();
        assert_eq!(past.x, 5.0, "never extrapolates backwards");
    }

    #[test]
    fn newcomers_and_gaps_fall_back_to_the_newest_pose() {
        let s = snaps(&[(0.0, vec![]), (0.05, vec![ent(3, [7.0, 0.0, 0.0], [0.0; 3])])]);
        assert_eq!(sample(&s, None, 3, 0.02).unwrap().0.x, 7.0, "missing in the older snapshot");
        assert_eq!(sample(&s, None, 3, 3.0).unwrap().0.x, 7.0, "outside every bracket");
        assert!(sample(&s, None, 9, 0.02).is_none(), "gone from the newest snapshot");
        assert!(sample(&VecDeque::new(), None, 3, 0.0).is_none());
    }
}
