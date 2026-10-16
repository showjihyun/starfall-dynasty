//! Fixed-step simulation: flight, weapons, NPC AI, deaths, engagements, snapshots.

use super::*;
use history::{DomainEvent, Side};
use shared::galaxy::RiskTier;

const BOUNDS: f32 = 16_000.0;

use super::weapons::Shot;

impl Game {
    pub fn fx(&mut self, sys: SystemId, fx: Fx) {
        self.pending_fx.entry(sys).or_default().push(fx);
    }

    pub fn players_in(&self, sys: SystemId) -> Vec<(ConnId, PlayerId)> {
        self.sessions.iter().filter(|(_, s)| s.system == sys && s.player.is_some()).map(|(c, s)| (*c, s.player.unwrap())).collect()
    }

    /// Hostility rules: fleets never hurt each other; PvP only where the risk tier allows it.
    pub fn can_damage(&self, sys: SystemId, a: EntityId, t: EntityId) -> bool {
        if a == t {
            return false;
        }
        let (Some(ae), Some(te)) = (self.ent(sys, a), self.ent(sys, t)) else { return false };
        if !te.is_ship() || te.dead || te.docked || ae.docked {
            return false;
        }
        match (ae.side(), te.side()) {
            // NPC hostiles never fight each other.
            (None, None) => false,
            (None, Some(_)) | (Some(_), None) => true,
            (Some(pa), Some(pt)) => {
                if pa == pt || (ae.fleet.is_some() && ae.fleet == te.fleet) || (ae.alliance.is_some() && ae.alliance == te.alliance) {
                    return false;
                }
                self.pvp_allowed(sys, ae.alliance, te.alliance)
            }
        }
    }

    /// PvP: Wild/War Zone/Deep always; anywhere but Core between alliances at war.
    pub fn pvp_allowed(&self, sys: SystemId, a: Option<AllianceId>, b: Option<AllianceId>) -> bool {
        let tier = galaxy::system(sys).tier;
        tier.pvp_allowed() || (tier != RiskTier::Core && self.at_war(a, b))
    }

    pub fn tick(&mut self, dt: f32, unix: u64) {
        self.now += dt as f64;
        self.unix = unix;
        self.tick += 1;

        let mut lap = std::time::Instant::now();
        self.tick_sessions(dt);
        self.lap(0, &mut lap);
        let sys_ids: Vec<SystemId> = self.systems.keys().copied().collect();
        for sys in sys_ids {
            self.tick_system(sys, dt);
        }
        lap = std::time::Instant::now();
        self.tick_engagements();

        if self.tick % 20 == 0 {
            self.store.decay_supply(0.9995);
        }
        if self.tick % 1200 == 0 {
            let day = missions::day_index(self.unix);
            let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
            for pid in online {
                let before = self.store.profile(pid).map(|p| p.daily_day);
                self.ensure_dailies(pid, day);
                if before != Some(day) {
                    self.toast(pid, "새로운 일일 미션이 도착했습니다");
                    self.push_profile(pid);
                }
            }
        }
        self.lap(6, &mut lap);
        self.send_snapshots();
        self.lap(7, &mut lap);
    }

    fn lap(&mut self, phase: usize, t: &mut std::time::Instant) {
        self.prof[phase] += t.elapsed().as_nanos();
        *t = std::time::Instant::now();
    }

