//! Fixed-step simulation: flight, weapons, NPC AI, deaths, engagements, snapshots.

use super::*;
use shared::prof;
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

    /// Fleet members in `sys` holding their formation post (`Controls::hold_post`): entity → (post,
    /// leader's velocity). The leader must be in the same system.
    fn held_posts(&self, sys: SystemId) -> std::collections::BTreeMap<EntityId, (Vec3, Vec3)> {
        let mut out = std::collections::BTreeMap::new();
        let spacing = self.tuning.ships.formation_spacing;
        for f in self.fleets.values().filter(|f| f.formation != shared::formation::Formation::Free) {
            let Some(l) = self.player_entity(f.leader).filter(|(s, _)| *s == sys).and_then(|(_, id)| self.ent(sys, id)) else { continue };
            for (k, m) in f.members.iter().filter(|m| **m != f.leader).enumerate() {
                if let Some((s, id)) = self.player_entity(*m)
                    && s == sys
                    && self.ent(sys, id).is_some_and(|e| e.controls.hold_post && !e.dead && !e.docked)
                    && let Some(post) = f.formation.post_at(k + 1, l.pos, l.rot, spacing)
                {
                    out.insert(id, (post, l.vel));
                }
            }
        }
        out
    }

    pub fn tick(&mut self, dt: f32, unix: u64) {
        self.now += dt as f64;
        self.unix = unix;
        self.tick += 1;
        let _tick = prof::span("tick");

        let mut lap = std::time::Instant::now();
        let p = prof::span("sessions");
        self.tick_sessions(dt);
        drop(p);
        self.lap(0, &mut lap);
        let sys_ids: Vec<SystemId> = self.systems.keys().copied().collect();
        for sys in sys_ids {
            let _p = prof::span("system");
            self.tick_system(sys, dt);
        }
        lap = std::time::Instant::now();
        let p = prof::span("engagements+dailies");
        self.tick_engagements();
        self.tick_fronts();

        if self.tick % 20 == 0 {
            self.store.decay_supply(0.5f32.powf(1.0 / self.tuning.economy.supply_half_life.max(1.0)));
        }
        if self.tick % 1200 == 0 && !self.store.data.contracts.is_empty() {
            self.expire_contracts();
        }
        if self.tick.is_multiple_of(200) {
            let online: Vec<PlayerId> = self.conn_of.keys().copied().filter(|p| self.store.profile(*p).is_some_and(|x| !x.jobs.is_empty())).collect();
            for pid in online {
                self.expire_jobs(pid);
            }
        }
        if self.tick % (3600 * TICK_HZ as u64) == 0 && !self.store.data.influence.is_empty() {
            self.decay_influence();
        }
        if self.tick.is_multiple_of(3600 * TICK_HZ as u64) {
            self.hourly_betrayal();
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
        drop(p);
        self.lap(6, &mut lap);
        let p = prof::span("snapshots");
        self.send_snapshots();
        drop(p);
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
                    if let Some(e) = self.ent_mut(sys, eid) {
                        e.warping = false;
                    }
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
        // The way out of here and into there: the galaxy map direction from the old system to the new one.
        let (a, b) = (galaxy::system(from).map_pos, galaxy::system(to).map_pos);
        let way = Vec3::new(b[0] - a[0], 0.0, b[2] - a[2]).normalize_or(Vec3::NEG_Z);
        // The ship is gone before the snapshot goes out, so the flash carries where it was.
        if let Some(pos) = self.ent(from, eid).map(|e| e.pos.to_array()) {
            self.fx(from, Fx::WarpOut { entity: eid, pos });
        }
        if let Some(c) = self.conn_of.get(&pid).copied() {
            self.sessions.get_mut(&c).unwrap().warp = None;
        }
        self.spawn_player(pid, to, false);
        if let Some((s, e)) = self.player_entity(pid) {
            let t = self.tuning.combat;
            let layout = &self.systems[&s].layout;
            let (jump, station) = (layout.jump_point, layout.station);
            // Drop out on the side we came from, scattered, sliding in along the way.
            let jitter = shared::rng::Rng::from_parts(&[0x3A2B, self.tick, pid]).in_sphere() * t.warp_arrive_spread;
            let at = jump - way * t.warp_arrive_dist + jitter;
            let ent = self.ent_mut(s, e).unwrap();
            ent.hull = hull.min(ent.stats.max_hull);
            ent.shield = shield.min(ent.stats.max_shield);
            let dir = if t.warp_arrive_speed > 0.0 {
                ent.pos = at;
                ent.vel = way * ent.stats.speed * t.warp_arrive_speed;
                way
            } else {
                // Before arrival tuning: on the warp point, drifting at the station.
                let d = (station - ent.pos).normalize_or_zero();
                ent.vel = d * 60.0;
                d
            };
            if dir != Vec3::ZERO {
                ent.rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
            }
            if t.warp_arrive_speed > 0.0 {
                // Escorts were placed around the warp point: form them up on the flagship's new spot.
                self.despawn_escorts(pid);
                self.spawn_escorts(pid);
            }
            self.fx(s, Fx::WarpIn { entity: e, dir: dir.to_array() });
        }
        let name = galaxy::system(to).name;
        let tier = galaxy::system(to).tier.label();
        self.toast(pid, format!("{name} 도착 — {tier}"));
        if galaxy::is_generated(to) {
            self.chart_system(pid, to);
        }
    }

    /// The first captain to reach a generated system charts it: a discovery for the chronicle and a
    /// reward scaled by the system's risk (tuning `galaxy.ron`).
    fn chart_system(&mut self, pid: PlayerId, sys: SystemId) {
        let Some(prof) = self.store.profile(pid).cloned() else { return };
        let def = galaxy::system(sys);
        let event = DomainEvent::Discovery { player: prof.name.clone(), ship: prof.ship.name.clone(), system: sys, site: def.name.to_string(), kind: "System".into() };
        if self.history.record(self.unix, event).is_none() {
            return;
        }
        let credits = (self.tuning.galaxy.chart_reward as f32 * self.tuning.economy.risk_reward[&def.tier]) as u64;
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits += credits;
        }
        self.toast(pid, format!("미답 성계 {} 최초 도달 — 항로국에 등록되었습니다 (+{credits} CR)", def.name));
        self.push_profile(pid);
        let charted = ServerMsg::Charted { systems: self.history.charted_systems() };
        let conns: Vec<ConnId> = self.conn_of.values().copied().collect();
        for c in conns {
            self.send(c, charted.clone());
        }
    }

    /// The controlling alliance's station batteries (`social.station_guard`): the nearest captain's ship
    /// at war with it inside the range takes steady fire, so an enemy cannot sit off a home station and
    /// sink its captains as they rebuild (war QA). No kill credit — the wreck says "우주의 위험".
    /// The controller's station batteries (`social.station_guard`): they shoot the nearest ship at war
    /// with the holder on the approach, while every such ship wears them down (the siege). Silenced,
    /// they come back after a quiet spell paid by the treasury; meanwhile attackers who outnumber the
    /// defenders on the approach draw influence from the holder each second — how a system is taken by
    /// fighting.
    fn station_guard(&mut self, sys: SystemId, dt: f32) {
        let guard = self.tuning.social.station_guard;
        let Some(owner) = self.controller(sys).filter(|_| guard.dps > 0.0) else {
            self.store.data.batteries.remove(&sys);
            return;
        };
        let station = self.systems[&sys].layout.station;
        // Ships on the approach: (id, distance, alliance, a captain — escorts draw fire but neither wear
        // the batteries down nor hold the approach).
        let near: Vec<(EntityId, f32, Option<AllianceId>, bool)> = self.systems[&sys]
            .ents
            .values()
            .filter(|e| matches!(e.kind, EntityKind::Player | EntityKind::Escort) && !e.dead && !e.docked && e.pos.distance(station) <= guard.range + e.stats.radius)
            .map(|e| (e.id, e.pos.distance(station), e.alliance, e.kind == EntityKind::Player))
            .collect();
        let attackers: Vec<&(EntityId, f32, Option<AllianceId>, bool)> = near.iter().filter(|(_, _, a, _)| self.at_war(Some(owner), *a)).collect();
        let captains = attackers.iter().filter(|x| x.3).count();
        let fresh = crate::store::Battery { owner, ..Default::default() };
        let mut b = self.battery(sys).unwrap_or_else(|| fresh.clone());
        if attackers.is_empty() && b.damage == 0.0 && !b.silenced {
            return;
        }
        let k = guard.level_k(b.level);
        let before = b.clone();
        if b.silenced {
            if attackers.is_empty() {
                b.quiet += dt;
                let treasury = self.alliance(owner).map_or(0, |a| a.treasury);
                if b.quiet >= guard.rebuild_secs && treasury >= guard.rebuild_cost {
                    if let Some(a) = self.store.data.alliances.iter_mut().find(|a| a.id == owner) {
                        a.treasury -= guard.rebuild_cost;
                    }
                    b.silenced = false;
                    b.damage = 0.0;
                    let text = format!("{} 정거장 포대 재가동 (금고 −{} CR)", galaxy::system(sys).name, guard.rebuild_cost);
                    for m in self.alliance(owner).map(|a| a.members.clone()).unwrap_or_default() {
                        self.toast(m, text.clone());
                    }
                }
            } else {
                b.quiet = 0.0;
                // The strongest attacking alliance on the approach, if it outnumbers the defenders.
                let defenders = near.iter().filter(|(_, _, a, c)| *c && *a == Some(owner)).count();
                let mut count: BTreeMap<AllianceId, usize> = BTreeMap::new();
                for (_, _, a, c) in &attackers {
                    if *c && let Some(a) = a {
                        *count.entry(*a).or_default() += 1;
                    }
                }
                if let Some((&att, &n)) = count.iter().max_by(|x, y| x.1.cmp(y.1).then(y.0.cmp(x.0)))
                    && n > defenders
                {
                    self.store.data.batteries.insert(sys, b);
                    // Once a second (a tick's worth is below the 0.1 that influence keeps).
                    if self.tick.is_multiple_of(TICK_HZ as u64) {
                        self.shift_influence(owner, sys, -guard.siege_drain);
                        self.shift_influence(att, sys, guard.siege_gain);
                    }
                    return;
                }
            }
        } else {
            if guard.hp > 0.0 {
                b.damage = if attackers.is_empty() { (b.damage - guard.siege_dps * dt).max(0.0) } else { b.damage + guard.siege_dps * captains as f32 * dt };
            }
            if let Some(&&(id, ..)) = attackers.iter().min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))) {
                self.battery_fire(sys, id, guard.dps * k * dt);
            }
            if guard.hp > 0.0 && b.damage >= guard.hp * k {
                b.silenced = true;
                b.quiet = 0.0;
                let name = |g: &Game, a: Option<AllianceId>| a.and_then(|a| g.alliance(a)).map(|a| format!("[{}] {}", a.tag, a.name)).unwrap_or_default();
                let by = attackers.iter().filter_map(|x| x.2).min();
                let (holder, by) = (name(self, Some(owner)), name(self, by));
                self.history.record(self.unix, history::DomainEvent::StationSilenced { system: sys, holder: holder.clone(), by: by.clone() });
                let text = format!("⚔ {by}이(가) {}의 {holder} 정거장 포대를 침묵시켰습니다", galaxy::system(sys).name);
                let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
                for p in online {
                    self.toast(p, text.clone());
                }
            }
        }
        if b != before {
            self.store.dirty = true;
            if b == fresh {
                self.store.data.batteries.remove(&sys);
            } else {
                self.store.data.batteries.insert(sys, b);
            }
            // Sieges move by the second; standings go out when the batteries change state.
            if before.silenced != self.battery(sys).is_some_and(|x| x.silenced) || (before.damage == 0.0) != (self.battery(sys).map_or(0.0, |x| x.damage) == 0.0) {
                let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
                for p in online {
                    self.send_influence(p);
                }
            }
        }
    }

    /// A battery hit: shield bleed, then hull; no kill credit ("우주의 위험").
    fn battery_fire(&mut self, sys: SystemId, id: EntityId, dmg: f32) {
        let (bleed, now, tick) = (self.tuning.combat.shield_bleed, self.now, self.tick);
        let t = self.ent_mut(sys, id).unwrap();
        let absorbed = (dmg * (1.0 - bleed)).min(t.shield);
        t.shield -= absorbed;
        t.hull -= dmg - absorbed;
        t.last_hit = now;
        let pos = t.pos;
        // A hit spark on the target every half second (the batteries are part of the station model).
        if tick.is_multiple_of(10) {
            self.fx(sys, Fx::Explosion { pos: pos.to_array(), size: 0.3 });
        }
    }

    fn tick_system(&mut self, sys: SystemId, dt: f32) {
        let has_players = self.sessions.values().any(|s| s.system == sys && s.player.is_some());
        let tier = galaxy::system(sys).tier;
        // Pirate population.
        let amb = self.tuning.npcs.ambient;
        let wave_here = amb.leave_waves && self.sessions.values().any(|s| s.wave_system == Some(sys) && !s.wave.is_empty());
        if tier != RiskTier::Core && has_players && !wave_here {
            let s = self.systems.get_mut(&sys).unwrap();
            s.pirate_cd -= dt;
            // Only free-roaming pirates count (not scenario waves, and not players' escorts, which also
            // carry an AI — counting them used to stop pirates spawning around big fleets).
            let pop = s.ents.values().filter(|e| matches!(e.kind, EntityKind::Npc(_)) && !e.dead && e.ai.as_ref().is_some_and(|a| a.scenario_owner.is_none())).count();
            let target = (amb.base + amb.per_level * tier.pirate_level() as u32) as usize;
            if s.pirate_cd <= 0.0 && pop < target {
                s.pirate_cd = amb.interval;
                self.spawn_pirate_group(sys, tier);
            }
        }

        let ids: Vec<EntityId> = self.systems[&sys].ents.keys().copied().collect();
        let mut shots: Vec<Shot> = Vec::new();
        let star_r = self.systems[&sys].layout.star_radius;

        let mut lap = std::time::Instant::now();
        let p = prof::span("ai");
        // AI decisions first (needs a read-only view).
        if has_players {
            for &id in &ids {
                self.think(sys, id, dt);
            }
        }

        drop(p);
        let p = prof::span("flight");
        let roll_rate = self.tuning.combat.roll_rate;
        let (boost_k, strafe, boost_power) = (self.tuning.combat.boost, self.tuning.combat.strafe, self.tuning.combat.boost_power);
        let posts = self.held_posts(sys);
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
            let mut c = e.controls;
            // Holding a formation post: thrust and strafe toward it (full speed beyond 300 u, easing in).
            let mut carry = Vec3::ZERO;
            if let Some(&(post, leader_vel)) = posts.get(&id) {
                let err = post - e.pos;
                let l = e.rot.inverse() * err.normalize_or_zero() * (err.length() / 300.0).min(1.0);
                (c.thrust, c.strafe_x, c.strafe_y) = ((-l.z).clamp(-1.0, 1.0), (l.x / strafe).clamp(-1.0, 1.0), (l.y / strafe).clamp(-1.0, 1.0));
                carry = leader_vel;
            }
            let rate = e.stats.turn * dt;
            e.rot = (e.rot * Quat::from_rotation_y(-c.yaw * rate) * Quat::from_rotation_x(c.pitch * rate) * Quat::from_rotation_z(-c.roll * rate * roll_rate)).normalize();
            let (fwd, right, up) = (e.rot * Vec3::NEG_Z, e.rot * Vec3::X, e.rot * Vec3::Y);
            let boost = if c.boost { boost_k } else { 1.0 };
            let desired = (fwd * c.thrust + right * c.strafe_x * strafe + up * c.strafe_y * strafe) * e.stats.speed * boost + carry;
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
            // Shield regeneration after a quiet period (not while boosting, `combat.boost_power`).
            if now - e.last_hit > 4.0 && !(boost_power && c.boost) {
                e.shield = (e.shield + e.stats.shield_regen * dt).min(e.stats.max_shield);
            }
            if e.hull <= 0.0 && !e.dead {
                e.hull = 0.0;
            }
        }

        drop(p);
        self.lap(1, &mut lap);
        // Weapons: forward guns, turret batteries, bow main gun.
        let p = prof::span("grid");
        let sm = self.systems.get_mut(&sys).unwrap();
        sm.grid.rebuild(sm.ents.values());
        drop(p);
        let p = prof::span("weapons");
        for &id in &ids {
            self.fire_forward_guns(sys, id, &mut shots);
            self.fire_turrets(sys, id, dt, &mut shots);
            self.fire_main_gun(sys, id, dt, &mut shots);
        }

        drop(p);
        self.lap(2, &mut lap);
        // Missiles.
        let p = prof::span("missiles");
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

        drop(p);
        let p = prof::span("damage");
        let war = galaxy::system(sys).tier == RiskTier::WarZone;
        for s in shots {
            // Captains against captains (escorts included) in a war zone fight longer (combat.ron war_pvp_damage).
            let pvp = |id| self.ent(sys, id).is_some_and(|e| matches!(e.kind, EntityKind::Player | EntityKind::Escort));
            let k = if war && pvp(s.from) && pvp(s.to) { self.tuning.combat.war_pvp_damage } else { 1.0 };
            self.apply_damage(sys, s.from, s.to, s.dmg * k);
        }
        drop(p);

        self.lap(3, &mut lap);
        // Mining.
        let p = prof::span("mining");
        for &id in &ids {
            self.tick_mining(sys, id, dt);
        }
        drop(p);
        self.station_guard(sys, dt);

        self.lap(4, &mut lap);
        // Deaths.
        let _p = prof::span("deaths");
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
            let (xz, y) = (self.tuning.npcs.tactics.spawn_xz, self.tuning.npcs.tactics.spawn_y);
            let pos = anchor + Vec3::new(r.range(-xz, xz), r.range(-y, y) + 60.0, r.range(-xz, xz)) + Vec3::X * i as f32 * 40.0;
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
            // Captains fighting their chapter's wave here, whom roaming pirates leave alone.
            let busy: Vec<PlayerId> = if ai.scenario_owner.is_none() && self.tuning.npcs.ambient.leave_waves { self.sessions.values().filter(|s| s.wave_system == Some(sys) && !s.wave.is_empty()).filter_map(|s| s.player).collect() } else { vec![] };
            let mut best: Option<(EntityId, f32)> = None;
            let (station, leash) = (self.systems[&sys].layout.station, self.tuning.npcs.ambient.station_leash);
            for (tid, t) in &self.systems[&sys].ents {
                if t.kind != EntityKind::Player || t.dead || t.docked {
                    continue;
                }
                let d = t.pos.distance(e.pos);
                let is_owner_side = ai.scenario_owner.is_some() && (t.owner == ai.scenario_owner || (fleet_of_owner.is_some() && t.fleet == fleet_of_owner));
                let attacked_me = e.attackers.iter().any(|(a, _, time)| a == tid && self.now - time < 20.0);
                let ok = if ai.scenario_owner.is_some() { is_owner_side || attacked_me } else { (d < aggro && !t.owner.is_some_and(|o| busy.contains(&o)) && t.pos.distance(station) >= leash) || attacked_me };
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
                let tac = self.tuning.npcs.tactics;
                if d > range * 0.7 {
                    controls.thrust = 1.0;
                    // Dive in from above or below rather than head-on in the plane.
                    tp + tv * 0.6 + Vec3::Y * dive_side(id) * d * tac.dive_slope
                } else {
                    // Strafe around the target while keeping guns on it, in a tilted, turning plane.
                    controls.thrust = 0.25;
                    (controls.strafe_x, controls.strafe_y) = orbit_strafe(id, ai.orbit_sign, self.now, &tac);
                    tp + tv * 0.3
                }
            }
            None => {
                let home_d = pos.distance(ai.home);
                if home_d > 300.0 {
                    controls.thrust = 0.6;
                }
                let (s, c) = (self.now as f32 * 0.1 + id as f32).sin_cos();
                ai.home + Vec3::new(s * 200.0, c * 200.0 * self.tuning.npcs.tactics.wander_tilt, c * 200.0)
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
        let rate = e.stats.mine_rate * if prof.profession == Profession::Miner { self.tuning.economy.miner_bonus } else { 1.0 };
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
            self.find_specimens(pid, sys, &a, n);
        }
    }

    /// Rare deposits sometimes give up a specimen of their exact mineral (tuning `minerals.ron`); the
    /// first specimen of a mineral anywhere is a discovery for the chronicle.
    fn find_specimens(&mut self, pid: PlayerId, sys: SystemId, a: &galaxy::Asteroid, units: u32) {
        let t = self.tuning.clone();
        let t = &t.minerals;
        let Some(m) = shared::minerals::mineral_in(t, a.mineral) else { return };
        let chance = t.specimen_chance.get(&m.rarity).copied().unwrap_or(0.0);
        if chance <= 0.0 {
            return;
        }
        let mut r = shared::rng::Rng::from_parts(&[0x5BEC_1E, self.tick, pid as u64, a.id as u64]);
        let found = (0..units).filter(|_| r.f32() < chance).count() as u32;
        if found == 0 {
            return;
        }
        let rep = t.discovery_reputation.get(&m.rarity).copied().unwrap_or(0);
        let value = m.specimen_value(t);
        let Some(prof) = self.store.profile_mut(pid) else { return };
        *prof.specimens.entry(m.id).or_default() += found;
        let (name, ship) = (prof.name.clone(), prof.ship.name.clone());
        let event = DomainEvent::MineralDiscovered { player: name, ship, system: sys, mineral: m.id, name: m.name.clone(), rarity: m.rarity.label().into(), legendary: m.rarity == shared::minerals::Rarity::Legendary };
        if self.history.record(self.unix, event).is_some() {
            if let Some(p) = self.store.profile_mut(pid) {
                p.reputation += rep;
            }
            self.toast(pid, format!("신광물 발견! {} — 광물학회에 발견자로 등록되었습니다 (평판 +{rep}, 표본 시세 {value} CR)", m.label()));
        } else {
            self.toast(pid, format!("표본 확보: {} ×{found} (정거장 시세 {value} CR)", m.label()));
        }
        self.push_profile(pid);
    }

    pub fn apply_damage(&mut self, sys: SystemId, from: EntityId, to: EntityId, dmg: f32) {
        if !self.can_damage(sys, from, to) {
            return;
        }
        let now = self.now;
        let combat = self.tuning.combat;
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
            // No ship is invulnerable: part of every hit bleeds through shields, and every hit costs some hull.
            let absorbed = (dmg * (1.0 - combat.shield_bleed)).min(t.shield);
            let hull_dmg = if dmg > 0.0 { (dmg - absorbed).max(combat.min_hull_damage) } else { 0.0 };
            t.shield -= absorbed;
            t.hull -= hull_dmg;
            if now - t.last_hit > 30.0 {
                t.fight_start = now;
            }
            t.last_hit = now;
            // One entry per attacker (latest hit last), so a ship under fire from hundreds stays cheap.
            t.attackers.retain(|(a, _, time)| *a != from && now - time < 30.0);
            t.attackers.push((from, attacker_pid, now));
            let to_side = t.side();
            if let Some(m) = self.metrics.as_mut() {
                let elev = dir.y.clamp(-1.0, 1.0).asin().to_degrees();
                m.push(crate::metrics::Metric::Hit { t: now, sys, from, from_side: attacker_pid, to, to_side, shield: absorbed, hull: hull_dmg, elev });
            }
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
        // Turning on a contract partner is a betrayal (the contract breaks before anything else).
        if !self.store.data.contracts.is_empty() {
            self.contract_hit(attacker_pid, victim_pid, sys);
        }
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
        self.front_loss(sys, &e);
        let now = self.now;
        let killer = e.attackers.iter().rev().find(|(_, p, t)| p.is_some() && now - t < 30.0).map(|(a, p, _)| (*a, p.unwrap()));
        let killer_ent_name = e.attackers.last().and_then(|(a, _, _)| self.ent(sys, *a)).map(|k| k.name.clone());
        let tier = galaxy::system(sys).tier;
        if let Some(m) = self.metrics.as_mut() {
            m.push(crate::metrics::Metric::Kill { t: now, sys, victim: id, victim_name: e.name.clone(), victim_side: e.side(), killer: killer.map(|k| k.1), ttk: (now - e.fight_start) as f32 });
        }
        match e.kind {
            EntityKind::Npc(arch) => {
                let level = e.ai.as_ref().map(|a| a.level).unwrap_or(0);
                if let Some((_, kpid)) = killer {
                    let mut reward = arch.bounty(level);
                    let mult = tier.bounty_mult() * if self.store.profile(kpid).is_some_and(|p| p.profession == Profession::CombatPilot) { 1.3 } else { 1.0 };
                    reward.credits = (reward.credits as f32 * mult * self.home_turf(kpid, sys)) as u64;
                    // A job's pirates: the job pays, the kill only a share.
                    if let Some(owner) = e.ai.as_ref().and_then(|a| a.scenario_owner)
                        && self.in_job_wave(owner, id)
                    {
                        let share = self.tuning.trade.jobs.wave_bounty;
                        reward.credits = (reward.credits as f32 * share).round() as u64;
                    }
                    if self.player_entity(kpid).and_then(|(s, k)| self.ent(s, k)).is_some_and(|k| k.mining.is_some()) {
                        let share = self.tuning.economy.mining_bounty;
                        let cut = |v: u32| (v as f32 * share).round() as u32;
                        reward.credits = (reward.credits as f32 * share).round() as u64;
                        (reward.alloy, reward.crystal, reward.datacore, reward.relic) = (cut(reward.alloy), cut(reward.crystal), cut(reward.datacore), cut(reward.relic));
                    }
                    let group = self.credit_group(kpid, sys);
                    for (i, pid) in group.iter().enumerate() {
                        let mut r = reward.clone();
                        if i > 0 {
                            r.credits /= 2;
                            r.alloy /= 2;
                            r.crystal /= 2;
                            r.datacore /= 2;
                        }
                        r.credits = self.levy(*pid, sys, r.credits);
                        if let Some(pm) = self.store.profile_mut(*pid) {
                            pm.wallet.add(&r);
                            if i == 0 {
                                pm.kills += 1;
                            }
                        }
                        if let Some(m) = self.metrics.as_mut() {
                            m.push(crate::metrics::Metric::Reward { t: now, player: *pid, credits: r.credits });
                        }
                        if i == 0 {
                            self.gain_influence(*pid, sys, self.tuning.social.influence_pirate);
                        }
                        self.toast(*pid, format!("{} 격파: {}", e.name, r.describe()));
                        self.progress(*pid, Progress::KilledPirate);
                    }
                    if matches!(arch, NpcArchetype::Warlord | NpcArchetype::Sentinel) {
                        let killers: Vec<String> = e.attackers.iter().filter_map(|(_, p, _)| *p).collect::<HashSet<_>>().into_iter().filter_map(|p| self.store.profile(p).map(|x| x.name.clone())).collect();
                        self.history.record(self.unix, DomainEvent::BossDestroyed { boss: arch.name().into(), system: sys, killers });
                    }
                }
                if let Some(owner) = e.ai.as_ref().and_then(|a| a.scenario_owner)
                    && !self.job_wave_kill(owner, id)
                {
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
                if let Some((_, kp)) = killer
                    && kp != vpid
                {
                    self.contract_kill(kp, &vprof.name, sys);
                    self.seize_intel(kp, vpid, sys);
                }
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
                    artifacts: vec![],
                });
                let idx = self.store.data.wrecks.len() - 1;
                self.sink_artifacts(vpid, sys, idx, ev);
                self.lose_survey(vpid);
                let wid = self.spawn_wreck_entity(sys, e.pos, format!("{}의 잔해 「{}」", vprof.name, vprof.ship.name), idx);
                if let Some(pm) = self.store.profile_mut(vpid) {
                    pm.cargo.clear();
                    pm.deaths += 1;
                }
                // Repeated losses wait longer before the rebuild (`combat.rebuild`).
                let rb = self.tuning.combat.rebuild;
                let now = self.now;
                let recent = self.recent_deaths.entry(vpid).or_default();
                recent.retain(|t| now - t < rb.window_secs as f64);
                recent.push(now);
                let rebuild_in = rb.wait(recent.len());
                if let Some(c) = self.conn_of.get(&vpid).copied() {
                    let s = self.sessions.get_mut(&c).unwrap();
                    s.entity = None;
                    s.dead = true;
                    s.rebuild_at = now + rebuild_in as f64;
                    s.warp = None;
                    s.scan = None;
                    let lost_wave = !s.wave.is_empty();
                    if lost_wave {
                        s.wave_losses += 1;
                    }
                    let hint = if lost_wave { self.wave_hint(c) } else { None };
                    self.clear_wave(c);
                    self.send(c, ServerMsg::Died { killer: killer_name.clone(), wreck: wid, hint, rebuild_in });
                }
                self.jobs_on_death(vpid);
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
            self.end_fronts(sys);
            if eng.sides.len() < 2 {
                continue;
            }
            let total_losses: u32 = eng.sides.values().map(|s| s.2).sum();
            let min_losses = eng.sides.values().map(|s| s.2).min().unwrap_or(0);
            let winners: Vec<&String> = eng.sides.iter().filter(|(_, s)| s.2 == min_losses).map(|(k, _)| k).collect();
            let winner_key = if total_losses > 0 && winners.len() == 1 { Some(winners[0].clone()) } else { None };
            let sides: Vec<Side> = eng.sides.values().map(|(n, m, l)| Side { name: n.clone(), members: m.iter().cloned().collect(), losses: *l }).collect();
            let winner_name = winner_key.as_ref().map(|k| eng.sides[k].0.clone());
            if let Some(k) = &winner_key {
                let keys: Vec<String> = eng.sides.keys().cloned().collect();
                self.battle_influence(sys, k, &keys);
            }
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

/// Which way a ship dives at its target: half from above, half from below.
pub(super) fn dive_side(id: EntityId) -> f32 {
    if id % 2 == 0 { 1.0 } else { -1.0 }
}

/// Orbit strafe (local x, local y) for a ship circling its target: tilted out of its horizontal by a
/// per-ship angle in `orbit_tilt` that turns at `orbit_spin` (tuning `npcs.ron` tactics).
pub(super) fn orbit_strafe(id: EntityId, sign: f32, now: f64, tac: &shared::tuning::Tactics) -> (f32, f32) {
    let k = shared::rng::Rng::from_parts(&[0x0B17, id as u64]).f32();
    let tilt = (tac.orbit_tilt.0 + (tac.orbit_tilt.1 - tac.orbit_tilt.0) * k).to_radians() + now as f32 * tac.orbit_spin;
    (sign * tilt.cos(), sign * tilt.sin())
}
