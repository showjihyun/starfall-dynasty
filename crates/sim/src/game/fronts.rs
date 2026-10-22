//! War-zone fronts during a battle: every `fronts.report_secs` each war zone in a battle counts,
//! per front (`galaxy::fronts`, a ship is on the nearest), each side's ships, their hull + shield
//! left and the captains it has lost there, and sends that one report to everyone in the system.
//! The counting runs for all such systems in parallel; it reads only the system and a side table.

use super::*;
use rayon::prelude::*;

/// Per front: (side, ships, hull + shield left), in side order.
type Tally = BTreeMap<u8, BTreeMap<SideId, (u32, f32)>>;

/// Counts one system's fronts: every captain, escort and pirate on the front nearest to it.
fn tally(sys: &Sys, sides: &BTreeMap<PlayerId, SideId>) -> Tally {
    let mut t: Tally = BTreeMap::new();
    for e in sys.ents.values() {
        let side = match e.kind {
            EntityKind::Player | EntityKind::Escort => e.owner.and_then(|p| sides.get(&p)).copied(),
            EntityKind::Npc(_) => Some(SideId::Pirates),
            _ => None,
        };
        let (Some(side), Some(f)) = (side, galaxy::front_of(&sys.fronts, e.pos)) else { continue };
        let v = t.entry(f as u8).or_default().entry(side).or_default();
        v.0 += 1;
        v.1 += e.hull.max(0.0) + e.shield.max(0.0);
    }
    t
}

impl Game {
    /// A captain's side in front reports: alliance, else fleet, else alone (as engagement sides).
    pub(super) fn side_id(&self, pid: PlayerId) -> SideId {
        match (self.alliance_of(pid), self.fleet_of(pid)) {
            (Some(a), _) => SideId::Alliance(a),
            (None, Some(f)) => SideId::Fleet(f),
            _ => SideId::Captain(pid),
        }
    }

    fn side_name(&self, side: SideId) -> String {
        match side {
            SideId::Alliance(a) => self.alliance(a).map(|a| format!("[{}] {}", a.tag, a.name)).unwrap_or_default(),
            SideId::Fleet(f) => self.fleets.get(&f).and_then(|fl| self.store.profile(fl.leader)).map(|p| format!("{}의 함대", p.name)).unwrap_or_default(),
            SideId::Captain(p) => self.store.profile(p).map(|p| p.name.clone()).unwrap_or_default(),
            SideId::Pirates => "해적".into(),
            SideId::Others => "그 외".into(),
        }
    }

    /// A captain died: count the loss on the front where it happened, while a battle is on.
    pub(super) fn front_loss(&mut self, sys: SystemId, e: &Ent) {
        let Some(pid) = e.owner.filter(|_| e.kind == EntityKind::Player) else { return };
        let side = self.side_id(pid);
        let s = self.systems.get_mut(&sys).unwrap();
        if s.engagement.is_some()
            && let Some(f) = galaxy::front_of(&s.fronts, e.pos)
        {
            *s.front_losses.entry((f as u8, side)).or_default() += 1;
        }
    }

    /// Every report interval: the standing on each front of every war zone in a battle.
    pub(super) fn tick_fronts(&mut self) {
        let every = (self.tuning.galaxy.fronts.report_secs * TICK_HZ).round().max(1.0) as u64;
        if !self.tick.is_multiple_of(every) {
            return;
        }
        let busy: Vec<SystemId> = self.systems.values().filter(|s| s.engagement.is_some() && !s.fronts.is_empty()).map(|s| s.id).collect();
        if busy.is_empty() {
            return;
        }
        let _p = shared::prof::span("fronts");
        let sides: BTreeMap<PlayerId, SideId> = self.sessions.values().filter_map(|s| s.player).map(|p| (p, self.side_id(p))).collect();
        let tallies: Vec<(SystemId, Tally)> = busy.par_iter().map(|id| (*id, tally(&self.systems[id], &sides))).collect();
        for (sys, mut t) in tallies {
            for (f, side) in self.systems[&sys].front_losses.keys() {
                t.entry(*f).or_default().entry(*side).or_default();
            }
            let fronts: Vec<FrontReport> = t
                .into_iter()
                .map(|(f, by_side)| {
                    let mut sides: Vec<FrontSide> = by_side
                        .into_iter()
                        .map(|(side, (ships, strength))| FrontSide { side, name: self.side_name(side), ships, strength: strength.round() as u32, lost: self.systems[&sys].front_losses.get(&(f, side)).copied().unwrap_or(0) })
                        .collect();
                    sides.sort_by(|a, b| b.strength.cmp(&a.strength).then(a.side.cmp(&b.side)));
                    let keep = self.tuning.galaxy.fronts.max_sides.max(1);
                    if sides.len() > keep {
                        let rest = sides.split_off(keep - 1);
                        let sum = |k: fn(&FrontSide) -> u32| rest.iter().map(k).sum();
                        sides.push(FrontSide { side: SideId::Others, name: format!("외 {}개 세력", rest.len()), ships: sum(|s| s.ships), strength: sum(|s| s.strength), lost: sum(|s| s.lost) });
                    }
                    FrontReport { front: f, sides }
                })
                .collect();
            for (c, _) in self.players_in(sys) {
                self.send(c, ServerMsg::Fronts { fronts: fronts.clone() });
            }
        }
    }

    /// The battle in `sys` is over: clear its fronts on every screen.
    pub(super) fn end_fronts(&mut self, sys: SystemId) {
        let s = self.systems.get_mut(&sys).unwrap();
        s.front_losses.clear();
        if s.fronts.is_empty() {
            return;
        }
        for (c, _) in self.players_in(sys) {
            self.send(c, ServerMsg::Fronts { fronts: vec![] });
        }
    }
}
