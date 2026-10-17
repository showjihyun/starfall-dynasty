//! Weapon systems by ship size.
//! - Fighters (and NPCs, escorts): forward guns within a narrow cone of the nose.
//! - Cruisers/capitals: turret batteries that track targets all around the hull,
//!   plus a bow main gun that fires a heavy beam volley along the nose.

use super::*;
use super::interest::Probe;
use shared::ships::{SizeClass, WeaponKind};

pub(super) struct Shot {
    pub from: EntityId,
    pub to: EntityId,
    pub dmg: f32,
}

/// Grid queries pad the weapon range by this so huge hulls at the edge are still found.
const MAX_TARGET_RADIUS: f32 = 400.0;

impl Game {
    fn combat_pilot_bonus(&self, e: &Ent) -> f32 {
        match e.side() {
            Some(pid) if self.store.profile(pid).is_some_and(|p| p.profession == Profession::CombatPilot) => 1.08,
            _ => 1.0,
        }
    }

    /// `can_damage` for a grid probe: same rules, no entity lookups.
    fn hostile(&self, sys: SystemId, e: &Ent, t: &Probe) -> bool {
        if t.id == e.id || t.docked || e.docked {
            return false;
        }
        match (e.side(), t.side) {
            (None, None) => false,
            (None, Some(_)) | (Some(_), None) => true,
            (Some(pa), Some(pt)) => pa != pt && !(e.fleet.is_some() && e.fleet == t.fleet) && !(e.alliance.is_some() && e.alliance == t.alliance) && self.pvp_allowed(sys, e.alliance, t.alliance),
        }
    }

    /// Best target inside a cone around the nose (angular size of targets widens it a little).
    /// AI pilots only shoot what they are hunting; players take the best-aligned hostile.
    fn cone_target(&self, sys: SystemId, e: &Ent, range: f32, cone: f32, preferred: Option<EntityId>) -> Option<EntityId> {
        let fwd = e.forward();
        let aim = |pos: Vec3, radius: f32| {
            let to = pos - e.pos;
            let dist = to.length();
            (dist <= range + radius).then(|| fwd.angle_between(to / dist.max(1e-3)) - (radius / dist.max(1.0)).atan())
        };
        if let Some(p) = preferred
            && let Some(t) = self.ent(sys, p).filter(|t| !t.dead)
            && aim(t.pos, t.stats.radius).is_some_and(|a| a < self.tuning.combat.lock_cone.max(cone))
            && self.can_damage(sys, e.id, p)
        {
            return Some(p);
        }
        if e.kind != EntityKind::Player {
            return None;
        }
        let mut best: Option<(EntityId, f32)> = None;
        self.systems[&sys].grid.visit(e.pos, range + MAX_TARGET_RADIUS, |t| {
            if let Some(ang) = aim(t.pos, t.radius)
                && ang < cone
                && best.is_none_or(|(_, a)| ang < a)
                && self.hostile(sys, e, t)
            {
                best = Some((t.id, ang));
            }
        });
        best.map(|b| b.0)
    }

    pub(super) fn fire_forward_guns(&mut self, sys: SystemId, id: EntityId, shots: &mut Vec<Shot>) {
        let Some(e) = self.ent(sys, id) else { return };
        if e.dead || e.docked || !e.is_ship() || e.fire_cd > 0.0 {
            return;
        }
        // Big player ships trade fighter lasers for turrets + the main gun.
        if e.kind == EntityKind::Player && e.class.is_some_and(|c| c != SizeClass::Small) {
            return;
        }
        let wants = match e.kind {
            EntityKind::Player => e.controls.fire,
            EntityKind::Npc(_) | EntityKind::Escort => e.ai.as_ref().is_some_and(|a| a.target.is_some()),
            _ => false,
        };
        if !wants {
            return;
        }
        let cone = if e.kind == EntityKind::Player { self.tuning.combat.laser_cone } else { self.tuning.combat.npc_cone };
        let preferred = if e.kind == EntityKind::Player { e.target } else { e.ai.as_ref().and_then(|a| a.target) };
        let best = self.cone_target(sys, e, e.stats.weapon_range, cone, preferred);
        let dmg = e.stats.damage * self.combat_pilot_bonus(e);
        let interval = e.stats.fire_interval;
        if e.kind != EntityKind::Player && best.is_none() {
            return;
        }
        self.ent_mut(sys, id).unwrap().fire_cd = interval;
        let to = best.unwrap_or(0);
        if to != 0 {
            shots.push(Shot { from: id, to, dmg });
        }
        self.fx(sys, Fx::Shot { from: id, to, weapon: WeaponKind::Laser, mount: (self.tick % 4) as u8, hit: to != 0 });
    }

