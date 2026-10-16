//! Player intents. Every action is validated against authoritative state.

use super::*;
use history::DomainEvent;
use shared::galaxy::{Ore, PoiKind, DOCK_RANGE, MINE_RANGE};
use shared::missions::{Step, scenarios};
use shared::ships::{Module, tier_cap, upgrade_cost, MAX_MODULE_LEVEL};

impl Game {
    pub(super) fn handle_player(&mut self, conn: ConnId, pid: PlayerId, msg: ClientMsg) {
        let (sys, eid) = {
            let s = &self.sessions[&conn];
            (s.system, s.entity)
        };
        let dead = self.sessions[&conn].dead;
        match msg {
            ClientMsg::Controls(c) => {
                if let Some(e) = eid.and_then(|e| self.ent_mut(sys, e)) {
                    let clamp = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
                    e.controls = Controls { thrust: clamp(c.thrust), strafe_x: clamp(c.strafe_x), strafe_y: clamp(c.strafe_y), yaw: clamp(c.yaw), pitch: clamp(c.pitch), roll: clamp(c.roll), boost: c.boost, fire: c.fire };
                }
            }
            ClientMsg::SetTarget(t) => {
                if let Some(e) = eid.and_then(|e| self.ent_mut(sys, e)) {
                    e.target = t;
                }
            }
            ClientMsg::FireMissile => self.fire_missile(pid, sys, eid),
            ClientMsg::Warp { system } => self.start_warp(conn, pid, sys, eid, system),
            ClientMsg::Dock => self.dock(pid, sys, eid),
            ClientMsg::Undock => {
                if let Some(e) = eid.and_then(|e| self.ent_mut(sys, e))
                    && e.docked
                {
                    e.docked = false;
                    e.vel = e.forward() * 30.0;
                    let r = e.stats.radius;
                    e.pos += e.forward() * (80.0 + r);
                    self.spawn_escorts(pid);
                }
            }
            ClientMsg::EscortCommand { attack } => {
                self.sessions.get_mut(&conn).unwrap().escort_attack = attack;
                self.toast(pid, if attack { "호위함: 목표를 공격하라!" } else { "호위함: 기함 호위로 복귀" });
            }
            ClientMsg::AllianceCreate { name, tag } => self.alliance_create(pid, name, tag),
            ClientMsg::AllianceInvite { name } => self.alliance_invite(pid, &name),
            ClientMsg::AllianceAccept { alliance } => self.alliance_accept(conn, pid, alliance),
            ClientMsg::AllianceLeave => self.alliance_leave(pid),
            ClientMsg::AllianceDeclareWar { target } => self.alliance_war(pid, target, true),
            ClientMsg::AlliancePeace { target } => self.alliance_war(pid, target, false),
            ClientMsg::RequestAlliances => self.send_alliances(pid),
            ClientMsg::AppraiseArtifact { id } => self.artifact_service(conn, pid, sys, eid, id, super::artifacts::ArtifactAction::Appraise),
            ClientMsg::SellArtifact { id } => self.artifact_service(conn, pid, sys, eid, id, super::artifacts::ArtifactAction::Sell),
            ClientMsg::DonateArtifact { id } => self.artifact_service(conn, pid, sys, eid, id, super::artifacts::ArtifactAction::Donate),
            ClientMsg::Mine { asteroid } => self.mine(pid, sys, eid, asteroid),
            ClientMsg::Scan { poi } => self.start_scan(conn, pid, sys, eid, poi),
            ClientMsg::InvestigateWreck { wreck } => self.investigate(pid, sys, eid, wreck),
            ClientMsg::Upgrade { module } => self.upgrade(pid, sys, eid, module),
            ClientMsg::Sell { ore, units } => self.sell(pid, sys, eid, ore, units),
            ClientMsg::Refine { ore, units } => self.refine(pid, sys, eid, ore, units),
            ClientMsg::ClaimDaily { index } => self.claim_daily(pid, index),
            ClientMsg::StartScenario { id } => self.start_scenario(pid, id),
            ClientMsg::AbandonScenario => {
                self.clear_wave(conn);
                if let Some(p) = self.store.profile_mut(pid) {
                    p.scenario = None;
                }
                self.push_profile(pid);
            }
            ClientMsg::FleetInvite { name } => self.fleet_invite(pid, &name),
            ClientMsg::FleetAccept { fleet } => self.fleet_accept(conn, pid, fleet),
            ClientMsg::FleetLeave => self.fleet_leave(pid),
            ClientMsg::Respawn => {
                if dead {
                    self.respawn(pid);
                }
            }
            ClientMsg::Chat { text } => {
                let text: String = text.chars().take(200).collect();
                if text.trim().is_empty() {
                    return;
                }
                if self.dev_mode && text.starts_with('/') {
                    return self.dev_command(pid, &text);
                }
                let from = self.store.profile(pid).unwrap().name.clone();
                let conns: Vec<ConnId> = self.conn_of.values().copied().collect();
                for c in conns {
                    self.send(c, ServerMsg::Chat { from: from.clone(), text: text.clone() });
                }
            }
            ClientMsg::RequestChronicle => self.send_chronicle(conn, pid),
            ClientMsg::RequestBiography { name } => {
                let ship_bio = self.store.find_by_name(&name).and_then(|id| self.store.profile(id)).map(|p| self.history.ship_biography(&p.name, &p.ship.name)).unwrap_or_default();
                let character = self.history.biography(&name);
                self.send(conn, ServerMsg::Biography { name, character, ship: ship_bio });
            }
            ClientMsg::PublishResearch { event, thesis, cite } => self.publish_research(conn, pid, event, thesis, cite),
            ClientMsg::ForgeEvidence { evidence, text } => self.forge_evidence(conn, pid, evidence, text),
            ClientMsg::AuthenticateEvidence { evidence } => self.authenticate_evidence(conn, pid, evidence),
            ClientMsg::FileStory { event, headline, body, cite } => self.file_story(conn, pid, sys, event, headline, body, cite),
            ClientMsg::DisputeResearch { event, author } => {
                let me = self.store.profile(pid).unwrap();
                let name = me.name.clone();
                // Disputing requires holding evidence about the event yourself.
                let holds = me.archive.iter().any(|id| self.history.evidence.iter().any(|e| e.id == *id && e.event == event));
                if !holds {
                    return self.error(conn, "반박하려면 해당 사건의 증거를 직접 확보해야 합니다 (잔해 조사)");
                }
                if self.history.record(self.unix, DomainEvent::ResearchDisputed { disputer: name, author, event }).is_some() {
                    if let Some(p) = self.store.profile_mut(pid) {
                        p.reputation += 2;
                    }
                    self.toast(pid, "반박문이 연대기에 기록되었습니다");
                    self.send_chronicle(conn, pid);
                } else {
                    self.error(conn, "반박할 수 없습니다");
                }
            }
            ClientMsg::Hello { .. } | ClientMsg::CreateCharacter { .. } | ClientMsg::Ping { .. } => {}
        }
    }

