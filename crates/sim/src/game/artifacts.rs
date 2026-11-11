//! Archaeology (design doc §21): Archaeologists dig named artifacts out of ruins and gates; at a
//! station they are appraised, sold or donated to the museum; a captain's artifacts go down with
//! the ship, and whoever investigates the wreck recovers them (the Lost Artifact Hunt). An appraised
//! artifact can be fitted to the ship instead of sold; sites hidden deep enough give one to anyone.
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

    /// What the captain's fitted artifact does (nothing fitted: no effect).
    pub(super) fn fit_effect(&self, prof: &Profile) -> shared::tuning::FitEffect {
        prof.fitted.and_then(|id| self.store.data.artifacts.get(&id)).and_then(|a| self.tuning.artifacts.fit.get(&a.culture)).cloned().unwrap_or_default()
    }

    /// The captain's flagship numbers: the hull as their people build it, fitted artifact included.
    pub(super) fn ship_stats(&self, prof: &Profile) -> Stats {
        let mut stats = prof.ship.stats_as(prof.species);
        self.fit_effect(prof).apply(&mut stats);
        stats
    }

    /// Apply a changed fit to the flagship in space or at the dock, keeping its hull and shield share.
    fn refit(&mut self, pid: PlayerId, sys: SystemId, eid: Option<EntityId>) {
        let Some(prof) = self.store.profile(pid) else { return };
        let stats = self.ship_stats(prof);
        if let Some(e) = eid.and_then(|e| self.ent_mut(sys, e)) {
            let (hf, sf) = (e.hull / e.stats.max_hull, e.shield / e.stats.max_shield);
            e.stats = stats;
            e.hull = hf * e.stats.max_hull;
            e.shield = sf * e.stats.max_shield;
        }
    }

    /// QA: an appraised artifact of this culture in the captain's hold (`/relic Veyr`).
    pub(super) fn dev_relic(&mut self, pid: PlayerId, culture: &str) {
        let Some(culture) = artifacts::Culture::ALL.into_iter().find(|c| format!("{c:?}") == culture) else {
            return self.toast(pid, "[dev] /relic Human|Veyr|Kharz|Precursor");
        };
        self.store.data.next_artifact += 1;
        let id = self.store.data.next_artifact;
        let mut art = artifacts::excavate(id, 0, "QA", galaxy::PoiKind::Ruin, 0, 0, pid, self.unix);
        art.culture = culture;
        art.appraised = true;
        self.store.data.artifacts.insert(id, art);
        self.store.dirty = true;
        self.push_artifacts(pid);
    }

    /// After a scan: the site's artifact for today, if nobody took it yet. Archaeologists dig ruins and
    /// gates; any captain digs a site hidden deep enough.
    pub(super) fn unearth(&mut self, pid: PlayerId, sys: SystemId, poi: &Poi) {
        let Some(prof) = self.store.profile(pid).cloned() else { return };
        let deep = self.tuning.artifacts.open_dig_depth.is_some_and(|d| self.tuning.galaxy.explore.depth(sys, poi.id, galaxy::system(sys).tier) >= d);
        if !deep && (prof.profession != Profession::Archaeologist || !artifacts::yields_artifacts(poi.kind)) {
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
        if let Some(p) = self.store.profile_mut(pid) {
            p.fitted = None;
        }
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
        if prof.fitted == Some(id) && matches!(action, ArtifactAction::Sell | ArtifactAction::Donate) {
            return self.error(conn, "장착을 해제해야 팔거나 기증할 수 있습니다");
        }
        match action {
            ArtifactAction::Fit => {
                if !a.appraised {
                    return self.error(conn, "감정한 유물만 장착할 수 있습니다");
                }
                if prof.fitted == Some(id) {
                    return self.toast(pid, "이미 장착한 유물입니다");
                }
                if let Some(p) = self.store.profile_mut(pid) {
                    p.fitted = Some(id);
                }
                self.store.data.artifacts.get_mut(&id).unwrap().note(unix, format!("{}이(가) 함선에 장착했다.", prof.name));
                self.refit(pid, sys, eid);
                let effect = self.tuning.artifacts.fit.get(&a.culture).map(|f| f.describe()).unwrap_or_default();
                self.toast(pid, format!("{} 장착 — {effect}", a.name));
            }
            ArtifactAction::Unfit => {
                if prof.fitted != Some(id) {
                    return self.toast(pid, "장착하지 않은 유물입니다");
                }
                if let Some(p) = self.store.profile_mut(pid) {
                    p.fitted = None;
                }
                self.refit(pid, sys, eid);
                self.toast(pid, format!("{} 장착 해제", a.name));
            }
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
                self.gain_influence(pid, sys, self.tuning.social.influence_donation);
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
    Fit,
    Unfit,
}