    /// Who a turret battery engages: the locked/AI target if in reach, otherwise the closest
    /// hostile that is actively in the fight (or anything hostile while the trigger is held).
    /// The search runs on a staggered ~0.5 s timer; between searches the last pick is kept while it stays valid.
    fn turret_target(&self, sys: SystemId, e: &Ent) -> (Option<EntityId>, bool) {
        let range = e.stats.turret_range;
        let preferred = if e.kind == EntityKind::Player { e.target } else { e.ai.as_ref().and_then(|a| a.target) };
        let in_reach = |t: &Ent| t.pos.distance(e.pos) <= range + t.stats.radius;
        let valid = |id: EntityId| self.ent(sys, id).is_some_and(|t| !t.dead && in_reach(t)) && self.can_damage(sys, e.id, id);
        if let Some(p) = preferred
            && valid(p)
        {
            return (Some(p), false);
        }
        // Keep the last pick (or the last "nothing in reach") until the timer runs out; a target
        // that died or left reach triggers a fresh search at once.
        if e.retarget_cd > 0.0 {
            match e.turret_target {
                Some(t) if valid(t) => return (Some(t), false),
                None => return (None, false),
                _ => {}
            }
        }
        let now = self.now;
        let side = e.side();
        let recent: HashSet<EntityId> = e.attackers.iter().filter(|(_, _, at)| now - at < 15.0).map(|(a, _, _)| *a).collect();
        let mut best: Option<(EntityId, f32)> = None;
        self.systems[&sys].grid.visit(e.pos, range + MAX_TARGET_RADIUS, |t| {
            let d = t.pos.distance(e.pos);
            if d > range + t.radius || best.is_some_and(|(_, bd)| d >= bd) {
                return;
            }
            let engaged = match side {
                // Player side: shoot pirates that are hunting us, anyone who hit us, or anything if trigger held.
                Some(_) => e.controls.fire || t.hunting || recent.contains(&t.id),
                // NPC capital ships shoot any player-side ship in reach.
                None => true,
            };
            if engaged && self.hostile(sys, e, t) {
                best = Some((t.id, d));
            }
        });
        (best.map(|b| b.0), true)
    }

    pub(super) fn fire_turrets(&mut self, sys: SystemId, id: EntityId, dt: f32, shots: &mut Vec<Shot>) {
        let Some(e) = self.ent(sys, id) else { return };
        if e.turret_cds.is_empty() || e.dead || e.docked {
            return;
        }
        let (target, searched) = self.turret_target(sys, e);
        // Firing arcs: where the target sits in the ship's own frame (+y = the hull's top).
        let overlap = self.tuning.combat.turret_arc_overlap;
        let up = target.and_then(|t| self.ent(sys, t)).map(|t| (e.rot.inverse() * (t.pos - e.pos)).normalize_or_zero().y).unwrap_or(0.0);
        let covers = |i: usize| if i % 2 == 0 { up >= -overlap } else { up <= overlap };
        let dmg = e.stats.turret_damage * self.combat_pilot_bonus(e);
        let interval = e.stats.turret_interval;
        let n = e.turret_cds.len();
        let mut fired = vec![];
        {
            let e = self.ent_mut(sys, id).unwrap();
            e.turret_target = target;
            e.retarget_cd = if searched { 0.5 } else { e.retarget_cd - dt };
            for (i, cd) in e.turret_cds.iter_mut().enumerate() {
                *cd -= dt;
                if *cd <= 0.0 && target.is_some() && covers(i) {
                    // Stagger the batteries so they ripple instead of firing in lockstep.
                    *cd = interval * (0.85 + 0.3 * ((i * 7 + 3) % n.max(1)) as f32 / n.max(1) as f32);
                    fired.push(i as u8);
                } else if *cd < 0.0 {
                    *cd = 0.0;
                }
            }
        }
        if let Some(t) = target {
            for mount in fired {
                shots.push(Shot { from: id, to: t, dmg });
                self.fx(sys, Fx::Shot { from: id, to: t, weapon: WeaponKind::Pulse, mount, hit: true });
            }
        }
    }

