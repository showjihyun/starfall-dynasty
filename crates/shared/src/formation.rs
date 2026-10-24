//! Fleet formations (main goal "epic spread-out battles"): the fleet leader picks one and every
//! other member gets a post around the leader's ship, shown on its HUD (`client` ui) and kept by
//! the war bench's captains (`sim::battle`). Spacing between posts: `ships.ron` `formation_spacing`.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Formation {
    /// No posts: everyone flies as they like.
    #[default]
    Free,
    /// Abreast: alternately right and left of the leader — the whole fleet's guns face the enemy.
    Line,
    /// Line astern: one behind the other — narrow, for passing between fronts.
    Column,
    /// A wall facing the enemy: right, left, above, below the leader, then the diagonals.
    Wall,
}

impl Formation {
    pub const ALL: [Formation; 4] = [Formation::Free, Formation::Line, Formation::Column, Formation::Wall];

    pub fn name(self) -> &'static str {
        match self {
            Formation::Free => "자유",
            Formation::Line => "횡대",
            Formation::Column => "종대",
            Formation::Wall => "벽",
        }
    }

    /// Where member `slot` (1 = the first after the leader) keeps station, in the leader's frame
    /// (x right, y up, z behind) in units of the spacing. None for the leader itself and in `Free`.
    pub fn post(self, slot: usize) -> Option<Vec3> {
        if slot == 0 {
            return None;
        }
        let side = |k: usize| if k % 2 == 1 { 1.0 } else { -1.0 };
        let rank = slot.div_ceil(2) as f32;
        match self {
            Formation::Free => None,
            Formation::Line => Some(Vec3::new(side(slot) * rank, 0.0, 0.0)),
            Formation::Column => Some(Vec3::new(0.0, 0.0, slot as f32)),
            Formation::Wall => {
                const RING: [(f32, f32); 8] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0)];
                let (x, y) = RING[(slot - 1) % RING.len()];
                let ring = ((slot - 1) / RING.len() + 1) as f32;
                Some(Vec3::new(x * ring, y * ring, 0.0))
            }
        }
    }

    /// Member `slot`'s post in the world around a leader at `pos` facing `rot`, posts `spacing` apart.
    pub fn post_at(self, slot: usize, pos: Vec3, rot: Quat, spacing: f32) -> Option<Vec3> {
        self.post(slot).map(|p| pos + rot * (p * spacing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posts_spread_the_fleet_and_never_stack() {
        for f in [Formation::Line, Formation::Column, Formation::Wall] {
            let posts: Vec<Vec3> = (1..5).map(|k| f.post(k).unwrap()).collect();
            for (i, a) in posts.iter().enumerate() {
                assert!(a.length() >= 1.0, "{f:?} slot {} sits on the leader", i + 1);
                for b in &posts[i + 1..] {
                    assert!(a.distance(*b) >= 1.0, "{f:?}: two posts stacked at {a}");
                }
            }
        }
        assert!(Formation::Free.post(1).is_none() && Formation::Line.post(0).is_none());
        // Abreast is across the leader's heading, astern along it, the wall faces forward.
        assert!((1..5).all(|k| Formation::Line.post(k).unwrap().x != 0.0 && Formation::Line.post(k).unwrap().z == 0.0));
        assert!((1..5).all(|k| Formation::Column.post(k).unwrap().z > 0.0));
        assert!((1..5).all(|k| Formation::Wall.post(k).unwrap().z == 0.0));
        assert!((1..5).any(|k| Formation::Wall.post(k).unwrap().y != 0.0), "the wall uses height");
        // In the world: turns with the leader.
        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let p = Formation::Line.post_at(1, Vec3::ZERO, rot, 400.0).unwrap();
        assert!((p - rot * Vec3::new(400.0, 0.0, 0.0)).length() < 1e-3);
    }
}