    pub fn send_chronicle(&mut self, conn: ConnId, pid: PlayerId) {
        let events = self.history.chronicle(shared::chronicle::Importance::Notable, 60);
        let archive = self.store.profile(pid).map(|p| p.archive.clone()).unwrap_or_default();
        let evidence = self.history.visible_evidence(&events, &archive);
        let herald = self.history.herald(8);
        self.pay_for_readers(pid, &herald);
        self.send(conn, ServerMsg::Chronicle { events, evidence, herald });
    }

    /// A captain read these Herald pages: each article's reporter earns once per distinct reader.
    fn pay_for_readers(&mut self, reader: PlayerId, herald: &[shared::chronicle::Headline]) {
        let t = self.tuning.history;
        let me = self.store.profile(reader).map(|p| p.name.clone()).unwrap_or_default();
        for h in herald {
            let (Some(story), Some(by)) = (h.story, h.by.as_ref()) else { continue };
            if *by == me || self.history.stories.iter().any(|s| s.id == story && s.retracted) {
                continue;
            }
            let readers = self.store.data.story_readers.entry(story).or_default();
            if readers.contains(&reader) || readers.len() >= t.story_reader_cap as usize {
                continue;
            }
            readers.push(reader);
            if let Some(rp) = self.store.find_by_name(by)
                && let Some(p) = self.store.profile_mut(rp)
            {
                p.wallet.credits += t.story_per_reader;
            }
        }
    }

