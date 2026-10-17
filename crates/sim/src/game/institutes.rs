//! Player institutions (design doc §24): found a research institute or museum at a station, put
//! artifacts on display, and earn from the captains who come to see them (one visit per captain per
//! day, counted when they dock in its system). Donating also builds the donor alliance's influence.

use super::*;
use history::DomainEvent;
use shared::artifacts::Holder;
use shared::institutes::{Institute, InstituteId, InstituteView};

impl Game {
    pub fn institute_by_name(&self, name: &str) -> Option<&Institute> {
        self.store.data.institutes.values().find(|i| i.name == name)
    }

    pub(super) fn send_institutes(&mut self, pid: PlayerId) {
        let list: Vec<InstituteView> = self
            .store
            .data
            .institutes
            .values()
            .map(|i| InstituteView { institute: i.clone(), exhibits: i.exhibits.iter().filter_map(|a| self.store.data.artifacts.get(a).cloned()).collect() })
            .collect();
        self.send_player(pid, ServerMsg::Institutes { list });
    }

    fn broadcast_institutes(&mut self) {
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.send_institutes(p);
        }
    }

    pub(super) fn found_institute(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, name: String) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let name = name.trim().to_string();
        if !(2..=30).contains(&name.chars().count()) {
            return self.error(conn, "기관 이름은 2~30자입니다");
        }
        if self.store.data.institutes.values().any(|i| i.founder == pid) {
            return self.error(conn, "기관은 함장당 하나만 세울 수 있습니다");
        }
        if self.store.data.institutes.values().any(|i| i.name.eq_ignore_ascii_case(&name)) {
            return self.error(conn, "같은 이름의 기관이 이미 있습니다");
        }
        let cost = self.tuning.social.institute_cost;
        let prof = self.store.profile(pid).unwrap().clone();
        if prof.wallet.credits < cost {
            return self.error(conn, format!("설립 비용 {cost} CR이 부족합니다"));
        }
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits -= cost;
        }
        self.store.data.next_institute += 1;
        let id = self.store.data.next_institute;
        self.store.data.institutes.insert(id, Institute { id, name: name.clone(), founder: pid, founder_name: prof.name.clone(), system: sys, founded_unix: self.unix, exhibits: vec![], visitors: 0 });
        self.store.dirty = true;
        self.history.record(self.unix, DomainEvent::InstituteFounded { institute_id: id, name: name.clone(), founder: prof.name.clone(), system: sys });
        self.toast(pid, format!("「{name}」 개관 (-{cost} CR). 유물을 전시하면 이 성계에 들르는 함장들이 관람객이 됩니다."));
        self.push_profile(pid);
        self.broadcast_institutes();
    }

    pub(super) fn donate_to_institute(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, artifact: u64, institute: InstituteId) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let Some(inst) = self.store.data.institutes.get(&institute).cloned() else { return self.error(conn, "없는 기관입니다") };
        if inst.system != sys {
            return self.error(conn, format!("「{}」은(는) {} 정거장에 있습니다", inst.name, galaxy::system(inst.system).name));
        }
        let Some(a) = self.store.data.artifacts.get(&artifact).filter(|a| a.holder == Holder::Player(pid)).cloned() else {
            return self.error(conn, "가지고 있지 않은 유물입니다");
        };
        let prof = self.store.profile(pid).unwrap().clone();
        let unix = self.unix;
        let label = a.label();
        {
            let art = self.store.data.artifacts.get_mut(&artifact).unwrap();
            art.holder = Holder::Institute(institute);
            art.note(unix, format!("{}이(가) 「{}」에 기증해 전시되었다.", prof.name, inst.name));
        }
        self.store.data.institutes.get_mut(&institute).unwrap().exhibits.push(artifact);
        let rep = self.tuning.social.exhibit_reputation;
        if let Some(p) = self.store.profile_mut(pid) {
            p.reputation += rep;
        }
        self.store.dirty = true;
        self.history.record(unix, DomainEvent::ExhibitAcquired { institute_id: institute, institute: inst.name.clone(), artifact_id: artifact, artifact: label.clone(), donor: prof.name.clone(), system: sys });
        self.gain_influence(pid, sys, self.tuning.social.influence_donation);
        self.toast(pid, format!("{label}을(를) 「{}」에 기증했습니다 (평판 +{rep})", inst.name));
        if inst.founder != pid {
            self.toast(inst.founder, format!("{}이(가) 「{}」에 {label}을(를) 기증했습니다", prof.name, inst.name));
        }
        self.push_artifacts(pid);
        self.push_profile(pid);
        self.broadcast_institutes();
    }

    /// Docking in a system: each of its institutes counts this captain once per day; founders earn.
    pub(super) fn visit_institutes(&mut self, pid: PlayerId, sys: SystemId) {
        let day = missions::day_index(self.unix);
        let here: Vec<InstituteId> = self.store.data.institutes.values().filter(|i| i.system == sys && i.founder != pid && !i.exhibits.is_empty()).map(|i| i.id).collect();
        let t = self.tuning.social;
        for id in here {
            let seen = self.store.data.institute_visits.entry(id).or_default();
            seen.retain(|(_, d)| *d == day);
            if seen.iter().any(|(p, _)| *p == pid) {
                continue;
            }
            seen.push((pid, day));
            let inst = self.store.data.institutes.get_mut(&id).unwrap();
            inst.visitors += 1;
            let (founder, name, n) = (inst.founder, inst.name.clone(), inst.exhibits.len() as u64);
            let fee = t.visit_base + t.visit_per_exhibit * n;
            if let Some(p) = self.store.profile_mut(founder) {
                p.wallet.credits += fee;
            }
            self.push_profile(founder);
            self.toast(pid, format!("「{name}」 관람: 전시 {n}점"));
            self.store.dirty = true;
        }
    }
}
