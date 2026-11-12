//! The open galaxy's work and trade: station job boards, the goods market, titles
//! (`shared::jobs`, `shared::market`, `shared::titles`; numbers in `tuning/trade.ron`).
//!
//! Boards and prices are pure functions in `shared`; this is where they meet the world: taking a
//! job at a dock, the named pirate that waits for you two systems out, the armada that warps in,
//! the sealed cargo that goes down with your ship, the market that moves when you buy it out, the
//! customs officer who finds stims in your hold, and the title that comes with a career.

use super::*;
use shared::jobs::{self, ActiveJob, JobId, JobKind};
use shared::market::{self, Good};
use shared::titles::{self, Title};

impl Game {
    /// The station's stock offset of a good now (drifted back since it was last traded).
    pub fn market_offset(&self, sys: SystemId, g: Good) -> f32 {
        let t = &self.tuning.trade.market;
        self.store.data.market.iter().find(|r| r.system == sys && r.good == g).map_or(0.0, |r| market::relax(t, r.offset, self.unix.saturating_sub(r.unix) as f64))
    }

    fn set_market_offset(&mut self, sys: SystemId, g: Good, offset: f32) {
        let unix = self.unix;
        let rows = &mut self.store.data.market;
        match rows.iter_mut().find(|r| r.system == sys && r.good == g) {
            Some(r) => {
                r.offset = offset;
                r.unix = unix;
            }
            None => rows.push(crate::store::MarketRow { system: sys, good: g, offset, unix }),
        }
        self.store.dirty = true;
    }

    /// The board and market of the station `pid` is docked at.
    pub(super) fn send_station(&mut self, pid: PlayerId, sys: SystemId) {
        let t = self.tuning.clone();
        let period = jobs::period(&t, self.unix);
        let board = jobs::board(&t, sys, period);
        let quotes = market::quotes(&t.trade.market, sys, |g| self.market_offset(sys, g));
        let next_board = (period + 1) * t.trade.jobs.period_secs.max(1);
        self.send_player(pid, ServerMsg::StationBoard { system: sys, jobs: board, market: quotes, next_board });
    }

    pub(super) fn handle_trade(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, msg: ClientMsg) {
        match msg {
            ClientMsg::RequestStation => {
                if eid.and_then(|e| self.ent(sys, e)).is_some_and(|e| e.docked) {
                    self.send_station(pid, sys);
                }
            }
            ClientMsg::AcceptJob { id } => self.accept_job(pid, sys, eid, id),
            ClientMsg::AbandonJob { id } => self.abandon_job(pid, id),
            ClientMsg::BuyGood { good, units } => self.buy_good(pid, sys, eid, good, units),
            ClientMsg::SellGood { good, units } => self.sell_good(pid, sys, eid, good, units),
            ClientMsg::SetTitle { title } => {
                let Some(p) = self.store.profile_mut(pid) else { return };
                if title.is_none_or(|t| p.titles.contains(&t)) {
                    p.title = title;
                    self.push_profile(pid);
                }
            }
            _ => {}
        }
    }

    // ---- Jobs --------------------------------------------------------------

