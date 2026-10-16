//! Alliances: player-founded factions. Members never hurt each other; alliances at war
//! may fight anywhere outside Core space without it counting as a crime.

use super::*;
use crate::store::Alliance;
use history::DomainEvent;

pub const ALLIANCE_COST: u64 = 1000;

impl Game {
    pub fn alliance_info(&self, a: &Alliance) -> AllianceInfo {
        let name = |p: &PlayerId| self.store.profile(*p).map(|x| x.name.clone()).unwrap_or_default();
        AllianceInfo { id: a.id, name: a.name.clone(), tag: a.tag.clone(), leader: name(&a.leader), members: a.members.iter().map(name).collect(), wars: a.wars.clone(), treasury: a.treasury }
    }

    pub fn send_alliances(&mut self, pid: PlayerId) {
        let list: Vec<AllianceInfo> = self.store.data.alliances.iter().map(|a| self.alliance_info(a)).collect();
        let mine = self.alliance_of(pid);
        self.send_player(pid, ServerMsg::Alliances { mine, list });
    }

    fn broadcast_alliances(&mut self) {
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.send_alliances(p);
        }
    }

    /// Re-stamp alliance id and tagged names on every online entity of these players (and their escorts).
    fn refresh_members(&mut self, pids: &[PlayerId]) {
        for &pid in pids {
            let alliance = self.alliance_of(pid);
            if self.store.profile(pid).is_none() {
                continue;
            }
            let name = self.display_name(pid);
            for sys in self.systems.values_mut() {
                for e in sys.ents.values_mut() {
                    if e.owner == Some(pid) && matches!(e.kind, EntityKind::Player | EntityKind::Escort) {
                        e.alliance = alliance;
                        if e.kind == EntityKind::Player {
                            e.name = name.clone();
                        }
                    }
                }
            }
        }
        self.store.dirty = true;
    }

    pub(super) fn alliance_create(&mut self, pid: PlayerId, name: String, tag: String) {
        let name = name.trim().to_string();
        let tag: String = tag.trim().to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if self.alliance_of(pid).is_some() {
            return self.toast(pid, "이미 동맹 소속입니다");
        }
        if !(2..=24).contains(&name.chars().count()) || !(2..=5).contains(&tag.len()) {
            return self.toast(pid, "동맹 이름 2~24자, 태그 영문/숫자 2~5자");
        }
        if self.store.data.alliances.iter().any(|a| a.name.eq_ignore_ascii_case(&name) || a.tag == tag) {
            return self.toast(pid, "같은 이름이나 태그의 동맹이 이미 있습니다");
        }
        let paid = self.store.profile_mut(pid).is_some_and(|p| p.wallet.pay(&shared::ships::Materials { credits: ALLIANCE_COST, ..Default::default() }));
        if !paid {
            return self.toast(pid, format!("동맹 창설 비용 {ALLIANCE_COST} CR이 부족합니다"));
        }
        self.store.data.next_alliance += 1;
        let id = self.store.data.next_alliance;
        self.store.data.alliances.push(Alliance { id, name: name.clone(), tag: tag.clone(), leader: pid, members: vec![pid], wars: vec![], treasury: 0 });
        let founder = self.store.profile(pid).unwrap().name.clone();
        self.history.record(self.unix, DomainEvent::AllianceFounded { alliance: format!("[{tag}] {name}"), founder });
        self.refresh_members(&[pid]);
        self.toast(pid, format!("동맹 [{tag}] {name} 창설!"));
        self.push_profile(pid);
        self.broadcast_alliances();
    }

    pub(super) fn alliance_invite(&mut self, pid: PlayerId, name: &str) {
        let Some(aid) = self.alliance_of(pid) else {
            return self.toast(pid, "동맹에 소속되어 있지 않습니다");
        };
        if self.alliance(aid).map(|a| a.leader) != Some(pid) {
            return self.toast(pid, "동맹 맹주만 초대할 수 있습니다");
        }
        let Some(target) = self.store.find_by_name(name) else {
            return self.toast(pid, "그런 함장은 없습니다");
        };
        if !self.conn_of.contains_key(&target) || self.alliance_of(target).is_some() {
            return self.toast(pid, "초대할 수 없습니다 (오프라인이거나 이미 동맹 소속)");
        }
        let c = self.conn_of[&target];
        self.sessions.get_mut(&c).unwrap().alliance_invite = Some(aid);
        let a = self.alliance(aid).unwrap();
        let msg = ServerMsg::AllianceInvite { from: self.store.profile(pid).unwrap().name.clone(), alliance: aid, name: format!("[{}] {}", a.tag, a.name) };
        self.send_player(target, msg);
        self.toast(pid, format!("{name}에게 동맹 초대를 보냈습니다"));
    }

    pub(super) fn alliance_accept(&mut self, conn: ConnId, pid: PlayerId, aid: AllianceId) {
        if self.sessions[&conn].alliance_invite != Some(aid) || self.alliance(aid).is_none() {
            return self.toast(pid, "유효한 동맹 초대가 없습니다");
        }
        if self.alliance_of(pid).is_some() {
            return self.toast(pid, "먼저 현재 동맹을 탈퇴하세요");
        }
        self.sessions.get_mut(&conn).unwrap().alliance_invite = None;
        let a = self.store.data.alliances.iter_mut().find(|a| a.id == aid).unwrap();
        a.members.push(pid);
        let members = a.members.clone();
        let label = format!("[{}] {}", a.tag, a.name);
        self.refresh_members(&[pid]);
        let me = self.store.profile(pid).unwrap().name.clone();
        for m in members {
            self.toast(m, format!("{me}이(가) {label}에 합류했습니다"));
        }
        self.broadcast_alliances();
    }

    pub(super) fn alliance_leave(&mut self, pid: PlayerId) {
        let Some(aid) = self.alliance_of(pid) else { return };
        let a = self.store.data.alliances.iter_mut().find(|a| a.id == aid).unwrap();
        a.members.retain(|m| *m != pid);
        if a.members.is_empty() {
            self.store.data.alliances.retain(|a| a.id != aid);
            for other in &mut self.store.data.alliances {
                other.wars.retain(|w| *w != aid);
            }
        } else if a.leader == pid {
            a.leader = a.members[0];
        }
        self.refresh_members(&[pid]);
        self.toast(pid, "동맹을 탈퇴했습니다");
        self.broadcast_alliances();
    }

    /// Leaders declare war (`war = true`) or make peace. Wars are symmetric.
    pub(super) fn alliance_war(&mut self, pid: PlayerId, target: AllianceId, war: bool) {
        let Some(aid) = self.alliance_of(pid) else {
            return self.toast(pid, "동맹에 소속되어 있지 않습니다");
        };
        if self.alliance(aid).map(|a| a.leader) != Some(pid) {
            return self.toast(pid, "동맹 맹주만 전쟁과 강화를 결정할 수 있습니다");
        }
        if target == aid || self.alliance(target).is_none() {
            return self.toast(pid, "잘못된 대상 동맹입니다");
        }
        let already = self.at_war(Some(aid), Some(target));
        if war == already {
            return self.toast(pid, if war { "이미 전쟁 중입니다" } else { "전쟁 중이 아닙니다" });
        }
        for (x, y) in [(aid, target), (target, aid)] {
            let a = self.store.data.alliances.iter_mut().find(|a| a.id == x).unwrap();
            if war {
                a.wars.push(y);
            } else {
                a.wars.retain(|w| *w != y);
            }
        }
        let label = |g: &Game, id| g.alliance(id).map(|a| format!("[{}] {}", a.tag, a.name)).unwrap_or_default();
        let (me, them) = (label(self, aid), label(self, target));
        let by = self.store.profile(pid).unwrap().name.clone();
        if war {
            self.history.record(self.unix, DomainEvent::WarDeclared { aggressor: me.clone(), target: them.clone(), by });
        } else {
            self.history.record(self.unix, DomainEvent::PeaceMade { a: me.clone(), b: them.clone(), by });
        }
        let text = if war { format!("⚔ {me}이(가) {them}에 선전포고! CORE를 제외한 모든 성계에서 교전 가능") } else { format!("☮ {me}과(와) {them} 강화 성립") };
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.toast(p, text.clone());
        }
        self.store.dirty = true;
        self.broadcast_alliances();
    }
}