    pub(super) fn fire_main_gun(&mut self, sys: SystemId, id: EntityId, dt: f32, shots: &mut Vec<Shot>) {
        let Some(e) = self.ent_mut(sys, id) else { return };
        e.main_cd = (e.main_cd - dt).max(0.0);
        if e.stats.main_damage <= 0.0 || e.dead || e.docked || e.kind != EntityKind::Player || !e.controls.fire || e.main_cd > 0.0 {
            return;
        }
        e.main_cd = e.stats.main_interval;
        let e = self.ent(sys, id).unwrap();
        let target = self.cone_target(sys, e, e.stats.main_range, self.tuning.combat.main_cone, e.target);
        let dmg = e.stats.main_damage * self.combat_pilot_bonus(e);
        if let Some(t) = target {
            shots.push(Shot { from: id, to: t, dmg });
        }
        // mount 255 = every bow port fires together.
        self.fx(sys, Fx::Shot { from: id, to: target.unwrap_or(0), weapon: WeaponKind::MainBeam, mount: 255, hit: target.is_some() });
    }

    /// Escort brain: hold formation on the flagship; engage its target (attack order) or whoever attacks it.
    pub(super) fn think_escort(&mut self, sys: SystemId, id: EntityId, owner: PlayerId) {
        let Some(e) = self.ent(sys, id) else { return };
        let ai = e.ai.clone().unwrap();
        let flag = self.player_entity(owner).filter(|(s, _)| *s == sys).and_then(|(_, f)| self.ent(sys, f)).cloned();
        let Some(flag) = flag else {
            if let Some(e) = self.ent_mut(sys, id) {
                e.controls = Controls::default();
            }
            return;
        };
        let attack = self.conn_of.get(&owner).and_then(|c| self.sessions.get(c)).is_some_and(|s| s.escort_attack);
        let now = self.now;
        let mut target = None;
        if attack && let Some(t) = flag.target.filter(|t| self.can_damage(sys, id, *t)) {
            target = Some(t);
        }
        if target.is_none() {
            // Defend: whoever recently hit the flagship or this escort.
            target = flag.attackers.iter().chain(e.attackers.iter()).rev().find(|(a, _, t)| now - t < 12.0 && self.can_damage(sys, id, *a)).map(|(a, _, _)| *a);
        }
        let mut c = Controls::default();
        let goal = match target.and_then(|t| self.ent(sys, t)) {
            Some(t) => {
                let d = t.pos.distance(e.pos);
                c.thrust = if d > e.stats.weapon_range * 0.6 { 1.0 } else { 0.3 };
                let (sx, sy) = super::sim::orbit_strafe(id, ai.orbit_sign, now, &self.tuning.npcs.tactics);
                (c.strafe_x, c.strafe_y) = (sx * 0.6, sy * 0.6);
                c.boost = d > 2000.0;
                t.pos + t.vel * 0.4
            }
            None => {
                let slot = flag.pos + flag.rot * (escort_slot(ai.slot) * flag.stats.radius * 1.6);
                let d = slot.distance(e.pos);
                c.thrust = (d / 200.0).clamp(0.0, 1.0);
                c.boost = d > 900.0;
                // Close to the slot: face the flagship's heading.
                if d < 60.0 { e.pos + flag.forward() * 500.0 } else { slot }
            }
        };
        let local = e.rot.inverse() * (goal - e.pos).normalize_or(e.forward());
        c.yaw = (local.x * 3.0).clamp(-1.0, 1.0);
        c.pitch = (local.y * 3.0).clamp(-1.0, 1.0);
        if local.z > 0.0 {
            c.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
        }
        let e = self.ent_mut(sys, id).unwrap();
        e.controls = c;
        if let Some(a) = e.ai.as_mut() {
            a.target = target;
        }
    }
}
