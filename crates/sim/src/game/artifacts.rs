//! Archaeology (design doc §21): Archaeologists dig named artifacts out of ruins and gates; at a
//! station they are appraised, sold or donated to the museum; a captain's artifacts go down with
//! the ship, and whoever investigates the wreck recovers them (the Lost Artifact Hunt).
//! Every step is written into the artifact's provenance and the chronicle.

use super::*;
use history::DomainEvent;
use shared::artifacts::{self, Artifact, ArtifactId, Holder};
use shared::chronicle::EventId;
use shared::galaxy::Poi;

impl Game {
    /// Artifacts a player holds, in id order.
    pub fn artifacts_of(&self, pid: PlayerId) -> Vec<&Artifact> {
        self.store.data.artifacts.values().filter(|a| a.holder == Holder::Player(pid)).collect()
    }

    pub(super) fn push_artifacts(&mut self, pid: PlayerId) {
        let held = self.artifacts_of(pid).into_iter().cloned().collect();
        self.send_player(pid, ServerMsg::Artifacts { held });
    }

    /// After an Archaeologist scans a ruin or gate: its artifact for today, if nobody took it yet.
    pub(super) fn unearth(&mut self, pid: PlayerId, sys: SystemId, poi: &Poi) {
        let Some(prof) = self.store.profile(pid).cloned() else { return };
        if prof.profession != Profession::Archaeologist || !artifacts::yields_artifacts(poi.kind) {
            return;
        }
        let day = missions::day_index(self.unix);
        if self.store.data.excavated.contains(&(sys, poi.id, day)) {
            return self.toast(pid, format!("{}의 오늘 발굴물은 이미 누군가 가져갔습니다", poi.name));
        }
        self.store.data.next_artifact += 1;
        let id = self.store.data.next_artifact;
        let art = artifacts::excavate(id, sys, &poi.name, poi.kind, poi.id, day, pid, self.unix);
        let label = art.label();
        let culture = art.culture.name().to_string();
        self.store.data.excavated.push((sys, poi.id, day));
        self.store.data.artifacts.insert(id, art);
        self.store.dirty = true;
        self.history.record(self.unix, DomainEvent::ArtifactUnearthed { artifact_id: id, artifact: label.clone(), culture, player: prof.name.clone(), ship: prof.ship.name.clone(), system: sys, site: poi.name.clone() });
        self.toast(pid, format!("유물 발굴: {label} — 정거장에서 감정할 수 있습니다"));
        self.push_artifacts(pid);
    }

    /// The flagship went down: everything it carried stays in the wreck (`wreck` = store index).
    pub(super) fn sink_artifacts(&mut self, pid: PlayerId, sys: SystemId, wreck: usize, wreck_event: EventId) {
        let ids: Vec<ArtifactId> = self.artifacts_of(pid).iter().map(|a| a.id).collect();
        if ids.is_empty() {
            return;
        }
        let name = self.store.profile(pid).map(|p| p.name.clone()).unwrap_or_default();
        let sys_name = galaxy::system(sys).name;
        for id in &ids {
            let unix = self.unix;
            let a = self.store.data.artifacts.get_mut(id).unwrap();
            a.holder = Holder::Wreck(wreck);
            a.note(unix, format!("{name}의 함선과 함께 {sys_name}에서 가라앉았다."));
            let label = a.label();
            self.history.record(self.unix, DomainEvent::ArtifactLost { artifact_id: *id, artifact: label, player: name.clone(), system: sys, wreck_event });
        }
        self.store.data.wrecks[wreck].artifacts.extend(ids);
        self.store.dirty = true;
        self.toast(pid, "함께 싣고 있던 유물이 잔해와 함께 가라앉았습니다");
        self.push_artifacts(pid);
    }