    fn accept_job(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, id: JobId) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let t = self.tuning.clone();
        let Some(job) = jobs::board(&t, sys, jobs::period(&t, self.unix)).into_iter().find(|j| j.id == id) else {
            return self.toast(pid, "이 의뢰는 이제 게시판에 없습니다");
        };
        let prof = self.store.profile(pid).unwrap();
        if prof.jobs.iter().any(|a| a.job.id == id) {
            return self.toast(pid, "이미 맡은 의뢰입니다");
        }
        if prof.jobs.len() >= t.trade.jobs.max_active {
            return self.toast(pid, format!("의뢰는 동시에 {}건까지 맡을 수 있습니다", t.trade.jobs.max_active));
        }
        if let Some((key, units)) = carried(&job.kind) {
            let room = self.ship_stats(prof).cargo as u32;
            if prof.cargo_used() + units > room {
                let what = if key == jobs::SEALED { "봉인 화물" } else { "밀수품" };
                return self.toast(pid, format!("화물칸이 부족합니다 — {what} {units}단위를 실을 자리가 필요합니다"));
            }
        }
        let text = job.describe();
        let p = self.store.profile_mut(pid).unwrap();
        if let Some((key, units)) = carried(&job.kind) {
            *p.cargo.entry(key.into()).or_insert(0) += units;
        }
        // The employer gives the wreck's coordinates.
        if let JobKind::Salvage { system, poi, .. } = job.kind
            && !p.revealed.contains(&(system, poi))
        {
            p.revealed.push((system, poi));
        }
        p.jobs.push(ActiveJob { job, accepted: self.unix, visited: vec![] });
        self.toast(pid, format!("의뢰 수락: {text}"));
        self.push_profile(pid);
        self.send_station(pid, sys);
        // A bounty or an armada right here starts now.
        self.jobs_on_enter(pid);
    }

    fn abandon_job(&mut self, pid: PlayerId, id: JobId) {
        let Some(p) = self.store.profile_mut(pid) else { return };
        let Some(k) = p.jobs.iter().position(|a| a.job.id == id) else { return };
        let a = p.jobs.remove(k);
        unload(p, &a.job.kind);
        p.reputation -= 1;
        self.clear_job_wave(pid, id);
        self.toast(pid, format!("의뢰 포기: {} (평판 −1)", a.job.describe()));
        self.push_profile(pid);
    }

    /// Pay out a finished job and put it on the record.
    fn complete_job(&mut self, pid: PlayerId, id: JobId) {
        let Some(p) = self.store.profile_mut(pid) else { return };
        let Some(k) = p.jobs.iter().position(|a| a.job.id == id) else { return };
        let a = p.jobs.remove(k);
        p.wallet.add(&a.job.reward);
        p.reputation += a.job.reputation;
        *p.career.jobs.entry(a.job.kind.tag()).or_insert(0) += 1;
        let tag = a.job.kind.tag();
        if let Some(m) = self.metrics.as_mut() {
            m.push(crate::metrics::Metric::Reward { t: self.now, player: pid, credits: a.job.reward.credits });
        }
        self.toast(pid, format!("★ 의뢰 완수 [{}] {} — {}", tag.name(), a.job.describe(), a.job.reward.describe()));
        self.check_titles(pid);
        self.push_profile(pid);
    }

    fn fail_job(&mut self, pid: PlayerId, id: JobId, why: &str) {
        let Some(p) = self.store.profile_mut(pid) else { return };
        let Some(k) = p.jobs.iter().position(|a| a.job.id == id) else { return };
        let a = p.jobs.remove(k);
        unload(p, &a.job.kind);
        p.reputation -= 2;
        self.clear_job_wave(pid, id);
        self.toast(pid, format!("의뢰 실패 — {why}: {} (평판 −2)", a.job.describe()));
        self.push_profile(pid);
    }

    /// Jobs past their deadline fail.
    pub(super) fn expire_jobs(&mut self, pid: PlayerId) {
        let unix = self.unix;
        let late: Vec<JobId> = self.store.profile(pid).map(|p| p.jobs.iter().filter(|a| a.expired(unix)).map(|a| a.job.id).collect()).unwrap_or_default();
        for id in late {
            self.fail_job(pid, id, "기한 초과");
        }
    }

    /// Arriving somewhere (warp, login, respawn): survey marks, the career's map, and the waiting
    /// targets of bounties and armadas in this system.
    pub(super) fn jobs_on_enter(&mut self, pid: PlayerId) {
        let Some(conn) = self.conn_of.get(&pid).copied() else { return };
        let sys = self.sessions[&conn].system;
        // Job targets stay in their system: leaving lets them go (they wait there again on return).
        let gone: Vec<JobId> = self.sessions[&conn].job_waves.iter().filter(|(_, (s, _))| *s != sys).map(|(id, _)| *id).collect();
        for id in gone {
            self.clear_job_wave(pid, id);
        }
        self.expire_jobs(pid);
        let Some(p) = self.store.profile_mut(pid) else { return };
        let first_visit = p.career.visited.insert(sys);
        let mut done = vec![];
        for a in p.jobs.iter_mut() {
            if let JobKind::Survey { systems } = &a.job.kind
                && systems.contains(&sys)
                && !a.visited.contains(&sys)
            {
                a.visited.push(sys);
                if a.visited.len() == systems.len() {
                    done.push(a.job.id);
                }
            }
        }
        let fights: Vec<(JobId, JobKind)> = p
            .jobs
            .iter()
            .filter(|a| match &a.job.kind {
                JobKind::Bounty { system, .. } | JobKind::Armada { system, .. } => *system == sys,
                JobKind::Patrol { systems, .. } => systems.contains(&sys) && !a.visited.contains(&sys),
                _ => false,
            })
            .map(|a| (a.job.id, a.job.kind.clone()))
            .collect();
        for id in done {
            self.complete_job(pid, id);
        }
        if first_visit {
            self.store.dirty = true;
        }
        self.check_titles(pid);
        let s = &self.sessions[&conn];
        if s.dead || s.entity.is_none() {
            return;
        }
        for (id, kind) in fights {
            if !self.sessions[&conn].job_waves.contains_key(&id) {
                self.spawn_job_wave(pid, conn, sys, id, kind);
            }
        }
    }

    fn spawn_job_wave(&mut self, pid: PlayerId, conn: ConnId, sys: SystemId, id: JobId, kind: JobKind) {
        let Some(eid) = self.sessions[&conn].entity else { return };
        let Some(e) = self.ent(sys, eid) else { return };
        let (fwd, at) = (e.forward(), e.pos);
        let (wave, level, name): (Vec<(NpcArchetype, u32)>, u8, Option<String>) = match &kind {
            JobKind::Bounty { target, archetype, level, escorts, .. } => (vec![(*archetype, 1), (NpcArchetype::Raider, *escorts)], *level, Some(target.clone())),
            JobKind::Armada { level, wave, .. } => (wave.clone(), *level, None),
            JobKind::Patrol { .. } => (self.tuning.trade.jobs.patrol_wave.clone(), galaxy::system(sys).tier.pirate_level(), None),
            _ => return,
        };
        let patrol = matches!(kind, JobKind::Patrol { .. });
        // They come out of warp some way off, spread wide: a fight with room in it.
        let dist = if name.is_some() { 1800.0 } else { 2600.0 };
        let base = at + fwd * dist + Vec3::new(0.0, 160.0, 0.0);
        let mut ids = vec![];
        let mut k = 0.0f32;
        for (arch, n) in wave {
            for _ in 0..n {
                let off = Vec3::new((k * 2.4).cos() * 320.0, (k * 1.3).sin() * 90.0, (k * 2.4).sin() * 320.0);
                let npc = self.spawn_npc(sys, arch, level, base + off, Some(pid));
                if let (Some(n), true) = (&name, ids.is_empty())
                    && let Some(ent) = self.ent_mut(sys, npc)
                {
                    ent.name = format!("「{n}」 {}", arch.name());
                }
                ids.push(npc);
                k += 1.0;
            }
        }
        let count = ids.len();
        self.sessions.get_mut(&conn).unwrap().job_waves.insert(id, (sys, ids));
        match name {
            Some(n) => self.toast(pid, format!("⚠ 현상금 표적 「{n}」이(가) 호위 {}척과 함께 나타났습니다!", count - 1)),
            None if patrol => self.toast(pid, format!("⚠ 순찰: 해적 무리 {count}척 발견 — 소탕하십시오")),
            None => self.toast(pid, format!("⚠ 해적 함대 {count}척이 워프아웃했습니다! 함대 방어전 개시")),
        }
    }

    fn clear_job_wave(&mut self, pid: PlayerId, id: JobId) {
        let Some(conn) = self.conn_of.get(&pid).copied() else { return };
        let Some((sys, ids)) = self.sessions.get_mut(&conn).and_then(|s| s.job_waves.remove(&id)) else { return };
        if let Some(st) = self.systems.get_mut(&sys) {
            for w in ids {
                st.ents.remove(&w);
            }
        }
    }

    /// A wave NPC died: if it belonged to one of the owner's jobs, the job may be done. Returns
    /// whether it was a job's.
    /// Whether a salvage job this captain holds has its wreck at this site (and the recorder is not out yet).
    pub(super) fn salvage_here(&self, pid: PlayerId, sys: SystemId, poi: u32) -> bool {
        self.store.profile(pid).is_some_and(|p| p.cargo.get(jobs::BLACKBOX).copied().unwrap_or(0) == 0 && p.jobs.iter().any(|a| matches!(a.job.kind, JobKind::Salvage { system, poi: x, .. } if system == sys && x == poi)))
    }

    /// The wreck of a salvage job scanned: its flight recorder goes into the hold.
    pub(super) fn salvage_scan(&mut self, pid: PlayerId, sys: SystemId, poi: u32) {
        if !self.salvage_here(pid, sys, poi) {
            return;
        }
        let Some(p) = self.store.profile_mut(pid) else { return };
        *p.cargo.entry(jobs::BLACKBOX.into()).or_insert(0) += 1;
        self.toast(pid, "잔해 회수: 블랙박스를 확보했습니다 — 의뢰한 정거장에 반납하세요");
    }

    /// Whether this NPC is one of a job's targets (its owner's bounty or armada).
    pub(super) fn in_job_wave(&self, owner: PlayerId, npc: EntityId) -> bool {
        self.conn_of.get(&owner).and_then(|c| self.sessions.get(c)).is_some_and(|s| s.job_waves.values().any(|(_, ids)| ids.contains(&npc)))
    }

    pub(super) fn job_wave_kill(&mut self, owner: PlayerId, npc: EntityId) -> bool {
        let Some(conn) = self.conn_of.get(&owner).copied() else { return false };
        let s = self.sessions.get_mut(&conn).unwrap();
        let Some(id) = s.job_waves.iter().find(|(_, (_, ids))| ids.contains(&npc)).map(|(id, _)| *id) else { return false };
        let (sys, ids) = s.job_waves.get_mut(&id).unwrap();
        let sys = *sys;
        ids.retain(|w| *w != npc);
        if ids.is_empty() {
            s.job_waves.remove(&id);
            // A patrol stop is cleared; the job ends with the last one.
            let stop = self.store.profile_mut(owner).and_then(|p| p.jobs.iter_mut().find(|a| a.job.id == id)).and_then(|a| match &a.job.kind {
                JobKind::Patrol { systems, .. } => {
                    if !a.visited.contains(&sys) {
                        a.visited.push(sys);
                    }
                    Some((a.visited.len(), systems.len()))
                }
                _ => None,
            });
            match stop {
                Some((done, all)) if done < all => {
                    self.toast(owner, format!("순찰: {} 소탕 ({done}/{all})", galaxy::system(sys).name));
                    self.push_profile(owner);
                }
                _ => self.complete_job(owner, id),
            }
        }
        true
    }

    /// The ship went down with the hold: sealed cargo is lost, and so is the haul.
    pub(super) fn jobs_on_death(&mut self, pid: PlayerId) {
        let hauls: Vec<JobId> = self.store.profile(pid).map(|p| p.jobs.iter().filter(|a| carried(&a.job.kind).is_some()).map(|a| a.job.id).collect()).unwrap_or_default();
        for id in hauls {
            self.fail_job(pid, id, "실은 화물이 함선과 함께 사라졌습니다");
        }
        let waves: Vec<JobId> = self.conn_of.get(&pid).and_then(|c| self.sessions.get(c)).map(|s| s.job_waves.keys().copied().collect()).unwrap_or_default();
        for id in waves {
            self.clear_job_wave(pid, id);
        }
    }

    /// Docked at `sys`: customs, then every job that ends here.
    pub(super) fn jobs_on_dock(&mut self, pid: PlayerId, sys: SystemId) {
        self.customs(pid, sys);
        self.expire_jobs(pid);
        let t = self.tuning.clone();
        let Some(p) = self.store.profile(pid).cloned() else { return };
        for a in &p.jobs {
            let here = a.job.station == sys;
            match &a.job.kind {
                JobKind::Haul { to, units } if *to == sys => {
                    if p.cargo.get(jobs::SEALED).copied().unwrap_or(0) >= *units {
                        take_cargo(self.store.profile_mut(pid).unwrap(), jobs::SEALED, *units);
                        self.complete_job(pid, a.job.id);
                    }
                }
                JobKind::Smuggle { to, units } if *to == sys => {
                    if p.cargo.get(jobs::CONTRABAND).copied().unwrap_or(0) >= *units {
                        let pm = self.store.profile_mut(pid).unwrap();
                        take_cargo(pm, jobs::CONTRABAND, *units);
                        pm.career.smuggled += *units;
                        self.complete_job(pid, a.job.id);
                    }
                }
                JobKind::Salvage { .. } if here => {
                    if p.cargo.get(jobs::BLACKBOX).copied().unwrap_or(0) >= 1 {
                        take_cargo(self.store.profile_mut(pid).unwrap(), jobs::BLACKBOX, 1);
                        self.complete_job(pid, a.job.id);
                    }
                }
                JobKind::Supply { good, units } if here => {
                    if p.cargo.get(good.key()).copied().unwrap_or(0) >= *units {
                        take_cargo(self.store.profile_mut(pid).unwrap(), good.key(), *units);
                        self.complete_job(pid, a.job.id);
                    } else {
                        self.toast(pid, format!("보급 의뢰: {} {}단위가 필요합니다", good.name(), units));
                    }
                }
                JobKind::Prospect { rarity, count } if here => {
                    // The cheapest qualifying specimens go first.
                    let held = self.store.profile(pid).unwrap().specimens.clone();
                    let mut fits: Vec<(u64, shared::minerals::MineralId, u32)> = held
                        .iter()
                        .filter_map(|(id, n)| shared::minerals::mineral_in(&t.minerals, *id).filter(|m| m.rarity >= *rarity).map(|m| (m.specimen_value(&t.minerals), *id, *n)))
                        .collect();
                    if fits.iter().map(|f| f.2).sum::<u32>() < *count {
                        self.toast(pid, format!("광물 주문: {} 이상 표본 {count}점이 필요합니다", rarity.label()));
                        continue;
                    }
                    fits.sort();
                    let mut need = *count;
                    let pm = self.store.profile_mut(pid).unwrap();
                    for (_, id, n) in fits {
                        let take = n.min(need);
                        need -= take;
                        let left = pm.specimens.get_mut(&id).unwrap();
                        *left -= take;
                        if *left == 0 {
                            pm.specimens.remove(&id);
                        }
                        if need == 0 {
                            break;
                        }
                    }
                    self.complete_job(pid, a.job.id);
                }
                _ => {}
            }
        }
        self.send_station(pid, sys);
    }

    /// Core customs: contraband in the hold is seized and the captain fined.
    fn customs(&mut self, pid: PlayerId, sys: SystemId) {
        let t = self.tuning.clone();
        let m = &t.trade.market;
        if !market::seized(m, galaxy::system(sys).tier) {
            return;
        }
        let Some(p) = self.store.profile_mut(pid) else { return };
        let mut fine = 0.0;
        let mut what = vec![];
        for g in m.contraband.iter() {
            if let Some(n) = p.cargo.remove(g.key()) {
                fine += n as f32 * m.base.get(g).copied().unwrap_or(0.0) * m.fine;
                what.push(format!("{} {n}단위", g.name()));
            }
        }
        let mut runs = vec![];
        if let Some(n) = p.cargo.remove(jobs::CONTRABAND) {
            fine += n as f32 * m.base.get(&Good::Stims).copied().unwrap_or(0.0) * m.fine;
            what.push(format!("밀수품 {n}상자"));
            runs = p.jobs.iter().filter(|a| matches!(a.job.kind, JobKind::Smuggle { .. })).map(|a| a.job.id).collect();
        }
        if what.is_empty() {
            return;
        }
        let fine = (fine.round() as u64).min(p.wallet.credits);
        p.wallet.credits -= fine;
        p.reputation -= m.fine_reputation;
        self.toast(pid, format!("세관 검색: {} 압수, 벌금 {fine} CR (평판 −{})", what.join(", "), m.fine_reputation));
        for id in runs {
            self.fail_job(pid, id, "세관에 밀수품을 압수당했습니다");
        }
        self.push_profile(pid);
    }

    // ---- Market ------------------------------------------------------------

    fn buy_good(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, good: Good, units: u32) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let t = self.tuning.clone();
        let m = &t.trade.market;
        let prof = self.store.profile(pid).unwrap();
        let room = (self.ship_stats(prof).cargo as u32).saturating_sub(prof.cargo_used());
        let offset = self.market_offset(sys, good);
        let Some(unit) = market::buy_price(m, sys, good, offset) else {
            return self.toast(pid, format!("이 정거장은 {}을(를) 팔지 않습니다", good.name()));
        };
        // As many as the hold, the wallet and the ask allow.
        let mut n = units.min(room);
        let credits = prof.wallet.credits;
        while n > 0 && market::buy_total(m, sys, good, offset, n).is_none_or(|(c, _)| c.ceil() as u64 > credits) {
            n = (n as f32 * 0.8) as u32;
        }
        if n == 0 {
            return self.toast(pid, if room == 0 { "화물칸이 가득 찼습니다".to_string() } else { format!("크레딧이 부족합니다 (단가 {unit:.0} CR)") });
        }
        let (cost, after) = market::buy_total(m, sys, good, offset, n).unwrap();
        let cost = cost.ceil() as u64;
        self.set_market_offset(sys, good, after);
        let p = self.store.profile_mut(pid).unwrap();
        p.wallet.credits -= cost;
        *p.cargo.entry(good.key().into()).or_insert(0) += n;
        p.career.trade_profit -= cost as i64;
        self.toast(pid, format!("{} {n}단위 구매: −{cost} CR", good.name()));
        self.push_profile(pid);
        self.send_station(pid, sys);
    }

    fn sell_good(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, good: Good, units: u32) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let t = self.tuning.clone();
        let m = &t.trade.market;
        let have = self.store.profile(pid).unwrap().cargo.get(good.key()).copied().unwrap_or(0);
        let n = units.min(have);
        if n == 0 {
            return self.toast(pid, "판매할 화물이 없습니다");
        }
        let offset = self.market_offset(sys, good);
        let Some((total, after)) = market::sell_total(m, sys, good, offset, n) else {
            return self.toast(pid, format!("이 정거장은 {}을(를) 사지 않습니다", good.name()));
        };
        self.set_market_offset(sys, good, after);
        let gross = (total * self.home_turf(pid, sys)).round() as u64;
        let net = self.levy(pid, sys, gross);
        let p = self.store.profile_mut(pid).unwrap();
        take_cargo(p, good.key(), n);
        p.wallet.credits += net;
        p.career.trade_profit += net as i64;
        if market::contraband(m, good) {
            p.career.smuggled += n;
        }
        self.toast(pid, format!("{} {n}단위 판매: +{net} CR", good.name()));
        self.progress(pid, Progress::Sold(n));
        self.gain_influence(pid, sys, n as f32 * self.tuning.social.influence_sale_unit);
        self.check_titles(pid);
        self.push_profile(pid);
        self.send_station(pid, sys);
    }

    // ---- Titles ------------------------------------------------------------

    /// Award every title the record has earned and not yet been given; wear the first one if
    /// none is worn.
    pub(super) fn check_titles(&mut self, pid: PlayerId) {
        let th = self.tuning.trade.titles.clone();
        let Some(p) = self.store.profile_mut(pid) else { return };
        let earned = titles::earned(&th, p.scenarios_completed, p.kills, &p.career);
        let new: Vec<Title> = earned.into_iter().filter(|t| !p.titles.contains(t)).collect();
        if new.is_empty() {
            return;
        }
        p.titles.extend(new.iter().copied());
        if p.title.is_none() {
            p.title = new.first().copied();
        }
        for t in new {
            self.toast(pid, format!("🏅 칭호 획득: 〈{}〉 — {}", t.name(), t.how(th.get(&t).copied().unwrap_or(0))));
        }
        self.push_profile(pid);
    }
}

/// What a job loads into the hold on accepting: its hold key and units.
fn carried(kind: &JobKind) -> Option<(&'static str, u32)> {
    match kind {
        JobKind::Haul { units, .. } => Some((jobs::SEALED, *units)),
        JobKind::Smuggle { units, .. } => Some((jobs::CONTRABAND, *units)),
        _ => None,
    }
}

/// A job given up or failed: what it loaded leaves the hold.
fn unload(p: &mut Profile, kind: &JobKind) {
    if let Some((key, units)) = carried(kind) {
        take_cargo(p, key, units);
    }
    if matches!(kind, JobKind::Salvage { .. }) {
        take_cargo(p, jobs::BLACKBOX, 1);
    }
}

fn take_cargo(p: &mut Profile, key: &str, units: u32) {
    if let Some(c) = p.cargo.get_mut(key) {
        *c = c.saturating_sub(units);
        if *c == 0 {
            p.cargo.remove(key);
        }
    }
}