    fn tick_sessions(&mut self, dt: f32) {
        let conns: Vec<ConnId> = self.sessions.keys().copied().collect();
        for conn in conns {
            let Some(s) = self.sessions.get(&conn) else { continue };
            let (Some(pid), Some(eid), sys) = (s.player, s.entity, s.system) else { continue };
            // Warp charge.
            if let Some((target, t)) = s.warp {
                let t = t - dt;
                let hit_recently = self.ent(sys, eid).is_some_and(|e| self.now - e.last_hit < 0.5);
                if hit_recently {
                    self.sessions.get_mut(&conn).unwrap().warp = None;
                    self.send(conn, ServerMsg::WarpCharging { system: target, seconds: -1.0 });
                    self.toast(pid, "피격으로 워프가 취소되었습니다");
                } else if t <= 0.0 {
                    self.complete_warp(pid, sys, eid, target);
                } else {
                    self.sessions.get_mut(&conn).unwrap().warp = Some((target, t));
                }
            }
            // Scanning.
            let Some(s) = self.sessions.get(&conn) else { continue };
            if let Some((poi, prog)) = s.scan {
                let Some(e) = self.ent(sys, eid) else { continue };
                let Some(p) = self.systems[&sys].layout.pois.iter().find(|p| p.id == poi) else { continue };
                if e.pos.distance(p.pos) > e.stats.scan_range * 1.1 {
                    self.sessions.get_mut(&conn).unwrap().scan = None;
                    self.send(conn, ServerMsg::ScanProgress { poi, frac: -1.0 });
                    self.toast(pid, "스캔 범위를 벗어나 스캔이 중단되었습니다");
                    continue;
                }
                let explorer = self.store.profile(pid).is_some_and(|p| p.profession == Profession::Explorer);
                let time = e.stats.scan_time * if explorer { 0.75 } else { 1.0 };
                let prog = prog + dt / time;
                if prog >= 1.0 {
                    self.sessions.get_mut(&conn).unwrap().scan = None;
                    self.send(conn, ServerMsg::ScanProgress { poi, frac: 1.0 });
                    self.finish_scan(pid, sys, poi);
                } else {
                    self.sessions.get_mut(&conn).unwrap().scan = Some((poi, prog));
                    if self.tick % 4 == 0 {
                        self.send(conn, ServerMsg::ScanProgress { poi, frac: prog });
                    }
                }
            }
        }
    }

    fn complete_warp(&mut self, pid: PlayerId, from: SystemId, eid: EntityId, to: SystemId) {
        let (hull, shield) = self.ent(from, eid).map(|e| (e.hull, e.shield)).unwrap_or((1.0, 1.0));
        self.fx(from, Fx::WarpOut { entity: eid });
        if let Some(c) = self.conn_of.get(&pid).copied() {
            self.sessions.get_mut(&c).unwrap().warp = None;
        }
        self.spawn_player(pid, to, false);
        if let Some((s, e)) = self.player_entity(pid) {
            if let Some(ent) = self.ent_mut(s, e) {
                ent.hull = hull.min(ent.stats.max_hull);
                ent.shield = shield.min(ent.stats.max_shield);
                // Arrive facing the station.
                let st = self.systems[&s].layout.station;
                let ent = self.ent_mut(s, e).unwrap();
                let dir = (st - ent.pos).normalize_or_zero();
                if dir != Vec3::ZERO {
                    ent.rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
                }
                ent.vel = dir * 60.0;
            }
            self.fx(s, Fx::WarpIn { entity: e });
        }
        let name = galaxy::system(to).name;
        let tier = galaxy::system(to).tier.label();
        self.toast(pid, format!("{name} 도착 — {tier}"));
    }

