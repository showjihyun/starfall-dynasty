//! Player contracts (design doc Phase 3, §11.8): post with escrow, accept, fulfil (checked by the
//! simulation), cancel, expire — and betrayal, when one party turns on the other mid-contract.

use super::*;
use history::DomainEvent;
use shared::contracts::{Contract, ContractId, ContractKind, ContractState};

impl Game {
    /// The board for one captain: open contracts and every contract they are party to (newest first).
    pub(super) fn send_contracts(&mut self, pid: PlayerId) {
        let list: Vec<Contract> = self
            .store
            .data
            .contracts
            .values()
            .rev()
            .filter(|c| c.state == ContractState::Open || c.employer == pid || c.contractor.as_ref().is_some_and(|(p, _)| *p == pid))
            .take(40)
            .cloned()
            .collect();
        self.send_player(pid, ServerMsg::Contracts { list });
    }

    fn contract_changed(&mut self, id: ContractId) {
        let Some(c) = self.store.data.contracts.get(&id).cloned() else { return };
        self.store.dirty = true;
        self.send_contracts(c.employer);
        if let Some((p, _)) = c.contractor {
            self.send_contracts(p);
        }
    }

    pub(super) fn post_contract(&mut self, conn: ConnId, pid: PlayerId, kind: ContractKind, reward: u64, hours: u32) {
        let t = self.tuning.social;
        let prof = self.store.profile(pid).unwrap().clone();
        if reward < t.min_reward {
            return self.error(conn, format!("보상은 {} CR 이상이어야 합니다", t.min_reward));
        }
        if prof.wallet.credits < reward {
            return self.error(conn, "보상을 맡길 크레딧이 부족합니다");
        }
        if hours == 0 || hours > t.max_hours {
            return self.error(conn, format!("기한은 1~{}시간입니다", t.max_hours));
        }
        let running = self.store.data.contracts.values().filter(|c| c.employer == pid && matches!(c.state, ContractState::Open | ContractState::Accepted)).count();
        if running >= t.max_open as usize {
            return self.error(conn, format!("진행 중인 계약은 {}건까지입니다", t.max_open));
        }
        match &kind {
            ContractKind::Hunt { target } => {
                if self.store.find_by_name(target).and_then(|p| self.store.profile(p)).is_none() {
                    return self.error(conn, format!("{target}이라는 함장은 없습니다"));
                }
                if target.eq_ignore_ascii_case(&prof.name) {
                    return self.error(conn, "자신을 표적으로 삼을 수 없습니다");
                }
            }
            ContractKind::Delivery { units, system, .. } => {
                if *units == 0 || galaxy::SYSTEMS.iter().all(|s| s.id != *system) {
                    return self.error(conn, "운송 조건이 올바르지 않습니다");
                }
            }
        }
        if let Some(p) = self.store.profile_mut(pid) {
            p.wallet.credits -= reward;
        }
        self.store.data.next_contract += 1;
        let id = self.store.data.next_contract;
        let terms = kind.describe();
        self.store.data.contracts.insert(id, Contract { id, employer: pid, employer_name: prof.name.clone(), kind, reward, posted_unix: self.unix, expires_unix: self.unix + hours as u64 * 3600, state: ContractState::Open, contractor: None, signed_event: None });
        self.toast(pid, format!("계약 게시: {terms} — 보상 {reward} CR을 공증인에게 맡겼습니다"));
        self.push_profile(pid);
        self.contract_changed(id);
    }

    pub(super) fn accept_contract(&mut self, conn: ConnId, pid: PlayerId, id: ContractId) {
        let Some(c) = self.store.data.contracts.get(&id).cloned() else { return self.error(conn, "없는 계약입니다") };
        let me = self.store.profile(pid).unwrap().name.clone();
        if c.state != ContractState::Open {
            return self.error(conn, "이미 체결되었거나 끝난 계약입니다");
        }
        if c.employer == pid {
            return self.error(conn, "자신의 계약은 수락할 수 없습니다");
        }
        if matches!(&c.kind, ContractKind::Hunt { target } if target.eq_ignore_ascii_case(&me)) {
            return self.error(conn, "자신을 노리는 계약입니다");
        }
        let terms = c.kind.describe();
        let signed = self.history.record(self.unix, DomainEvent::ContractSigned { contract_id: id, employer: c.employer_name.clone(), contractor: me.clone(), terms: terms.clone(), reward: c.reward });
        let k = self.store.data.contracts.get_mut(&id).unwrap();
        k.state = ContractState::Accepted;
        k.contractor = Some((pid, me.clone()));
        k.signed_event = signed;
        self.toast(pid, format!("계약 체결: {terms} (보상 {} CR). 계약 중에 의뢰인을 공격하면 배신입니다.", c.reward));
        self.toast(c.employer, format!("{me}이(가) 계약을 수락했습니다: {terms}"));
        self.contract_changed(id);
    }

    pub(super) fn cancel_contract(&mut self, conn: ConnId, pid: PlayerId, id: ContractId) {
        let Some(c) = self.store.data.contracts.get(&id).cloned() else { return self.error(conn, "없는 계약입니다") };
        if c.employer != pid || c.state != ContractState::Open {
            return self.error(conn, "수락되기 전의 자기 계약만 철회할 수 있습니다");
        }
        self.settle(id, ContractState::Cancelled, c.employer);
        self.toast(pid, format!("계약 철회 — {} CR 환불", c.reward));
    }