    /// Journalists file a Herald article about an event: from the scene (in its system) or holding
    /// its evidence. One article per event per journalist.
    #[allow(clippy::too_many_arguments)]
    fn file_story(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, event: shared::chronicle::EventId, headline: String, body: String, cite: Vec<u64>) {
        let prof = self.store.profile(pid).unwrap().clone();
        if prof.profession != Profession::Journalist {
            return self.error(conn, "기사는 기자(Journalist)만 송고할 수 있습니다");
        }
        let Some(ev) = self.history.event(event).cloned() else { return self.error(conn, "없는 사건입니다") };
        if self.history.stories.iter().any(|s| s.event == event && s.reporter == prof.name) {
            return self.error(conn, "이 사건에 대해서는 이미 기사를 송고했습니다");
        }
        let holds = prof.archive.iter().any(|id| self.history.evidence.iter().any(|e| e.id == *id && e.event == event));
        let on_scene = self.sessions.get(&conn).is_some_and(|s| s.entity.is_some()) && sys == ev.system;
        if !holds && !on_scene {
            return self.error(conn, format!("취재가 필요합니다: {} 성계 현장에 있거나 사건의 증거를 확보하세요", galaxy::system(ev.system).name));
        }
        let headline: String = headline.trim().chars().take(60).collect();
        let body: String = body.trim().chars().take(400).collect();
        if headline.chars().count() < 4 || body.chars().count() < 10 {
            return self.error(conn, "제목(4자 이상)과 본문(10자 이상)을 쓰세요");
        }
        let cite: Vec<u64> = cite.into_iter().filter(|c| prof.archive.contains(c)).collect();
        self.history.record(self.unix, DomainEvent::StoryFiled { reporter: prof.name.clone(), event, headline: headline.clone(), body, cited: cite });
        let fee = self.tuning.history.story_fee;
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits += fee;
        }
        self.toast(pid, format!("기사 「{headline}」이(가) Herald에 실렸습니다 (+{fee} CR, 독자 한 명당 +{} CR)", self.tuning.history.story_per_reader));
        self.push_profile(pid);
        self.send_chronicle(conn, pid);
    }

    fn fire_missile(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>) {
        let Some(eid) = eid else { return };
        let Some(e) = self.ent(sys, eid) else { return };
        if e.docked || e.missile_cd > 0.0 {
            return;
        }
        let Some(tid) = e.target else {
            return self.toast(pid, "미사일: 목표를 먼저 지정하세요 (T)");
        };
        let Some(t) = self.ent(sys, tid) else { return };
        if !t.is_ship() || t.pos.distance(e.pos) > e.stats.weapon_range * 2.0 {
            return self.toast(pid, "미사일: 사거리 밖입니다");
        }
        if !self.can_damage(sys, eid, tid) {
            return self.toast(pid, "이 대상은 공격할 수 없습니다 (안전 구역/아군)");
        }
        let (pos, fwd, vel, dmg, cd) = (e.pos, e.forward(), e.vel, e.stats.missile_damage, e.stats.missile_cooldown);
        let prof_bonus = if self.store.profile(pid).map(|p| p.profession) == Some(Profession::CombatPilot) { 1.08 } else { 1.0 };
        self.ent_mut(sys, eid).unwrap().missile_cd = cd;
        let mid = self.alloc_entity();
        let mut stats = self.ent(sys, eid).unwrap().stats;
        stats.radius = 2.0;
        stats.speed = 300.0;
        stats.turn = 3.2;
        stats.turrets = 0;
        let radius = self.ent(sys, eid).unwrap().stats.radius;
        let rot = self.ent(sys, eid).unwrap().rot;
        let mut m = Ent::base(mid, EntityKind::Missile, "Missile".into(), stats, pos + fwd * (radius + 10.0), rot);
        m.owner = Some(pid);
        m.vel = vel + fwd * 120.0;
        m.hull = 1.0;
        m.shield = 0.0;
        m.target = Some(tid);
        m.missile = Some(Missile { owner: eid, target: tid, damage: dmg * prof_bonus, life: 7.0 });
        self.systems.get_mut(&sys).unwrap().ents.insert(mid, m);
        self.fx(sys, Fx::MissileLaunch { from: eid, missile: mid });
    }

    fn start_warp(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, target: SystemId) {
        let Some(e) = eid.and_then(|e| self.ent(sys, e)) else { return };
        if target as usize >= galaxy::SYSTEMS.len() || target == sys {
            return self.error(conn, "잘못된 워프 목적지");
        }
        if e.docked {
            return self.error(conn, "도킹 중에는 워프할 수 없습니다");
        }
        if self.now - e.last_hit < COMBAT_LOCK {
            return self.error(conn, "교전 중 — 워프 드라이브가 잠겼습니다");
        }
        let dist = galaxy::distance_ly(sys, target);
        if dist > e.stats.warp_range + 1e-3 {
            return self.error(conn, format!("워프 거리 부족: {:.2} ly 필요, 현재 {:.2} ly (워프 코어 업그레이드 필요)", dist, e.stats.warp_range));
        }
        self.sessions.get_mut(&conn).unwrap().warp = Some((target, WARP_CHARGE));
        self.send(conn, ServerMsg::WarpCharging { system: target, seconds: WARP_CHARGE });
        let _ = pid;
    }

    fn dock(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>) {
        let station = self.systems[&sys].layout.station;
        let now = self.now;
        let Some(e) = eid.and_then(|e| self.ent_mut(sys, e)) else { return };
        if e.pos.distance(station) > DOCK_RANGE + 80.0 + e.stats.radius {
            return self.toast(pid, "정거장에 더 가까이 접근하세요");
        }
        if now - e.last_hit < COMBAT_LOCK {
            return self.toast(pid, "교전 중에는 도킹할 수 없습니다");
        }
        e.docked = true;
        e.vel = Vec3::ZERO;
        e.mining = None;
        e.hull = e.stats.max_hull;
        e.shield = e.stats.max_shield;
        self.despawn_escorts(pid);
        self.toast(pid, "도킹 완료 — 수리 및 재충전되었습니다");
    }

    fn mine(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, asteroid: Option<u32>) {
        let Some(eid) = eid else { return };
        let Some(aid) = asteroid else {
            if let Some(e) = self.ent_mut(sys, eid) {
                e.mining = None;
            }
            return;
        };
        let Some(a) = self.systems[&sys].layout.asteroids.iter().find(|a| a.id == aid).cloned() else { return };
        let Some(e) = self.ent_mut(sys, eid) else { return };
        if e.docked {
            return;
        }
        if e.pos.distance(a.pos) > MINE_RANGE + a.radius + e.stats.radius {
            return self.toast(pid, "채굴 레이저 사거리 밖입니다");
        }
        e.mining = Some(aid);
    }

    fn start_scan(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, poi: u32) {
        let Some(e) = eid.and_then(|e| self.ent(sys, e)) else { return };
        let Some(p) = self.systems[&sys].layout.pois.iter().find(|p| p.id == poi).cloned() else { return };
        if e.pos.distance(p.pos) > e.stats.scan_range {
            return self.error(conn, format!("스캔 범위 밖 ({:.0}u 이내로 접근)", e.stats.scan_range));
        }
        let scenario_needs = self.scenario_step(pid).is_some_and(|s| matches!(s, Step::ScanPoi { system, kind, .. } if system == sys && kind == p.kind));
        let already = self.store.profile(pid).unwrap().scanned.contains(&(sys, poi));
        if already && !scenario_needs {
            return self.error(conn, "오늘 이미 스캔한 지점입니다");
        }
        self.sessions.get_mut(&conn).unwrap().scan = Some((poi, 0.0));
    }

    pub(super) fn finish_scan(&mut self, pid: PlayerId, sys: SystemId, poi: u32) {
        let Some(p) = self.systems[&sys].layout.pois.iter().find(|p| p.id == poi).cloned() else { return };
        let prof = self.store.profile(pid).unwrap().clone();
        let tier = galaxy::system(sys).tier;
        let mult = tier.reward_mult() * if prof.profession == Profession::Explorer { 1.5 } else { 1.0 };
        let base = match p.kind {
            PoiKind::Anomaly => shared::ships::Materials { credits: 300, datacore: 2, crystal: 2, ..Default::default() },
            PoiKind::Ruin => shared::ships::Materials { credits: 500, datacore: 2, relic: 1, ..Default::default() },
            PoiKind::Derelict => shared::ships::Materials { credits: 400, alloy: 8, datacore: 1, ..Default::default() },
            PoiKind::Signal => shared::ships::Materials { credits: 350, datacore: 1, ..Default::default() },
            PoiKind::AncientGate => shared::ships::Materials { credits: 1500, relic: 2, datacore: 4, ..Default::default() },
        };
        let first_today = !prof.scanned.contains(&(sys, poi));
        let reward = if first_today {
            shared::ships::Materials {
                credits: (base.credits as f32 * mult) as u64,
                alloy: (base.alloy as f32 * mult).round() as u32,
                crystal: (base.crystal as f32 * mult).round() as u32,
                datacore: (base.datacore as f32 * mult).round() as u32,
                relic: base.relic,
            }
        } else {
            Default::default()
        };
        if let Some(pm) = self.store.profile_mut(pid) {
            pm.wallet.add(&reward);
            if first_today {
                pm.scanned.push((sys, poi));
            }
        }
        let kind = format!("{:?}", p.kind);
        self.history.record(self.unix, DomainEvent::Discovery { player: prof.name.clone(), ship: prof.ship.name.clone(), system: sys, site: p.name.clone(), kind });
        self.toast(pid, format!("스캔 완료: {} — 보상 {}", p.name, reward.describe()));
        let scanned: Vec<u32> = self.store.profile(pid).unwrap().scanned.iter().filter(|(s, _)| *s == sys).map(|(_, p)| *p).collect();
        self.send_player(pid, ServerMsg::Scanned { pois: scanned });
        if first_today {
            self.progress(pid, Progress::Scanned);
        }
        self.unearth(pid, sys, &p);
        self.scenario_scan(pid, sys, p.kind);
        self.push_profile(pid);
    }

    fn investigate(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, wreck: EntityId) {
        let Some(e) = eid.and_then(|e| self.ent(sys, e)) else { return };
        let Some(w) = self.ent(sys, wreck) else { return };
        if w.kind != EntityKind::Wreck {
            return;
        }
        if e.pos.distance(w.pos) > 350.0 {
            return self.toast(pid, "잔해에 350u 이내로 접근하세요");
        }
        let Some(&idx) = self.systems[&sys].wrecks.get(&wreck) else { return };
        let prof = self.store.profile(pid).unwrap().clone();
        let stored = self.store.data.wrecks[idx].clone();
        if stored.investigated_by.contains(&prof.name) {
            return self.toast(pid, "이미 조사한 잔해입니다");
        }
        let historian = prof.profession == Profession::Historian;
        let found = self.history.evidence_from_wreck(stored.event, historian);
        // Salvage: cargo that fits into the hold.
        let cap = prof.ship.stats().cargo as u32;
        let mut free = cap.saturating_sub(prof.cargo_used());
        let mut salvaged = 0;
        let mut taken: Vec<(String, u32)> = vec![];
        for (k, v) in stored.cargo.iter() {
            let n = (*v).min(free);
            if n > 0 {
                taken.push((k.clone(), n));
                free -= n;
                salvaged += n;
            }
        }
        {
            let w = &mut self.store.data.wrecks[idx];
            w.investigated_by.push(prof.name.clone());
            for (k, n) in &taken {
                if let Some(v) = w.cargo.get_mut(k) {
                    *v -= n;
                }
            }
            w.cargo.retain(|_, v| *v > 0);
        }
        if let Some(pm) = self.store.profile_mut(pid) {
            for (k, n) in taken {
                *pm.cargo.entry(k).or_default() += n;
            }
            for id in &found {
                if !pm.archive.contains(id) {
                    pm.archive.push(*id);
                }
            }
            pm.reputation += 1;
        }
        self.history.record(self.unix, DomainEvent::WreckInvestigated { investigator: prof.name.clone(), event: stored.event, found: found.len() as u32 });
        let artifacts = self.recover_artifacts(pid, sys, idx);
        let relics = if artifacts > 0 { format!(", 유물 {artifacts}점 회수") } else { String::new() };
        self.toast(pid, format!("잔해 조사: 증거 {}건 확보, 화물 {}단위 회수{relics}. 연대기에서 연구를 발표할 수 있습니다.", found.len(), salvaged));
        self.progress(pid, Progress::Scanned);
        self.push_profile(pid);
    }

    pub(super) fn require_docked(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>) -> bool {
        let docked = eid.and_then(|e| self.ent(sys, e)).is_some_and(|e| e.docked);
        if !docked {
            self.toast(pid, "정거장에 도킹해야 합니다");
        }
        docked
    }

    fn upgrade(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, module: Module) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let prof = self.store.profile(pid).unwrap().clone();
        let lvl = prof.ship.level(module);
        let cap = tier_cap(prof.scenarios_completed);
        if lvl >= MAX_MODULE_LEVEL {
            return self.toast(pid, "이미 최대 레벨입니다");
        }
        if lvl >= cap {
            return self.toast(pid, format!("Lv{} 이상은 시나리오를 돌파해야 해금됩니다 (현재 상한 Lv{cap})", cap + 1));
        }
        let cost = upgrade_cost(module, lvl);
        let old_rank = prof.ship.rank();
        let (new_rank, designation, ship_name, name) = {
            let pm = self.store.profile_mut(pid).unwrap();
            if !pm.wallet.pay(&cost) {
                let need = cost.describe();
                self.toast(pid, format!("자원 부족 — 필요: {need}"));
                return;
            }
            pm.ship.modules.insert(module, lvl + 1);
            (pm.ship.rank(), pm.ship.designation().to_string(), pm.ship.name.clone(), pm.name.clone())
        };
        let ship = self.store.profile(pid).unwrap().ship.clone();
        if let Some(e) = eid.and_then(|e| self.ent_mut(sys, e)) {
            let hf = e.hull / e.stats.max_hull;
            let sf = e.shield / e.stats.max_shield;
            e.stats = ship.stats();
            e.hull = hf * e.stats.max_hull;
            e.shield = sf * e.stats.max_shield;
            e.rank = new_rank;
            e.class = Some(ship.size_class());
            e.turret_cds.resize(e.stats.turrets as usize, 0.5);
        }
        self.toast(pid, format!("{} Lv{} → Lv{} 업그레이드 완료", module.name(), lvl, lvl + 1));
        if new_rank > old_rank {
            let old_class = shared::ships::SizeClass::of_rank(old_rank);
            let class = ship.size_class();
            self.history.record(self.unix, DomainEvent::ShipAscended { player: name, ship: ship_name, designation: designation.clone() });
            if class != old_class {
                let st = ship.stats();
                let what = match class {
                    shared::ships::SizeClass::Cruiser => format!("★ 「{}」이(가) 순양함으로 재건조되었습니다! 포탑 {}문, 함수포, 호위함 {}척", ship.name, st.turrets, st.escorts),
                    _ => format!("★★ 「{}」이(가) 주력함으로 거듭났습니다! 주포 일제사격, 포탑 {}문, 호위 함대 {}척", ship.name, st.turrets, st.escorts),
                };
                self.toast(pid, what);
            } else {
                self.toast(pid, format!("함선이 {designation}급으로 진급했습니다!"));
            }
        }
        self.push_profile(pid);
    }

    fn price(&self, sys: SystemId, ore: Ore, trader: bool) -> f32 {
        let tier = galaxy::system(sys).tier;
        let supply = self.store.supply(sys, ore);
        let scarcity = 1.0 / (1.0 + supply / 250.0);
        ore.base_price() * (0.8 + 0.25 * tier.pirate_level() as f32) * scarcity * if trader { 1.2 } else { 1.0 }
    }

    fn sell(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, ore: Ore, units: u32) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let have = *self.store.profile(pid).unwrap().cargo.get(ore.name()).unwrap_or(&0);
        let n = units.min(have);
        if n == 0 {
            return self.toast(pid, "판매할 화물이 없습니다");
        }
        let trader = self.store.profile(pid).unwrap().profession == Profession::Trader;
        // Price drops as you sell: integrate unit by unit (cheap: n ≤ cargo size).
        let mut total = 0.0;
        for _ in 0..n {
            total += self.price(sys, ore, trader);
            self.store.add_supply(sys, ore, 1.0);
        }
        let total = total.round() as u64;
        if let Some(pm) = self.store.profile_mut(pid) {
            let c = pm.cargo.get_mut(ore.name()).unwrap();
            *c -= n;
            if *c == 0 {
                pm.cargo.remove(ore.name());
            }
            pm.wallet.credits += total;
        }
        self.toast(pid, format!("{} {}단위 판매: +{} CR (현재 시세 {:.1})", ore.name(), n, total, self.price(sys, ore, trader)));
        self.progress(pid, Progress::Sold(n));
    }

    fn refine(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, ore: Ore, units: u32) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let ratio = match ore {
            Ore::Ferrite => 5,
            Ore::Lumen => 4,
            Ore::Xenite => 2,
        };
        let have = *self.store.profile(pid).unwrap().cargo.get(ore.name()).unwrap_or(&0);
        let batches = units.min(have) / ratio;
        if batches == 0 {
            return self.toast(pid, format!("정제하려면 {} {}단위 이상 필요합니다", ore.name(), ratio));
        }
        if let Some(pm) = self.store.profile_mut(pid) {
            let c = pm.cargo.get_mut(ore.name()).unwrap();
            *c -= batches * ratio;
            if *c == 0 {
                pm.cargo.remove(ore.name());
            }
            match ore {
                Ore::Ferrite => pm.wallet.alloy += batches,
                Ore::Lumen => pm.wallet.crystal += batches,
                Ore::Xenite => pm.wallet.datacore += batches,
            }
        }
        let what = match ore {
            Ore::Ferrite => "합금",
            Ore::Lumen => "결정",
            Ore::Xenite => "데이터코어",
        };
        self.toast(pid, format!("정제 완료: {what} +{batches}"));
        self.push_profile(pid);
    }

    fn claim_daily(&mut self, pid: PlayerId, index: usize) {
        let day = missions::day_index(self.unix);
        self.ensure_dailies(pid, day);
        let reward = {
            let pm = self.store.profile_mut(pid).unwrap();
            let Some(m) = pm.dailies.get_mut(index) else { return };
            if !m.complete() || m.claimed {
                None
            } else {
                m.claimed = true;
                Some(m.reward.clone())
            }
        };
        match reward {
            Some(r) => {
                self.store.profile_mut(pid).unwrap().wallet.add(&r);
                self.toast(pid, format!("일일 보상 수령: {}", r.describe()));
            }
            None => self.toast(pid, "아직 완료되지 않았거나 이미 수령했습니다"),
        }
        self.push_profile(pid);
    }

    // ---- Scenarios -------------------------------------------------------

    pub(super) fn scenario_step(&self, pid: PlayerId) -> Option<Step> {
        let sp = self.store.profile(pid)?.scenario.clone()?;
        scenarios().into_iter().find(|s| s.id == sp.id)?.steps.get(sp.step).cloned()
    }

    fn start_scenario(&mut self, pid: PlayerId, id: u32) {
        let prof = self.store.profile(pid).unwrap();
        if prof.scenario.is_some() {
            return self.toast(pid, "이미 진행 중인 시나리오가 있습니다");
        }
        if id > prof.scenarios_completed || id as usize >= scenarios().len() {
            return self.toast(pid, "이전 장을 먼저 완수하세요");
        }
        self.store.profile_mut(pid).unwrap().scenario = Some(ScenarioProgress { id, step: 0, step_progress: 0 });
        let title = scenarios()[id as usize].title;
        self.toast(pid, format!("시나리오 시작: {title}"));
        self.push_profile(pid);
        self.scenario_on_enter(pid);
    }

    /// Evaluate steps that complete by being somewhere, and spawn waves for Defeat steps.
    pub(super) fn scenario_on_enter(&mut self, pid: PlayerId) {
        let Some(conn) = self.conn_of.get(&pid).copied() else { return };
        let sys = self.sessions[&conn].system;
        // Waves stay in their system; leaving abandons the current wave (respawned on return).
        if self.sessions[&conn].wave_system.is_some_and(|w| w != sys) {
            self.clear_wave(conn);
        }
        match self.scenario_step(pid) {
            Some(Step::Reach { system }) if system == sys => self.advance_scenario(pid),
            Some(Step::Defeat { system, wave, level }) if system == sys && self.sessions[&conn].wave.is_empty() && !self.sessions[&conn].dead => {
                let Some(eid) = self.sessions[&conn].entity else { return };
                let Some(e) = self.ent(sys, eid) else { return };
                let base = e.pos + e.forward() * 1100.0 + Vec3::new(0.0, 120.0, 0.0);
                let mut ids = vec![];
                let mut k = 0.0;
                for (arch, n) in wave {
                    for _ in 0..n {
                        let off = Vec3::new((k * 2.4f32).cos() * 220.0, (k * 1.3f32).sin() * 60.0, (k * 2.4f32).sin() * 220.0);
                        ids.push(self.spawn_npc(sys, arch, level, base + off, Some(pid)));
                        k += 1.0;
                    }
                }
                let s = self.sessions.get_mut(&conn).unwrap();
                s.wave = ids;
                s.wave_system = Some(sys);
                self.toast(pid, "⚠ 적 함대가 워프아웃했습니다!");
            }
            _ => {}
        }
    }

    pub(super) fn scenario_scan(&mut self, pid: PlayerId, sys: SystemId, kind: PoiKind) {
        if let Some(Step::ScanPoi { system, kind: k, count }) = self.scenario_step(pid)
            && system == sys
            && k == kind
        {
            let done = {
                let sp = self.store.profile_mut(pid).unwrap().scenario.as_mut().unwrap();
                sp.step_progress += 1;
                sp.step_progress >= count
            };
            if done {
                self.advance_scenario(pid);
            } else {
                self.push_profile(pid);
            }
        }
    }

    /// Called when a wave NPC dies.
    pub(super) fn scenario_wave_kill(&mut self, owner: PlayerId, npc: EntityId) {
        let Some(conn) = self.conn_of.get(&owner).copied() else { return };
        let s = self.sessions.get_mut(&conn).unwrap();
        let before = s.wave.len();
        s.wave.retain(|w| *w != npc);
        if before > 0 && s.wave.is_empty() {
            s.wave_system = None;
            if matches!(self.scenario_step(owner), Some(Step::Defeat { .. })) {
                self.advance_scenario(owner);
            }
        }
    }

    pub(super) fn clear_wave(&mut self, conn: ConnId) {
        let s = self.sessions.get_mut(&conn).unwrap();
        let wave = std::mem::take(&mut s.wave);
        let ws = s.wave_system.take().unwrap_or(s.system);
        if let Some(sys) = self.systems.get_mut(&ws) {
            for w in wave {
                sys.ents.remove(&w);
            }
        }
    }

    fn advance_scenario(&mut self, pid: PlayerId) {
        let Some(sp) = self.store.profile(pid).unwrap().scenario.clone() else { return };
        let sc = scenarios().into_iter().find(|s| s.id == sp.id).unwrap();
        let next = sp.step + 1;
        if next >= sc.steps.len() {
            let prof = self.store.profile(pid).unwrap().clone();
            let first_clear = sp.id == prof.scenarios_completed;
            {
                let pm = self.store.profile_mut(pid).unwrap();
                pm.scenario = None;
                pm.wallet.add(&sc.reward);
                if first_clear {
                    pm.scenarios_completed += 1;
                }
                pm.reputation += 10;
            }
            self.history.record(self.unix, DomainEvent::ScenarioCompleted { player: prof.name.clone(), ship: prof.ship.name.clone(), chapter: sp.id, title: sc.title.into() });
            let cap = tier_cap(self.store.profile(pid).unwrap().scenarios_completed);
            self.toast(pid, format!("★ {} 완수! 보상 {} · 모듈 상한 Lv{cap}", sc.title, sc.reward.describe()));
        } else {
            self.store.profile_mut(pid).unwrap().scenario = Some(ScenarioProgress { id: sp.id, step: next, step_progress: 0 });
            self.toast(pid, format!("다음 목표: {}", sc.steps[next].describe()));
            self.push_profile(pid);
            self.scenario_on_enter(pid);
        }
        self.push_profile(pid);
    }

    // ---- Fleets ----------------------------------------------------------

    fn fleet_invite(&mut self, pid: PlayerId, name: &str) {
        let Some(target) = self.store.find_by_name(name) else {
            return self.toast(pid, "그런 함장은 없습니다");
        };
        if target == pid || !self.conn_of.contains_key(&target) {
            return self.toast(pid, "초대할 수 없습니다 (오프라인)");
        }
        if self.fleet_of(target).is_some() {
            return self.toast(pid, "이미 다른 함대 소속입니다");
        }
        let fid = match self.fleet_of(pid) {
            Some(f) => f,
            None => {
                let f = self.new_fleet(pid);
                self.broadcast_fleet(pid);
                f
            }
        };
        if self.fleets[&fid].members.len() >= 5 {
            return self.toast(pid, "함대는 최대 5척입니다");
        }
        let from = self.store.profile(pid).unwrap().name.clone();
        if let Some(c) = self.conn_of.get(&target).copied() {
            self.sessions.get_mut(&c).unwrap().invite = Some(fid);
        }
        self.send_player(target, ServerMsg::FleetInvite { from: from.clone(), fleet: fid });
        self.toast(pid, format!("{name}에게 함대 초대를 보냈습니다"));
    }

    fn fleet_accept(&mut self, conn: ConnId, pid: PlayerId, fid: FleetId) {
        if self.sessions[&conn].invite != Some(fid) || !self.fleets.contains_key(&fid) {
            return self.toast(pid, "유효한 초대가 없습니다");
        }
        if self.fleet_of(pid).is_some() {
            self.fleet_leave(pid);
        }
        let f = self.fleets.get_mut(&fid).unwrap();
        if f.members.len() >= 5 {
            return self.toast(pid, "함대가 가득 찼습니다");
        }
        f.members.push(pid);
        self.sessions.get_mut(&conn).unwrap().invite = None;
        self.broadcast_fleet(pid);
    }

    pub(super) fn fleet_leave(&mut self, pid: PlayerId) {
        let Some(fid) = self.fleet_of(pid) else { return };
        let f = self.fleets.get_mut(&fid).unwrap();
        f.members.retain(|m| *m != pid);
        let rest = f.members.clone();
        if rest.len() <= 1 {
            self.fleets.remove(&fid);
        } else if f.leader == pid {
            f.leader = rest[0];
        }
        if let Some((s, e)) = self.player_entity(pid)
            && let Some(ent) = self.ent_mut(s, e)
        {
            ent.fleet = None;
        }
        self.send_player(pid, ServerMsg::Fleet { id: None, members: vec![] });
        for m in rest {
            if !self.fleets.contains_key(&fid)
                && let Some((s, e)) = self.player_entity(m)
                && let Some(ent) = self.ent_mut(s, e)
            {
                ent.fleet = None;
            }
            self.broadcast_fleet(m);
        }
    }

    // ---- Research --------------------------------------------------------

    fn publish_research(&mut self, conn: ConnId, pid: PlayerId, event: shared::chronicle::EventId, thesis: String, cite: Vec<u64>) {
        let prof = self.store.profile(pid).unwrap().clone();
        let thesis: String = thesis.chars().take(160).collect();
        if thesis.trim().len() < 4 {
            return self.error(conn, "연구 주장을 입력하세요");
        }
        // Only evidence you hold can be cited: claims must stand on evidence.
        let cite: Vec<u64> = cite.into_iter().filter(|c| prof.archive.contains(c)).collect();
        if cite.is_empty() {
            return self.error(conn, "인용할 증거가 없습니다. 사건 현장의 잔해를 조사해 증거를 확보하세요.");
        }
        let historian = prof.profession == Profession::Historian;
        if self.history.record(self.unix, DomainEvent::ResearchPublished { author: prof.name.clone(), event, thesis, cited: cite, historian }).is_some() {
            if let Some(pm) = self.store.profile_mut(pid) {
                pm.reputation += 5;
                pm.wallet.credits += 400;
                pm.wallet.datacore += 1;
            }
            self.toast(pid, "연구가 연대기에 게재되었습니다 (+400 CR, 데이터코어 +1, 평판 +5)");
            self.push_profile(pid);
            self.send_chronicle(conn, pid);
        } else {
            self.error(conn, "이 사건에 대한 증거가 아닙니다");
        }
    }

    /// Forge a copy of evidence you hold (design doc §53). The copy joins your archive and can be
    /// cited; nothing public records the act until someone has it examined.
    fn forge_evidence(&mut self, conn: ConnId, pid: PlayerId, evidence: u64, text: String) {
        let prof = self.store.profile(pid).unwrap().clone();
        if !prof.archive.contains(&evidence) {
            return self.error(conn, "가지고 있는 증거만 위조할 수 있습니다");
        }
        let text: String = text.chars().take(160).collect();
        if text.trim().chars().count() < 4 {
            return self.error(conn, "위조할 내용을 입력하세요");
        }
        let cost = self.tuning.history.forge_cost;
        if prof.wallet.credits < cost {
            return self.error(conn, format!("위조 비용 {cost} CR이 부족합니다"));
        }
        let before = self.history.evidence.len();
        self.history.record(self.unix, DomainEvent::EvidenceForged { forger: prof.name.clone(), original: evidence, text });
        let Some(copy) = self.history.evidence.get(before).map(|e| e.id) else { return self.error(conn, "위조할 수 없는 증거입니다") };
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits -= cost;
            p.archive.push(copy);
        }
        self.toast(pid, format!("위조 사본 #{copy}을(를) 만들었습니다 (-{cost} CR). 누군가 감정하기 전까지는 원본과 구별되지 않습니다."));
        self.push_profile(pid);
        self.send_chronicle(conn, pid);
    }

    /// Have a piece of evidence examined (free for Historians): forgeries are exposed, with
    /// consequences for the forger and a reward for the examiner.
    fn authenticate_evidence(&mut self, conn: ConnId, pid: PlayerId, evidence: u64) {
        let prof = self.store.profile(pid).unwrap().clone();
        let Some(ev) = self.history.evidence.iter().find(|e| e.id == evidence).cloned() else {
            return self.error(conn, "없는 증거입니다");
        };
        if ev.authenticity != shared::chronicle::Authenticity::Unverified {
            return self.error(conn, format!("이미 감정된 증거입니다 ({})", ev.authenticity.label()));
        }
        let t = self.tuning.history;
        let cost = if prof.profession == Profession::Historian { 0 } else { t.authenticate_cost };
        if prof.wallet.credits < cost {
            return self.error(conn, format!("감정 비용 {cost} CR이 부족합니다"));
        }
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits -= cost;
        }
        let forger = self.history.forger_of(evidence).map(str::to_string);
        let exposed = self.history.record(self.unix, DomainEvent::EvidenceAuthenticated { examiner: prof.name.clone(), evidence, discredit: t.discredit });
        match (exposed, forger) {
            (Some(_), Some(forger)) => {
                if let Some(p) = self.store.profile_mut(pid) {
                    p.reputation += t.examiner_reputation;
                    p.wallet.credits += t.examiner_reward;
                }
                if let Some(fp) = self.store.find_by_name(&forger) {
                    if let Some(p) = self.store.profile_mut(fp) {
                        p.reputation += t.forger_reputation;
                    }
                    self.toast(fp, format!("⚠ 당신이 위조한 증거 #{evidence}이(가) {}에게 폭로되었습니다 (평판 {})", prof.name, t.forger_reputation));
                    self.push_profile(fp);
                }
                self.toast(pid, format!("위조 판명! 증거 #{evidence}은(는) {forger}의 위조품입니다 (+{} CR, 평판 +{})", t.examiner_reward, t.examiner_reputation));
                // Reporters whose articles rested on it answer for the correction.
                let reporters: Vec<String> = self.history.stories.iter().filter(|s| s.retracted && s.cited.contains(&evidence)).map(|s| s.reporter.clone()).collect();
                for r in reporters {
                    if let Some(rp) = self.store.find_by_name(&r) {
                        if let Some(p) = self.store.profile_mut(rp) {
                            p.reputation += t.retraction_reputation;
                        }
                        self.toast(rp, format!("⚠ 당신의 기사가 위조 증거에 근거한 것으로 드러나 정정되었습니다 (평판 {})", t.retraction_reputation));
                        self.push_profile(rp);
                    }
                }
            }
            _ => self.toast(pid, format!("감정 결과: 증거 #{evidence}은(는) 진품입니다{}", if cost > 0 { format!(" (비용 {cost} CR)") } else { String::new() })),
        }
        self.push_profile(pid);
        self.send_chronicle(conn, pid);
    }

    fn dev_command(&mut self, pid: PlayerId, text: &str) {
        let mut it = text.split_whitespace();
        match it.next() {
            Some("/grow") => {
                let lvl: u8 = it.next().and_then(|v| v.parse().ok()).unwrap_or(9).clamp(1, MAX_MODULE_LEVEL);
                if let Some(p) = self.store.profile_mut(pid) {
                    for m in Module::ALL {
                        p.ship.modules.insert(m, lvl);
                    }
                    p.scenarios_completed = p.scenarios_completed.max(5);
                }
                if let Some((sys, _)) = self.player_entity(pid) {
                    self.spawn_player(pid, sys, false);
                }
                self.push_profile(pid);
                self.toast(pid, format!("[dev] 모든 모듈 Lv{lvl}"));
            }
            Some("/cash") => {
                if let Some(p) = self.store.profile_mut(pid) {
                    p.wallet.add(&shared::ships::Materials { credits: 1_000_000, alloy: 5000, crystal: 5000, datacore: 2000, relic: 200 });
                }
                self.push_profile(pid);
            }
            _ => self.toast(pid, "[dev] /grow N, /cash"),
        }
    }

    pub(super) fn respawn(&mut self, pid: PlayerId) {
        let prof = self.store.profile(pid).unwrap().clone();
        let fee = (RESPAWN_FEE_PER_RANK * (1 + prof.ship.rank() as u64)).min(prof.wallet.credits);
        if let Some(pm) = self.store.profile_mut(pid) {
            pm.wallet.credits -= fee;
        }
        self.spawn_player(pid, prof.home, true);
        self.toast(pid, format!("「{}」이(가) 모항 정거장에서 재건되었습니다 (수리비 {fee} CR)", prof.ship.name));
        self.push_profile(pid);
    }
}