    fn tick_system(&mut self, sys: SystemId, dt: f32) {
        let has_players = self.sessions.values().any(|s| s.system == sys && s.player.is_some());
        let tier = galaxy::system(sys).tier;
        // Pirate population.
        if tier != RiskTier::Core && has_players {
            let s = self.systems.get_mut(&sys).unwrap();
            s.pirate_cd -= dt;
            let pop = s.ents.values().filter(|e| e.ai.as_ref().is_some_and(|a| a.scenario_owner.is_none())).count();
            let target = 2 + 2 * tier.pirate_level() as usize;
            if s.pirate_cd <= 0.0 && pop < target {
                s.pirate_cd = 18.0;
                self.spawn_pirate_group(sys, tier);
            }
        }

        let ids: Vec<EntityId> = self.systems[&sys].ents.keys().copied().collect();
        let mut shots: Vec<Shot> = Vec::new();
        let star_r = self.systems[&sys].layout.star_radius;

        let mut lap = std::time::Instant::now();
        // AI decisions first (needs a read-only view).
        if has_players {
            for &id in &ids {
                self.think(sys, id, dt);
            }
        }

        for &id in &ids {
            let now = self.now;
            let Some(e) = self.systems.get_mut(&sys).unwrap().ents.get_mut(&id) else { continue };
            if e.dead || e.kind == EntityKind::Wreck {
                continue;
            }
            e.fire_cd = (e.fire_cd - dt).max(0.0);
            e.missile_cd = (e.missile_cd - dt).max(0.0);
            if e.missile.is_some() {
                continue;
            }
            if e.docked {
                continue;
            }
            // Flight model with flight-assist.
            let c = e.controls;
            let rate = e.stats.turn * dt;
            e.rot = (e.rot * Quat::from_rotation_y(-c.yaw * rate) * Quat::from_rotation_x(c.pitch * rate) * Quat::from_rotation_z(-c.roll * rate * 1.3)).normalize();
            let (fwd, right, up) = (e.rot * Vec3::NEG_Z, e.rot * Vec3::X, e.rot * Vec3::Y);
            let boost = if c.boost { 1.7 } else { 1.0 };
            let desired = (fwd * c.thrust + right * c.strafe_x * 0.6 + up * c.strafe_y * 0.6) * e.stats.speed * boost;
            let dv = desired - e.vel;
            let max_dv = e.stats.accel * boost * dt;
            e.vel += if dv.length() > max_dv { dv.normalize() * max_dv } else { dv };
            e.pos += e.vel * dt;
            // Soft bounds and the star's corona.
            let d = e.pos.length();
            if d > BOUNDS {
                e.vel -= e.pos.normalize() * 40.0 * dt;
            }
            if d < star_r + 150.0 {
                e.pos = e.pos.normalize_or(Vec3::Z) * (star_r + 150.0);
                e.hull -= 40.0 * dt;
                e.last_hit = now;
            }
            // Shield regeneration after a quiet period.
            if now - e.last_hit > 4.0 {
                e.shield = (e.shield + e.stats.shield_regen * dt).min(e.stats.max_shield);
            }
            if e.hull <= 0.0 && !e.dead {
                e.hull = 0.0;
            }
        }

        self.lap(1, &mut lap);
        // Weapons: forward guns, turret batteries, bow main gun.
        let sm = self.systems.get_mut(&sys).unwrap();
        sm.grid.rebuild(sm.ents.values());
        for &id in &ids {
            self.fire_forward_guns(sys, id, &mut shots);
            self.fire_turrets(sys, id, dt, &mut shots);
            self.fire_main_gun(sys, id, dt, &mut shots);
        }

        self.lap(2, &mut lap);
        // Missiles.
        for &id in &ids {
            let Some(e) = self.ent(sys, id) else { continue };
            let Some(m) = e.missile.clone() else { continue };
            let target = self.ent(sys, m.target).filter(|t| !t.dead).map(|t| (t.pos, t.stats.radius));
            let e = self.ent_mut(sys, id).unwrap();
            let life = m.life - dt;
            e.missile.as_mut().unwrap().life = life;
            if let Some((tp, tr)) = target {
                let to = tp - e.pos;
                let desired = to.normalize_or_zero();
                let fwd = e.vel.normalize_or(e.forward());
                let steer = fwd.lerp(desired, (e.stats.turn * dt).min(1.0)).normalize_or(fwd);
                let speed = (e.vel.length() + 260.0 * dt).min(e.stats.speed);
                e.vel = steer * speed;
                e.rot = Quat::from_rotation_arc(Vec3::NEG_Z, steer);
                e.pos += e.vel * dt;
                if to.length() < tr + 8.0 {
                    e.dead = true;
                    shots.push(Shot { from: m.owner, to: m.target, dmg: m.damage });
                    let p = e.pos.to_array();
                    self.fx(sys, Fx::Explosion { pos: p, size: 0.6 });
                    continue;
                }
            } else {
                e.pos += e.vel * dt;
            }
            if life <= 0.0 {
                e.dead = true;
            }
        }

        for s in shots {
            self.apply_damage(sys, s.from, s.to, s.dmg);
        }

        self.lap(3, &mut lap);
        // Mining.
        for &id in &ids {
            self.tick_mining(sys, id, dt);
        }

        self.lap(4, &mut lap);
        // Deaths.
        let dead: Vec<EntityId> = self.systems[&sys].ents.values().filter(|e| (e.is_ship() && e.hull <= 0.0) || e.dead).map(|e| e.id).collect();
        for id in dead {
            self.kill(sys, id);
        }
        self.lap(5, &mut lap);
    }

