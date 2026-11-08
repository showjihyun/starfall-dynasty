//! Political influence (design doc Phase 3 "Faction · Political Influence"): what alliance members
//! do in a star system — hunting pirates, trading, honouring contracts, giving to its museum — earns
//! their alliance influence there. The leading alliance takes control (with a margin, so control
//! does not flicker), its members get better prices and bounties on home turf, and influence fades
//! hourly, so a system has to be held by staying active. Core space belongs to no one.

use super::*;
use history::DomainEvent;
use shared::galaxy::RiskTier;
use shared::protocol::SystemStanding;

impl Game {
    /// Influence points of an alliance in a system.
    pub fn influence(&self, sys: SystemId, alliance: AllianceId) -> f32 {
        self.store.data.influence.get(&sys).and_then(|m| m.get(&alliance)).copied().unwrap_or(0.0)
    }

    /// The alliance controlling a system, if it still exists.
    pub fn controller(&self, sys: SystemId) -> Option<AllianceId> {
        self.store.data.controllers.get(&sys).copied().filter(|a| self.alliance(*a).is_some())
    }

    /// Price/bounty multiplier for a player in a system: the controlling alliance's members do better.
    pub(super) fn home_turf(&self, pid: PlayerId, sys: SystemId) -> f32 {
        match (self.controller(sys), self.alliance_of(pid)) {
            (Some(c), Some(a)) if c == a => 1.0 + self.tuning.social.control_bonus,
            _ => 1.0,
        }
    }

    /// `pid` did something worth `points` of influence in `sys`.
    pub(super) fn gain_influence(&mut self, pid: PlayerId, sys: SystemId, points: f32) {
        let Some(a) = self.alliance_of(pid) else { return };
        if points > 0.0 {
            // Someone else's ground: hunting and trade barely move it — battles do (`battle_influence`).
            let foreign = self.controller(sys).is_some_and(|c| c != a);
            self.shift_influence(a, sys, if foreign { points * self.tuning.social.foreign_gain } else { points });
        }
    }

    /// The holder's station batteries in a system, if fortified or worn (none: fresh, level 0).
    pub fn battery(&self, sys: SystemId) -> Option<crate::store::Battery> {
        let owner = self.controller(sys)?;
        self.store.data.batteries.get(&sys).filter(|b| b.owner == owner).cloned()
    }

