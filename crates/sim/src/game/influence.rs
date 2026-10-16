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
        if points <= 0.0 || galaxy::system(sys).tier == RiskTier::Core {
            return;
        }
        *self.store.data.influence.entry(sys).or_default().entry(a).or_default() += points;
        self.store.dirty = true;
        self.update_control(sys);
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
        self.history.record(self.unix, DomainEvent::SystemControlChanged { system: sys, alliance: name_of(self, new), previous: previous.clone() });
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
        self.store.dirty = true;
    }

    /// Standings per system for the alliance window and the map.
    pub fn standings(&self) -> Vec<SystemStanding> {
        galaxy::SYSTEMS
            .iter()
            .map(|s| {
                let tag = |a: AllianceId| self.alliance(a).map(|x| x.tag.clone());
                let mut standings: Vec<(AllianceId, String, f32)> = self.store.data.influence.get(&s.id).into_iter().flatten().filter_map(|(a, v)| Some((*a, tag(*a)?, (*v * 10.0).round() / 10.0))).collect();
                standings.sort_by(|x, y| y.2.total_cmp(&x.2).then(x.0.cmp(&y.0)));
                SystemStanding { system: s.id, controller: self.controller(s.id).and_then(|a| Some((a, tag(a)?))), standings }
            })
            .collect()
    }

    pub(super) fn send_influence(&mut self, pid: PlayerId) {
        let systems = self.standings();
        self.send_player(pid, ServerMsg::Influence { systems, threshold: self.tuning.social.control_threshold });
    }
}
