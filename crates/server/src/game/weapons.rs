//! Weapon systems by ship size.
//! - Fighters (and NPCs, escorts): forward guns within a narrow cone of the nose.
//! - Cruisers/capitals: turret batteries that track targets all around the hull,
//!   plus a bow main gun that fires a heavy beam volley along the nose.

use super::*;
use shared::ships::{SizeClass, WeaponKind};

pub(super) struct Shot {
    pub from: EntityId,
    pub to: EntityId,
    pub dmg: f32,
}

const LASER_CONE: f32 = 0.11; // ~6.3°
const LOCK_CONE: f32 = 0.22;
const NPC_CONE: f32 = 0.35;
const MAIN_CONE: f32 = 0.12;

impl Game {
    fn combat_pilot_bonus(&self, e: &Ent) -> f32 {
        match e.side() {
            Some(pid) if self.store.profile(pid).is_some_and(|p| p.profession == Profession::CombatPilot) => 1.08,
            _ => 1.0,
        }
    }

    /// Best target inside a cone around the nose (angular size of targets widens it a little).
    fn cone_target(&self, sys: SystemId, e: &Ent, range: f32, cone: f32, preferred: Option<EntityId>) -> Option<EntityId> {
        let fwd = e.forward();
        let mut best: Option<(EntityId, f32)> = None;
        for (&tid, t) in &self.systems[&sys].ents {
            if tid == e.id || !t.is_ship() || t.dead {
                continue;
            }
            let to = t.pos - e.pos;
            let dist = to.length();
            if dist > range + t.stats.radius {
                continue;
            }
            let ang = fwd.angle_between(to.normalize_or_zero()) - (t.stats.radius / dist.max(1.0)).atan();
            let limit = if Some(tid) == preferred { LOCK_CONE.max(cone) } else { cone };
            if ang < limit && self.can_damage(sys, e.id, tid) && best.is_none_or(|(_, a)| ang < a) {
                best = Some((tid, ang));
            }
        }
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
        let cone = if e.kind == EntityKind::Player { LASER_CONE } else { NPC_CONE };
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
    fn turret_target(&self, sys: SystemId, e: &Ent) -> Option<EntityId> {
        let range = e.stats.turret_range;
        let preferred = if e.kind == EntityKind::Player { e.target } else { e.ai.as_ref().and_then(|a| a.target) };
        let in_reach = |t: &Ent| t.pos.distance(e.pos) <= range + t.stats.radius;
        if let Some(p) = preferred
            && let Some(t) = self.ent(sys, p)
            && in_reach(t)
            && self.can_damage(sys, e.id, p)
        {
            return Some(p);
        }
        let now = self.now;
        let side = e.side();
        let mut best: Option<(EntityId, f32)> = None;
        for (&tid, t) in &self.systems[&sys].ents {
            if !t.is_ship() || t.dead || !in_reach(t) || !self.can_damage(sys, e.id, tid) {
                continue;
            }
            let engaged = match side {
                // Player side: shoot pirates that are hunting us, anyone who hit us, or anything if trigger held.
                Some(_) => e.controls.fire || e.attackers.iter().any(|(a, _, at)| *a == tid && now - at < 15.0) || t.ai.as_ref().is_some_and(|a| a.target.is_some()),
                // NPC capital ships shoot any player-side ship in reach.
                None => true,
            };
            if !engaged {
                continue;
            }
            let d = t.pos.distance(e.pos);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((tid, d));
            }
        }
        best.map(|b| b.0)
    }

    pub(super) fn fire_turrets(&mut self, sys: SystemId, id: EntityId, dt: f32, shots: &mut Vec<Shot>) {
        let Some(e) = self.ent(sys, id) else { return };
        if e.turret_cds.is_empty() || e.dead || e.docked {
            return;
        }
        let target = self.turret_target(sys, e);
        let dmg = e.stats.turret_damage * self.combat_pilot_bonus(e);
        let interval = e.stats.turret_interval;
        let n = e.turret_cds.len();
        let mut fired = vec![];
        {
            let e = self.ent_mut(sys, id).unwrap();
            for (i, cd) in e.turret_cds.iter_mut().enumerate() {
                *cd -= dt;
                if *cd <= 0.0 && target.is_some() {
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
        let target = self.cone_target(sys, e, e.stats.main_range, MAIN_CONE, e.target);
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
                c.strafe_x = ai.orbit_sign * 0.6;
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