    /// Investigating a wreck: take every artifact that went down with it.
    pub(super) fn recover_artifacts(&mut self, pid: PlayerId, sys: SystemId, wreck: usize) -> usize {
        let ids = std::mem::take(&mut self.store.data.wrecks[wreck].artifacts);
        if ids.is_empty() {
            return 0;
        }
        let name = self.store.profile(pid).map(|p| p.name.clone()).unwrap_or_default();
        let (victim, wreck_event) = (self.store.data.wrecks[wreck].victim.clone(), self.store.data.wrecks[wreck].event);
        let sys_name = galaxy::system(sys).name;
        for id in &ids {
            let unix = self.unix;
            let Some(a) = self.store.data.artifacts.get_mut(id) else { continue };
            a.holder = Holder::Player(pid);
            a.note(unix, format!("{name}이(가) {sys_name}에서 {victim}의 잔해를 조사해 회수했다."));
            let label = a.label();
            self.history.record(self.unix, DomainEvent::ArtifactRecovered { artifact_id: *id, artifact: label, player: name.clone(), from: victim.clone(), system: sys, wreck_event });
        }
        self.store.dirty = true;
        self.push_artifacts(pid);
        ids.len()
    }

    /// Station services: appraise, sell or donate an artifact you hold (docked).
    pub(super) fn artifact_service(&mut self, conn: ConnId, pid: PlayerId, sys: SystemId, eid: Option<EntityId>, id: ArtifactId, action: ArtifactAction) {
        if !self.require_docked(pid, sys, eid) {
            return;
        }
        let Some(a) = self.store.data.artifacts.get(&id).filter(|a| a.holder == Holder::Player(pid)).cloned() else {
            return self.error(conn, "가지고 있지 않은 유물입니다");
        };
        let t = self.tuning.artifacts.clone();
        let prof = self.store.profile(pid).unwrap().clone();
        let sys_name = galaxy::system(sys).name;
        let unix = self.unix;
        match action {
            ArtifactAction::Appraise => {
                if a.appraised {
                    return self.toast(pid, "이미 감정한 유물입니다");
                }
                let cost = if prof.profession == Profession::Archaeologist { 0 } else { t.appraisal_cost };
                if prof.wallet.credits < cost {
                    return self.error(conn, format!("감정 비용 {cost} CR이 부족합니다"));
                }
                if let Some(p) = self.store.profile_mut(pid) {
                    p.wallet.credits -= cost;
                }
                let art = self.store.data.artifacts.get_mut(&id).unwrap();
                art.appraised = true;
                art.note(unix, format!("{sys_name} 정거장에서 감정되었다: {} ({} 유물, 감정가 {} CR).", a.name, a.culture.name(), a.value));
                self.history.record(unix, DomainEvent::ArtifactAppraised { artifact_id: id, artifact: a.name.clone(), player: prof.name.clone(), value: a.value });
                self.toast(pid, format!("감정 결과: {} — {} 유물, 감정가 {} CR{}", a.name, a.culture.name(), a.value, if cost > 0 { format!(" (비용 {cost} CR)") } else { String::new() }));
            }
            ArtifactAction::Sell => {
                let credits = if a.appraised { a.value } else { (a.value as f32 * t.unappraised_sale) as u64 };
                if let Some(p) = self.store.profile_mut(pid) {
                    p.wallet.credits += credits;
                }
                let art = self.store.data.artifacts.get_mut(&id).unwrap();
                art.holder = Holder::Sold(sys);
                art.note(unix, format!("{}이(가) {sys_name} 정거장에서 수집가에게 {credits} CR에 팔았다.", prof.name));
                let label = art.label();
                self.history.record(unix, DomainEvent::ArtifactSold { artifact_id: id, artifact: label.clone(), player: prof.name.clone(), credits, system: sys });
                self.toast(pid, format!("{label} 판매: {credits} CR{}", if a.appraised { "" } else { " (미감정 가격)" }));
            }
            ArtifactAction::Donate => {
                let fee = (a.value as f32 * t.donation_share) as u64;
                if let Some(p) = self.store.profile_mut(pid) {
                    p.wallet.credits += fee;
                    p.reputation += t.donation_reputation;
                }
                let art = self.store.data.artifacts.get_mut(&id).unwrap();
                art.holder = Holder::Museum(sys);
                art.note(unix, format!("{}이(가) {sys_name} 정거장 박물관에 기증했다.", prof.name));
                let label = art.label();
                self.history.record(unix, DomainEvent::ArtifactDonated { artifact_id: id, artifact: label.clone(), player: prof.name.clone(), system: sys });
                self.toast(pid, format!("{label}을(를) 박물관에 기증했습니다 — 평판 +{}, 사례금 {fee} CR", t.donation_reputation));
            }
        }
        self.store.dirty = true;
        self.push_artifacts(pid);
        self.push_profile(pid);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ArtifactAction {
    Appraise,
    Sell,
    Donate,
}