    /// The leader spends the treasury on a held system's station batteries (`station_guard.fortify_*`).
    pub(super) fn alliance_fortify(&mut self, pid: PlayerId, sys: SystemId) {
        let g = self.tuning.social.station_guard;
        let Some(aid) = self.alliance_of(pid) else { return self.toast(pid, "동맹 소속이 아닙니다") };
        let Some(a) = self.alliance(aid).cloned() else { return };
        if a.leader != pid {
            return self.toast(pid, "맹주만 포대를 강화할 수 있습니다");
        }
        if self.controller(sys) != Some(aid) {
            return self.toast(pid, "우리 동맹이 지배하는 성계가 아닙니다");
        }
        let mut b = self.battery(sys).unwrap_or(crate::store::Battery { owner: aid, ..Default::default() });
        if g.fortify_cost == 0 || b.level >= g.fortify_max {
            return self.toast(pid, "더 강화할 수 없습니다");
        }
        let cost = g.fortify_cost * (b.level as u64 + 1);
        if a.treasury < cost {
            return self.toast(pid, format!("금고가 모자랍니다 ({cost} CR 필요)"));
        }
        if let Some(x) = self.store.data.alliances.iter_mut().find(|x| x.id == aid) {
            x.treasury -= cost;
        }
        b.level += 1;
        let level = b.level;
        self.store.data.batteries.insert(sys, b);
        self.store.dirty = true;
        for m in &a.members {
            self.toast(*m, format!("[{}] {} 정거장 포대 Lv{level} (금고 −{cost} CR)", a.tag, galaxy::system(sys).name));
        }
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.send_alliances(p);
            self.send_influence(p);
        }
    }

    /// Systems an alliance controls.
    pub fn held_by(&self, a: AllianceId) -> Vec<SystemId> {
        self.store.data.controllers.iter().filter(|(_, c)| **c == a).map(|(s, _)| *s).collect()
    }

    /// Hourly treasury upkeep for holding `n` systems.
    pub fn upkeep(&self, n: usize) -> u64 {
        self.tuning.social.upkeep(n)
    }

    /// Add (or, negative, take away) an alliance's influence in a system, then re-check control.
    /// Gains shrink with every other system the alliance already holds (overreach).
    pub(super) fn shift_influence(&mut self, a: AllianceId, sys: SystemId, points: f32) {
        if galaxy::system(sys).tier == RiskTier::Core {
            return;
        }
        let points = if points > 0.0 {
            let others = self.held_by(a).into_iter().filter(|s| *s != sys).count();
            points / (1.0 + self.tuning.social.overreach * others as f32)
        } else {
            points
        };
        let m = self.store.data.influence.entry(sys).or_default();
        let v = m.entry(a).or_default();
        *v = (*v + points).max(0.0);
        m.retain(|_, v| *v >= 0.1);
        self.store.dirty = true;
        self.update_control(sys);
        // A holder knocked below half the threshold loses its grip at once.
        if let Some(h) = self.controller(sys)
            && self.influence(sys, h) < self.tuning.social.control_threshold * 0.5
        {
            self.set_controller(sys, None);
            self.update_control(sys);
        }
    }

    /// A battle ended in `sys`: alliances on the winning side gain influence, those on losing sides lose some.
    pub(super) fn battle_influence(&mut self, sys: SystemId, winner: &str, sides: &[String]) {
        let t = self.tuning.social;
        let alliance_of_key = |k: &str| k.strip_prefix('A').and_then(|id| id.parse::<AllianceId>().ok());
        for k in sides {
            let Some(a) = alliance_of_key(k) else { continue };
            if k == winner {
                self.shift_influence(a, sys, t.influence_battle_win);
            } else {
                self.shift_influence(a, sys, -t.influence_battle_loss);
            }
        }
    }

    /// Tax on a non-member's income in a controlled system: returns what they keep; the rest goes to
    /// the controller's treasury. Members (and uncontrolled space) pay nothing.
    pub(super) fn levy(&mut self, pid: PlayerId, sys: SystemId, income: u64) -> u64 {
        let Some(c) = self.controller(sys) else { return income };
        if self.alliance_of(pid) == Some(c) {
            return income;
        }
        let tax = (income as f32 * self.tuning.social.tax_rate).round() as u64;
        if tax == 0 {
            return income;
        }
        if let Some(a) = self.store.data.alliances.iter_mut().find(|a| a.id == c) {
            a.treasury += tax;
        }
        self.store.dirty = true;
        income - tax
    }

    /// The leader pays the treasury out in equal shares (the remainder stays).
    pub(super) fn alliance_dividend(&mut self, pid: PlayerId) {
        let Some(aid) = self.alliance_of(pid) else { return self.toast(pid, "동맹 소속이 아닙니다") };
        let Some(a) = self.alliance(aid).cloned() else { return };
        if a.leader != pid {
            return self.toast(pid, "맹주만 배당할 수 있습니다");
        }
        let n = a.members.len().max(1) as u64;
        let share = a.treasury / n;
        if share == 0 {
            return self.toast(pid, "배당할 금고 잔액이 없습니다");
        }
        if let Some(x) = self.store.data.alliances.iter_mut().find(|x| x.id == aid) {
            x.treasury -= share * n;
        }
        let mut names = vec![];
        for m in &a.members {
            if let Some(p) = self.store.profile_mut(*m) {
                p.wallet.credits += share;
                names.push(p.name.clone());
            }
            self.toast(*m, format!("[{}] 금고 배당: +{share} CR", a.tag));
            self.push_profile(*m);
        }
        self.history.record(self.unix, DomainEvent::DividendPaid { alliance: format!("[{}] {}", a.tag, a.name), share, members: names });
        self.store.dirty = true;
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.send_alliances(p);
        }
    }

    /// Hand control to the leading alliance once it passes the threshold and beats the holder by the margin.
    fn update_control(&mut self, sys: SystemId) {
        let t = self.tuning.social;
        let Some((&top, &points)) = self.store.data.influence.get(&sys).and_then(|m| m.iter().filter(|(a, _)| self.alliance(**a).is_some()).max_by(|x, y| x.1.total_cmp(y.1).then(y.0.cmp(x.0)))) else { return };
        let holder = self.controller(sys);
        if holder == Some(top) || points < t.control_threshold {
            return;
        }
        if let Some(h) = holder
            && points <= self.influence(sys, h) * (1.0 + t.control_margin)
        {
            return;
        }
        self.set_controller(sys, Some(top));
    }

    fn set_controller(&mut self, sys: SystemId, new: Option<AllianceId>) {
        self.change_controller(sys, new, None);
    }

    fn change_controller(&mut self, sys: SystemId, new: Option<AllianceId>, reason: Option<&str>) {
        let name_of = |g: &Game, a: Option<AllianceId>| a.and_then(|a| g.alliance(a)).map(|a| format!("[{}] {}", a.tag, a.name));
        let previous = name_of(self, self.controller(sys));
        match new {
            Some(a) => {
                self.store.data.controllers.insert(sys, a);
            }
            None => {
                self.store.data.controllers.remove(&sys);
            }
        }
        self.store.dirty = true;
        self.history.record(self.unix, DomainEvent::SystemControlChanged { system: sys, alliance: name_of(self, new), previous: previous.clone(), reason: reason.map(str::to_string) });
        let sys_name = galaxy::system(sys).name;
        let text = match (name_of(self, new), previous) {
            (Some(n), Some(p)) => format!("{sys_name}의 패권이 {p}에서 {n}(으)로 넘어갔습니다"),
            (Some(n), None) => format!("{n}이(가) {sys_name} 성계의 패권을 장악했습니다"),
            (None, Some(p)) => format!("{p}이(가) {sys_name} 성계에 대한 영향력을 잃었습니다"),
            (None, None) => return,
        };
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.toast(p, text.clone());
            self.send_influence(p);
        }
    }

    /// Once an hour: influence fades; a controller that falls below half the threshold loses its hold.
    pub(super) fn decay_influence(&mut self) {
        let t = self.tuning.social;
        for m in self.store.data.influence.values_mut() {
            for v in m.values_mut() {
                *v *= t.influence_decay_hourly;
            }
            m.retain(|_, v| *v >= 0.1);
        }
        self.store.data.influence.retain(|_, m| !m.is_empty());
        let lost: Vec<SystemId> = self.store.data.controllers.iter().filter(|(s, a)| self.influence(**s, **a) < t.control_threshold * 0.5).map(|(s, _)| *s).collect();
        for s in lost {
            self.set_controller(s, None);
        }
        self.collect_upkeep();
        self.store.dirty = true;
    }

    /// Hourly: each alliance's systems pay into its treasury (`system_income`), then upkeep is due; one
    /// that cannot pay abandons its weakest system — or, with `abandon_until_paid`, as many as it takes.
    fn collect_upkeep(&mut self) {
        let t = self.tuning.social;
        let ids: Vec<AllianceId> = self.store.data.alliances.iter().map(|a| a.id).collect();
        for a in ids {
            let mut held = self.held_by(a);
            let income = t.system_income * held.len() as u64;
            if let Some(al) = self.store.data.alliances.iter_mut().find(|x| x.id == a) {
                al.treasury += income;
            }
            loop {
                let cost = self.upkeep(held.len());
                if cost == 0 {
                    break;
                }
                let Some(al) = self.store.data.alliances.iter_mut().find(|x| x.id == a) else { break };
                if al.treasury >= cost {
                    al.treasury -= cost;
                    break;
                }
                if !t.abandon_until_paid {
                    al.treasury = 0;
                }
                let (tag, leader) = (al.tag.clone(), al.leader);
                let Some(weakest) = held.iter().copied().min_by(|x, y| self.influence(*x, a).total_cmp(&self.influence(*y, a)).then(x.cmp(y))) else { break };
                // Abandoned: the garrison leaves and half the standing goes with it — and, with
                // `abandon_until_paid`, at least to below the threshold, or the same alliance takes it back
                // at once and the abandonment does nothing.
                if let Some(v) = self.store.data.influence.get_mut(&weakest).and_then(|m| m.get_mut(&a)) {
                    *v *= 0.5;
                    if t.abandon_until_paid {
                        *v = v.min(t.control_threshold * 0.9);
                    }
                }
                self.change_controller(weakest, None, Some("금고 부족"));
                self.update_control(weakest);
                self.toast(leader, format!("[{tag}] 금고가 바닥나 유지비 {cost} CR을 내지 못해 {}에서 철수했습니다", galaxy::system(weakest).name));
                if !t.abandon_until_paid {
                    break;
                }
                held.retain(|s| *s != weakest);
            }
        }
    }

    /// Standings per system for the alliance window and the map.
    pub fn standings(&self) -> Vec<SystemStanding> {
        galaxy::systems()
            .iter()
            .map(|s| {
                let tag = |a: AllianceId| self.alliance(a).map(|x| x.tag.clone());
                let mut standings: Vec<(AllianceId, String, f32)> = self.store.data.influence.get(&s.id).into_iter().flatten().filter_map(|(a, v)| Some((*a, tag(*a)?, (*v * 10.0).round() / 10.0))).collect();
                standings.sort_by(|x, y| y.2.total_cmp(&x.2).then(x.0.cmp(&y.0)));
                let controller = self.controller(s.id).and_then(|a| Some((a, tag(a)?)));
                let batteries = controller.as_ref().map(|_| {
                    let b = self.battery(s.id).unwrap_or_default();
                    let hp = self.tuning.social.station_guard.hp * self.tuning.social.station_guard.level_k(b.level);
                    let integrity = if hp > 0.0 { (1.0 - b.damage / hp).clamp(0.0, 1.0) } else { 1.0 };
                    (b.level, (integrity * 100.0).round() / 100.0, b.silenced)
                });
                SystemStanding { system: s.id, controller, standings, batteries }
            })
            .collect()
    }

    pub(super) fn send_influence(&mut self, pid: PlayerId) {
        let systems = self.standings();
        self.send_player(pid, ServerMsg::Influence { systems, threshold: self.tuning.social.control_threshold });
    }
}