    fn spawn_pirate_group(&mut self, sys: SystemId, tier: RiskTier) {
        let layout = self.systems[&sys].layout.clone();
        let seed = self.tick ^ (sys as u64) << 32;
        let mut r = shared::rng::Rng::new(seed);
        let anchor = if r.f32() < 0.6 && !layout.asteroids.is_empty() { r.pick(&layout.asteroids).pos } else if !layout.pois.is_empty() { r.pick(&layout.pois).pos } else { layout.station };
        let n = 1 + r.below(3);
        let level = tier.pirate_level();
        for i in 0..n {
            let arch = if tier.pirate_level() >= 2 && r.f32() < 0.3 { NpcArchetype::Gunship } else { NpcArchetype::Raider };
            let pos = anchor + Vec3::new(r.range(-300., 300.), r.range(-80., 80.) + 60.0, r.range(-300., 300.)) + Vec3::X * i as f32 * 40.0;
            // Never spawn on top of the station.
            if pos.distance(layout.station) < 1200.0 {
                continue;
            }
            self.spawn_npc(sys, arch, level, pos, None);
        }
    }

    /// NPC brain: pick a target, steer, decide to shoot.
    fn think(&mut self, sys: SystemId, id: EntityId, dt: f32) {
        let Some(e) = self.ent(sys, id) else { return };
        let Some(ai) = e.ai.clone() else { return };
        if let Some(owner) = ai.escort_of {
            return self.think_escort(sys, id, owner);
        }
        let mut ai = ai;
        ai.think_cd -= dt;
        let aggro = 1500.0 + 300.0 * ai.level as f32;
        if ai.think_cd <= 0.0 {
            ai.think_cd = 0.5;
            let fleet_of_owner = ai.scenario_owner.and_then(|o| self.fleet_of(o));
            let mut best: Option<(EntityId, f32)> = None;
            for (tid, t) in &self.systems[&sys].ents {
                if t.kind != EntityKind::Player || t.dead || t.docked {
                    continue;
                }
                let d = t.pos.distance(e.pos);
                let is_owner_side = ai.scenario_owner.is_some() && (t.owner == ai.scenario_owner || (fleet_of_owner.is_some() && t.fleet == fleet_of_owner));
                let attacked_me = e.attackers.iter().any(|(a, _, time)| a == tid && self.now - time < 20.0);
                let ok = if ai.scenario_owner.is_some() { is_owner_side || attacked_me } else { d < aggro || attacked_me };
                if ok && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((*tid, d));
                }
            }
            ai.target = best.map(|b| b.0);
            // Leash: wander home if nobody to fight.
        }
        let tgt = ai.target.and_then(|t| self.ent(sys, t)).map(|t| (t.pos, t.vel));
        let speed = e.stats.speed;
        let range = e.stats.weapon_range;
        let (pos, rot, vel) = (e.pos, e.rot, e.vel);
        let mut controls = Controls::default();
        let goal = match tgt {
            Some((tp, tv)) => {
                let to = tp - pos;
                let d = to.length();
                if d > range * 0.7 {
                    controls.thrust = 1.0;
                    tp + tv * 0.6
                } else {
                    // Strafe around the target while keeping guns on it.
                    controls.thrust = 0.25;
                    controls.strafe_x = ai.orbit_sign;
                    tp + tv * 0.3
                }
            }
            None => {
                let home_d = pos.distance(ai.home);
                if home_d > 300.0 {
                    controls.thrust = 0.6;
                }
                ai.home + Vec3::new((self.now as f32 * 0.1 + id as f32).sin() * 200.0, 0.0, (self.now as f32 * 0.1 + id as f32).cos() * 200.0)
            }
        };
        let fwd = rot * Vec3::NEG_Z;
        let desired = (goal - pos).normalize_or(fwd);
        // Convert desired direction into local yaw/pitch commands.
        let local = rot.inverse() * desired;
        controls.yaw = (local.x * 3.0).clamp(-1.0, 1.0);
        controls.pitch = (local.y * 3.0).clamp(-1.0, 1.0);
        if local.z > 0.0 {
            // Target behind us: turn hard.
            controls.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
        }
        let _ = (speed, vel);
        let e = self.ent_mut(sys, id).unwrap();
        e.controls = controls;
        e.ai = Some(ai);
    }

    fn tick_mining(&mut self, sys: SystemId, id: EntityId, dt: f32) {
        let Some(e) = self.ent(sys, id) else { return };
        let (Some(aid), Some(pid)) = (e.mining, e.owner) else { return };
        let Some(a) = self.systems[&sys].layout.asteroids.iter().find(|a| a.id == aid).cloned() else { return };
        if e.docked || e.pos.distance(a.pos) > galaxy::MINE_RANGE + a.radius + e.stats.radius + 30.0 {
            self.ent_mut(sys, id).unwrap().mining = None;
            self.toast(pid, "채굴 대상이 사거리를 벗어났습니다");
            return;
        }
        let prof = self.store.profile(pid).unwrap().clone();
        let cap = e.stats.cargo as u32;
        if prof.cargo_used() >= cap {
            self.ent_mut(sys, id).unwrap().mining = None;
            self.toast(pid, "화물칸이 가득 찼습니다 — 정거장에서 판매/정제하세요");
            return;
        }
        let rate = e.stats.mine_rate * if prof.profession == Profession::Miner { 1.4 } else { 1.0 };
        let e = self.ent_mut(sys, id).unwrap();
        e.mine_acc += rate * dt;
        if e.mine_acc >= 1.0 {
            let n = e.mine_acc.floor() as u32;
            e.mine_acc -= n as f32;
            let free = cap - prof.cargo_used();
            let n = n.min(free);
            if let Some(pm) = self.store.profile_mut(pid) {
                *pm.cargo.entry(a.ore.name().to_string()).or_default() += n;
            }
            self.progress(pid, Progress::Mined(n));
        }
    }

    pub fn apply_damage(&mut self, sys: SystemId, from: EntityId, to: EntityId, dmg: f32) {
        if !self.can_damage(sys, from, to) {
            return;
        }
        let now = self.now;
        let tier = galaxy::system(sys).tier;
        // Kill credit, crime and battles belong to the *player side* (escorts fight for their owner).
        let (attacker_pid, attacker_fleet, attacker_alliance) = {
            let a = self.ent(sys, from).unwrap();
            (a.side(), a.fleet, a.alliance)
        };
        let dir;
        {
            let a_pos = self.ent(sys, from).unwrap().pos;
            let t = self.ent_mut(sys, to).unwrap();
            dir = (a_pos - t.pos).normalize_or_zero();
            let absorbed = dmg.min(t.shield);
            t.shield -= absorbed;
            t.hull -= dmg - absorbed;
            t.last_hit = now;
            // One entry per attacker (latest hit last), so a ship under fire from hundreds stays cheap.
            t.attackers.retain(|(a, _, time)| *a != from && now - time < 30.0);
            t.attackers.push((from, attacker_pid, now));
            if absorbed > 0.0 {
                self.pending_fx.entry(sys).or_default().push(Fx::ShieldHit { entity: to, dir: dir.to_array() });
            }
        }
        // NPCs retaliate.
        if let Some(ai) = self.ent_mut(sys, to).unwrap().ai.as_mut()
            && ai.target.is_none()
            && ai.escort_of.is_none()
            && attacker_pid.is_some()
        {
            ai.target = Some(from);
        }
        let target = self.ent(sys, to).unwrap();
        let (Some(attacker_pid), Some(victim_pid)) = (attacker_pid, target.side()) else { return };
        let victim_fleet = target.fleet;
        let victim_alliance = target.alliance;
        let victim_wanted = self.player_entity(victim_pid).and_then(|(s, e)| self.ent(s, e)).is_some_and(|v| v.wanted);
        let at_war = self.at_war(attacker_alliance, victim_alliance);
        // Self-defence: the victim's side hit the attacker's side recently.
        let self_defence = self.hostility.get(&(victim_pid, attacker_pid)).is_some_and(|t| now - t < 30.0);
        self.hostility.insert((attacker_pid, victim_pid), now);
        // Crime in Wild space: attacking an innocent makes you Wanted (wars are not crimes).
        if tier.crime_tracked() && !victim_wanted && !self_defence && !at_war {
            let flagship = self.player_entity(attacker_pid);
            let already = flagship.and_then(|(s, e)| self.ent(s, e)).is_some_and(|e| e.wanted);
            if !already {
                let rank = self.store.profile(attacker_pid).map(|p| p.ship.rank()).unwrap_or(0) as u64;
                let amount = 2000 + 600 * rank;
                let name = self.store.profile(attacker_pid).unwrap().name.clone();
                if let Some(pm) = self.store.profile_mut(attacker_pid) {
                    pm.bounty += amount;
                    pm.reputation -= 5;
                }
                if let Some((s, e)) = flagship
                    && let Some(f) = self.ent_mut(s, e)
                {
                    f.wanted = true;
                }
                self.history.record(self.unix, DomainEvent::BountyPlaced { target: name, amount, system: sys, reason: "무고한 함장 공격".into() });
                self.toast(attacker_pid, format!("⚠ 범죄 기록: 현상금 {amount} CR이 걸렸습니다"));
                self.push_profile(attacker_pid);
            }
        }
        // Engagement tracking: sides are alliances, else fleets, else lone captains.
        let (ak, an) = self.side_key(attacker_pid, attacker_fleet, attacker_alliance);
        let (vk, vn) = self.side_key(victim_pid, victim_fleet, victim_alliance);
        let aname = self.store.profile(attacker_pid).unwrap().name.clone();
        let vname = self.store.profile(victim_pid).unwrap().name.clone();
        let s = self.systems.get_mut(&sys).unwrap();
        let eng = s.engagement.get_or_insert_with(|| Engagement { started: now, last_fire: now, sides: BTreeMap::new(), dirty: false });
        let fresh = eng.sides.is_empty();
        eng.last_fire = now;
        let new_a = eng.sides.entry(ak).or_insert_with(|| (an, BTreeSet::new(), 0)).1.insert(aname);
        let new_v = eng.sides.entry(vk).or_insert_with(|| (vn, BTreeSet::new(), 0)).1.insert(vname);
        eng.dirty |= new_a || new_v;
        if fresh {
            self.broadcast_engagement(sys);
        }
    }

    /// Engagement side key + display name for a player.
    pub fn side_key(&self, pid: PlayerId, fleet: Option<FleetId>, alliance: Option<AllianceId>) -> (String, String) {
        if let Some(a) = alliance.and_then(|a| self.alliance(a)) {
            return (format!("A{}", a.id), format!("[{}] {}", a.tag, a.name));
        }
        match fleet {
            Some(f) => {
                let leader = self.fleets.get(&f).and_then(|fl| self.store.profile(fl.leader)).map(|p| p.name.clone()).unwrap_or_default();
                (format!("F{f}"), format!("{leader}의 함대"))
            }
            None => (format!("P{pid}"), self.store.profile(pid).map(|p| p.name.clone()).unwrap_or_default()),
        }
    }

    fn kill(&mut self, sys: SystemId, id: EntityId) {
        let Some(e) = self.systems.get_mut(&sys).unwrap().ents.remove(&id) else { return };
        if e.kind == EntityKind::Missile {
            return;
        }
        if e.kind == EntityKind::Wreck {
            self.systems.get_mut(&sys).unwrap().ents.insert(id, e);
            return;
        }
        let size = (e.stats.radius / 10.0).max(1.0);
        self.fx(sys, Fx::Explosion { pos: e.pos.to_array(), size });
        let now = self.now;
        let killer = e.attackers.iter().rev().find(|(_, p, t)| p.is_some() && now - t < 30.0).map(|(a, p, _)| (*a, p.unwrap()));
        let killer_ent_name = e.attackers.last().and_then(|(a, _, _)| self.ent(sys, *a)).map(|k| k.name.clone());
        let tier = galaxy::system(sys).tier;
        match e.kind {
            EntityKind::Npc(arch) => {
                let level = e.ai.as_ref().map(|a| a.level).unwrap_or(0);
                if let Some((_, kpid)) = killer {
                    let mut reward = arch.bounty(level);
                    let mult = tier.reward_mult().max(1.0) * if self.store.profile(kpid).is_some_and(|p| p.profession == Profession::CombatPilot) { 1.3 } else { 1.0 };
                    reward.credits = (reward.credits as f32 * mult) as u64;
                    let group = self.credit_group(kpid, sys);
                    for (i, pid) in group.iter().enumerate() {
                        let mut r = reward.clone();
                        if i > 0 {
                            r.credits /= 2;
                            r.alloy /= 2;
                            r.crystal /= 2;
                            r.datacore /= 2;
                        }
                        if let Some(pm) = self.store.profile_mut(*pid) {
                            pm.wallet.add(&r);
                            if i == 0 {
                                pm.kills += 1;
                            }
                        }
                        self.toast(*pid, format!("{} 격파: {}", e.name, r.describe()));
                        self.progress(*pid, Progress::KilledPirate);
                    }
                    if matches!(arch, NpcArchetype::Warlord | NpcArchetype::Sentinel) {
                        let killers: Vec<String> = e.attackers.iter().filter_map(|(_, p, _)| *p).collect::<HashSet<_>>().into_iter().filter_map(|p| self.store.profile(p).map(|x| x.name.clone())).collect();
                        self.history.record(self.unix, DomainEvent::BossDestroyed { boss: arch.name().into(), system: sys, killers });
                    }
                }
                if let Some(owner) = e.ai.as_ref().and_then(|a| a.scenario_owner) {
                    self.scenario_wave_kill(owner, id);
                }
            }
            EntityKind::Escort => {
                if let Some(owner) = e.owner {
                    if let Some(c) = self.conn_of.get(&owner).copied() {
                        self.sessions.get_mut(&c).unwrap().escorts.retain(|x| *x != id);
                    }
                    self.toast(owner, format!("{} 격침 — 도킹하면 재편성됩니다", e.name));
                }
            }
            EntityKind::Player => {
                let vpid = e.owner.unwrap();
                self.despawn_escorts(vpid);
                let vprof = self.store.profile(vpid).unwrap().clone();
                let (killer_name, killer_ship) = match killer {
                    Some((_, kp)) => {
                        let kp = self.store.profile(kp).unwrap();
                        (kp.name.clone(), Some(kp.ship.name.clone()))
                    }
                    None => (killer_ent_name.unwrap_or_else(|| "우주의 위험".into()), None),
                };
                let pvp = killer.is_some_and(|(_, kp)| kp != vpid);
                let witnesses: Vec<String> = self.systems[&sys]
                    .ents
                    .values()
                    .filter(|w| w.kind == EntityKind::Player && w.pos.distance(e.pos) < 4000.0 && w.owner != Some(vpid) && w.owner != killer.map(|k| k.1))
                    .filter_map(|w| w.owner.and_then(|p| self.store.profile(p)).map(|p| p.name.clone()))
                    .take(12)
                    .collect();
                let cargo_lost = vprof.cargo_used();
                let ev = self
                    .history
                    .record(
                        self.unix,
                        DomainEvent::ShipDestroyed {
                            victim: vprof.name.clone(),
                            victim_ship: vprof.ship.name.clone(),
                            victim_rank: vprof.ship.designation().into(),
                            killer: killer_name.clone(),
                            killer_ship,
                            system: sys,
                            witnesses,
                            cargo_lost,
                            pvp,
                            victim_bounty: vprof.bounty,
                        },
                    )
                    .unwrap_or(0);
                // The wreck persists for days: anyone can come and investigate it later.
                self.store.data.wrecks.push(crate::store::StoredWreck {
                    system: sys,
                    pos: e.pos.to_array(),
                    victim: vprof.name.clone(),
                    ship: vprof.ship.name.clone(),
                    event: ev,
                    cargo: vprof.cargo.clone(),
                    created_unix: self.unix,
                    investigated_by: vec![],
                });
                let idx = self.store.data.wrecks.len() - 1;
                let wid = self.spawn_wreck_entity(sys, e.pos, format!("{}의 잔해 「{}」", vprof.name, vprof.ship.name), idx);
                if let Some(pm) = self.store.profile_mut(vpid) {
                    pm.cargo.clear();
                    pm.deaths += 1;
                }
                if let Some(c) = self.conn_of.get(&vpid).copied() {
                    let s = self.sessions.get_mut(&c).unwrap();
                    s.entity = None;
                    s.dead = true;
                    s.warp = None;
                    s.scan = None;
                    self.clear_wave(c);
                    self.send(c, ServerMsg::Died { killer: killer_name.clone(), wreck: wid });
                }
                // Engagement loss bookkeeping.
                let vkey = self.side_key(vpid, e.fleet, e.alliance).0;
                if let Some(eng) = self.systems.get_mut(&sys).unwrap().engagement.as_mut()
                    && let Some(side) = eng.sides.get_mut(&vkey)
                {
                    side.2 += 1;
                }
                if let Some((_, kpid)) = killer.filter(|_| pvp) {
                    if let Some(pm) = self.store.profile_mut(kpid) {
                        pm.kills += 1;
                    }
                    for p in self.credit_group(kpid, sys) {
                        self.progress(p, Progress::KilledPlayer);
                    }
                    if vprof.bounty > 0 {
                        let kname = self.store.profile(kpid).unwrap().name.clone();
                        if let Some(pm) = self.store.profile_mut(kpid) {
                            pm.wallet.credits += vprof.bounty;
                            pm.reputation += 3;
                        }
                        if let Some(pm) = self.store.profile_mut(vpid) {
                            pm.bounty = 0;
                        }
                        self.history.record(self.unix, DomainEvent::BountyClaimed { hunter: kname, target: vprof.name.clone(), amount: vprof.bounty, system: sys });
                        self.toast(kpid, format!("현상금 {} CR 회수!", vprof.bounty));
                    }
                    self.push_profile(kpid);
                }
                self.push_profile(vpid);
            }
            _ => {}
        }
    }

    /// Engagement banner: the biggest sides by name, the rest summarised (1000-captain brawls).
    fn broadcast_engagement(&mut self, sys: SystemId) {
        let Some(eng) = self.systems.get_mut(&sys).and_then(|s| s.engagement.as_mut()) else { return };
        eng.dirty = false;
        let mut sides: Vec<(&String, usize)> = eng.sides.values().map(|v| (&v.0, v.1.len())).collect();
        sides.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut names: Vec<String> = sides.iter().take(6).map(|(n, c)| format!("{n} ({c})")).collect();
        if sides.len() > 6 {
            let rest: usize = sides[6..].iter().map(|s| s.1).sum();
            names.push(format!("외 {}개 세력 ({rest}명)", sides.len() - 6));
        }
        for (c, _) in self.players_in(sys) {
            self.send(c, ServerMsg::Engagement { active: true, sides: names.clone() });
        }
    }

    fn tick_engagements(&mut self) {
        let now = self.now;
        if self.tick % 20 == 0 {
            let dirty: Vec<SystemId> = self.systems.values().filter(|s| s.engagement.as_ref().is_some_and(|e| e.dirty)).map(|s| s.id).collect();
            for sys in dirty {
                self.broadcast_engagement(sys);
            }
            self.hostility.retain(|_, t| now - *t < 30.0);
        }
        let ended: Vec<SystemId> = self.systems.values().filter(|s| s.engagement.as_ref().is_some_and(|e| now - e.last_fire > ENGAGEMENT_TIMEOUT)).map(|s| s.id).collect();
        for sys in ended {
            let eng = self.systems.get_mut(&sys).unwrap().engagement.take().unwrap();
            for (c, _) in self.players_in(sys) {
                self.send(c, ServerMsg::Engagement { active: false, sides: vec![] });
            }
            if eng.sides.len() < 2 {
                continue;
            }
            let total_losses: u32 = eng.sides.values().map(|s| s.2).sum();
            let min_losses = eng.sides.values().map(|s| s.2).min().unwrap_or(0);
            let winners: Vec<&String> = eng.sides.iter().filter(|(_, s)| s.2 == min_losses).map(|(k, _)| k).collect();
            let winner_key = if total_losses > 0 && winners.len() == 1 { Some(winners[0].clone()) } else { None };
            let sides: Vec<Side> = eng.sides.values().map(|(n, m, l)| Side { name: n.clone(), members: m.iter().cloned().collect(), losses: *l }).collect();
            let winner_name = winner_key.as_ref().map(|k| eng.sides[k].0.clone());
            self.history.record(self.unix, DomainEvent::EngagementConcluded { system: sys, sides, winner: winner_name.clone(), seconds: (eng.last_fire - eng.started).max(1.0) as u32 });
            if let Some(k) = winner_key {
                let reward = (1500.0 * galaxy::system(sys).tier.reward_mult()) as u64;
                let members: Vec<String> = eng.sides[&k].1.iter().cloned().collect();
                for name in members {
                    if let Some(pid) = self.store.find_by_name(&name) {
                        if let Some(pm) = self.store.profile_mut(pid) {
                            pm.wallet.credits += reward;
                            pm.wallet.datacore += 2;
                            pm.reputation += 4;
                        }
                        self.toast(pid, format!("교전 승리! +{reward} CR, 데이터코어 +2 — 전투가 연대기에 기록되었습니다"));
                        self.progress(pid, Progress::WonEngagement);
                    }
                }
            }
        }
    }
}