    /// Delivery: docked at the destination station with the goods in the hold.
    pub(super) fn deliver_contract(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, id: ContractId) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let Some(c) = self.store.data.contracts.get(&id).cloned() else { return self.error(conn, "없는 계약입니다") };
        let ContractKind::Delivery { ore, units, system } = c.kind.clone() else { return self.error(conn, "운송 계약이 아닙니다") };
        if !c.binding() || c.contractor.as_ref().is_none_or(|(p, _)| *p != pid) {
            return self.error(conn, "당신이 맡은 진행 중인 계약이 아닙니다");
        }
        if sys != system {
            return self.error(conn, format!("{} 정거장으로 가져가야 합니다", galaxy::system(system).name));
        }
        let have = self.store.profile(pid).and_then(|p| p.cargo.get(ore.name()).copied()).unwrap_or(0);
        if have < units {
            return self.error(conn, format!("{} {units}단위가 필요합니다 (보유 {have})", ore.name()));
        }
        if let Some(p) = self.store.profile_mut(pid) {
            let left = have - units;
            if left == 0 {
                p.cargo.remove(ore.name());
            } else {
                p.cargo.insert(ore.name().to_string(), left);
            }
        }
        if let Some(e) = self.store.profile_mut(c.employer) {
            *e.cargo.entry(ore.name().to_string()).or_default() += units;
        }
        self.fulfil(id, sys);
    }

    /// A captain destroyed another: does it complete a hunt contract?
    pub(super) fn contract_kill(&mut self, killer: PlayerId, victim_name: &str, sys: SystemId) {
        let ids: Vec<ContractId> = self
            .store
            .data
            .contracts
            .values()
            .filter(|c| c.binding() && c.contractor.as_ref().is_some_and(|(p, _)| *p == killer) && matches!(&c.kind, ContractKind::Hunt { target } if target.eq_ignore_ascii_case(victim_name)))
            .map(|c| c.id)
            .collect();
        for id in ids {
            self.fulfil(id, sys);
        }
    }

    /// `attacker` hit `victim`'s ships: if they are parties to a running contract, it is a betrayal.
    pub(super) fn contract_hit(&mut self, attacker: PlayerId, victim: PlayerId, sys: SystemId) {
        if attacker == victim {
            return;
        }
        let ids: Vec<ContractId> = self.store.data.contracts.values().filter(|c| c.binding() && c.parties().is_some_and(|(e, k)| (e, k) == (attacker, victim) || (k, e) == (attacker, victim))).map(|c| c.id).collect();
        for id in ids {
            let c = self.store.data.contracts[&id].clone();
            let betrayer = self.store.profile(attacker).map(|p| p.name.clone()).unwrap_or_default();
            let betrayed = self.store.profile(victim).map(|p| p.name.clone()).unwrap_or_default();
            self.history.record(self.unix, DomainEvent::ContractBetrayed { contract_id: id, betrayer: betrayer.clone(), betrayed: betrayed.clone(), terms: c.kind.describe(), system: sys });
            // The escrow goes to the wronged party; the betrayer pays in reputation.
            self.settle(id, ContractState::Breached { by: betrayer.clone() }, victim);
            if let Some(p) = self.store.profile_mut(attacker) {
                p.reputation += self.tuning.social.betrayal_reputation;
            }
            self.toast(attacker, format!("⚠ 계약 상대 {betrayed}을(를) 공격해 계약이 깨졌습니다 — 배신은 연대기에 기록됩니다 (평판 {})", self.tuning.social.betrayal_reputation));
            self.toast(victim, format!("⚠ {betrayer}이(가) 계약을 깨고 당신을 공격했습니다 — 보상 {} CR은 당신에게 돌아갑니다", c.reward));
            self.push_profile(attacker);
        }
    }

    /// Once a minute: deadlines that passed refund the employer.
    pub(super) fn expire_contracts(&mut self) {
        let unix = self.unix;
        let due: Vec<(ContractId, PlayerId)> = self.store.data.contracts.values().filter(|c| matches!(c.state, ContractState::Open | ContractState::Accepted) && c.expires_unix <= unix).map(|c| (c.id, c.employer)).collect();
        for (id, employer) in due {
            self.settle(id, ContractState::Expired, employer);
            self.toast(employer, "계약 기한이 지나 보상이 환불되었습니다");
        }
    }

    fn fulfil(&mut self, id: ContractId, sys: SystemId) {
        let c = self.store.data.contracts[&id].clone();
        let Some((cp, cname)) = c.contractor.clone() else { return };
        self.history.record(self.unix, DomainEvent::ContractFulfilled { contract_id: id, employer: c.employer_name.clone(), contractor: cname, terms: c.kind.describe(), reward: c.reward, system: sys });
        self.settle(id, ContractState::Fulfilled, cp);
        if let Some(p) = self.store.profile_mut(cp) {
            p.reputation += self.tuning.social.fulfil_reputation;
        }
        self.toast(cp, format!("계약 완수: {} — {} CR 수령", c.kind.describe(), c.reward));
        self.toast(c.employer, format!("계약이 완수되었습니다: {}", c.kind.describe()));
    }

    /// Ends a contract and pays the escrow to `payee`.
    fn settle(&mut self, id: ContractId, state: ContractState, payee: PlayerId) {
        let reward = {
            let c = self.store.data.contracts.get_mut(&id).unwrap();
            c.state = state;
            c.reward
        };
        if let Some(p) = self.store.profile_mut(payee) {
            p.wallet.credits += reward;
        }
        self.push_profile(payee);
        self.contract_changed(id);
    }
}
