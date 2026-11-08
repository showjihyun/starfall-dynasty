//! Alliances: player-founded factions. Members never hurt each other; alliances at war
//! may fight anywhere outside Core space without it counting as a crime.

use super::*;
use crate::store::Alliance;
use history::DomainEvent;

pub const ALLIANCE_COST: u64 = 1000;

impl Game {
    pub fn alliance_info(&self, a: &Alliance) -> AllianceInfo {
        let name = |p: &PlayerId| self.store.profile(*p).map(|x| x.name.clone()).unwrap_or_default();
        AllianceInfo { id: a.id, name: a.name.clone(), tag: a.tag.clone(), leader: name(&a.leader), members: a.members.iter().map(name).collect(), wars: a.wars.clone(), treasury: a.treasury, infamy: (a.infamy * 10.0).round() / 10.0, truces: a.truces.clone(), votes: a.votes.iter().map(|(v, c)| (name(v), name(c))).collect(), hegemon: self.hegemon() == Some(a.id) }
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
        self.store.data.alliances.push(Alliance { id, name: name.clone(), tag: tag.clone(), leader: pid, members: vec![pid], wars: vec![], treasury: 0, infamy: 0.0, truces: vec![], votes: BTreeMap::new() });
        let founder = self.store.profile(pid).unwrap().name.clone();
        self.history.record(self.unix, DomainEvent::AllianceFounded { alliance: format!("[{tag}] {name}"), founder });
        self.refresh_members(&[pid]);
        self.toast(pid, format!("동맹 [{tag}] {name} 창설!"));
        self.bring_defection(pid);
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
        self.bring_defection(pid);
        self.broadcast_alliances();
    }

    /// A captain who left an alliance at one of its stations brings the influence they carried out.
    fn bring_defection(&mut self, pid: PlayerId) {
        let Some((sys, points, until)) = self.store.data.defectors.remove(&pid) else { return };
        let Some(aid) = self.alliance_of(pid) else { return };
        if self.unix <= until {
            self.shift_influence(aid, sys, points);
            self.toast(pid, format!("{}에서 가지고 나온 영향력 {points:.0}이(가) 새 동맹의 것이 되었습니다", galaxy::system(sys).name));
            let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
            for p in online {
                self.send_influence(p);
            }
        }
    }

    pub(super) fn alliance_leave(&mut self, pid: PlayerId) {
        let Some(aid) = self.alliance_of(pid) else { return };
        // Leaving while docked at a station the alliance controls: a governor's secession — a share of
        // its influence there goes with them (`social.betrayal`).
        let b = self.tuning.social.betrayal;
        let docked_at = self.player_entity(pid).filter(|(s, e)| self.ent(*s, *e).is_some_and(|x| x.docked) && self.controller(*s) == Some(aid)).map(|(s, _)| s);
        if let Some(sys) = docked_at.filter(|_| b.defect_share > 0.0) {
            let points = self.influence(sys, aid) * b.defect_share;
            let label = self.alliance(aid).map(|a| format!("[{}] {}", a.tag, a.name)).unwrap_or_default();
            let who = self.store.profile(pid).map(|p| p.name.clone()).unwrap_or_default();
            self.shift_influence(aid, sys, -points);
            self.store.data.defectors.insert(pid, (sys, points, self.unix + (b.defect_hours * 3600.0) as u64));
            self.history.record(self.unix, DomainEvent::AllianceDefection { player: who.clone(), alliance: label.clone(), system: sys, points });
            let text = format!("⚑ {who}이(가) {}에서 {label}을(를) 등지고 영향력 {points:.0}을(를) 가지고 나갔습니다", galaxy::system(sys).name);
            let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
            for p in online {
                self.toast(p, text.clone());
                self.send_influence(p);
            }
        }
        let a = self.store.data.alliances.iter_mut().find(|a| a.id == aid).unwrap();
        a.members.retain(|m| *m != pid);
        a.votes.retain(|v, c| *v != pid && *c != pid);
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
        // Declaring inside a truce breaks the treaty: infamy, which makes this and later wars dearer.
        let b = self.tuning.social.betrayal;
        let unix = self.unix;
        // War on the hegemon is a coalition's right: free, and no betrayal even inside a truce.
        let on_hegemon = war && self.hegemon() == Some(target);
        let breach = war && !on_hegemon && self.alliance(aid).is_some_and(|a| a.truces.iter().any(|(t, end)| *t == target && *end > unix));
        let infamy = self.alliance(aid).map_or(0.0, |a| a.infamy) + if breach { b.breach_infamy } else { 0.0 };
        // A war costs the treasury per system we hold (a newcomer declares for free).
        let cost = if war && !on_hegemon { self.tuning.social.war_cost_for(self.held_by(aid).len(), infamy) } else { 0 };
        if cost > 0 {
            let a = self.store.data.alliances.iter_mut().find(|a| a.id == aid).unwrap();
            if a.treasury < cost {
                return self.toast(pid, format!("선전포고에는 금고 {cost} CR이 필요합니다"));
            }
            a.treasury -= cost;
        }
        let truce_end = unix + (b.truce_hours * 3600.0) as u64;
        for (x, y) in [(aid, target), (target, aid)] {
            let a = self.store.data.alliances.iter_mut().find(|a| a.id == x).unwrap();
            a.truces.retain(|(t, _)| *t != y);
            if war {
                a.wars.push(y);
            } else {
                a.wars.retain(|w| *w != y);
                if b.truce_hours > 0.0 {
                    a.truces.push((y, truce_end));
                }
            }
        }
        if breach && let Some(a) = self.store.data.alliances.iter_mut().find(|a| a.id == aid) {
            a.infamy = infamy;
        }
        let label = |g: &Game, id| g.alliance(id).map(|a| format!("[{}] {}", a.tag, a.name)).unwrap_or_default();
        let (me, them) = (label(self, aid), label(self, target));
        let by = self.store.profile(pid).unwrap().name.clone();
        if breach {
            self.history.record(self.unix, DomainEvent::TreatyBroken { breaker: me.clone(), victim: them.clone(), by: by.clone() });
        }
        if war {
            self.history.record(self.unix, DomainEvent::WarDeclared { aggressor: me.clone(), target: them.clone(), by });
        } else {
            self.history.record(self.unix, DomainEvent::PeaceMade { a: me.clone(), b: them.clone(), by });
        }
        let text = match (war, breach) {
            (true, true) => format!("⚔ {me}이(가) 휴전을 깨고 {them}에 선전포고! (악명 {infamy:.1})"),
            (true, false) => format!("⚔ {me}이(가) {them}에 선전포고! CORE를 제외한 모든 성계에서 교전 가능"),
            _ => format!("☮ {me}과(와) {them} 강화 성립 — 휴전 {:.0}시간", b.truce_hours),
        };
        let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
        for p in online {
            self.toast(p, text.clone());
        }
        self.store.dirty = true;
        self.broadcast_alliances();
    }

    /// Back a fellow member as leader (`AllianceVote`); a majority makes them leader.
    pub(super) fn alliance_vote(&mut self, pid: PlayerId, name: &str) {
        let Some(aid) = self.alliance_of(pid) else { return self.toast(pid, "동맹 소속이 아닙니다") };
        let Some(cand) = self.store.find_by_name(name) else { return self.toast(pid, "그런 함장은 없습니다") };
        let a = self.alliance(aid).cloned().unwrap();
        if !a.members.contains(&cand) {
            return self.toast(pid, "같은 동맹의 함장만 추대할 수 있습니다");
        }
        let x = self.store.data.alliances.iter_mut().find(|x| x.id == aid).unwrap();
        if cand == pid {
            x.votes.remove(&pid);
        } else {
            x.votes.insert(pid, cand);
        }
        // The candidate stands for themself unless they back someone else.
        let backers: Vec<PlayerId> = a.members.iter().copied().filter(|m| x.votes.get(m) == Some(&cand) || (*m == cand && !x.votes.contains_key(m))).collect();
        let needed = (a.members.len() as f32 * self.tuning.social.betrayal.coup_majority).floor() as usize + 1;
        self.store.dirty = true;
        if cand != a.leader && backers.len() >= needed && a.members.len() >= 2 {
            x.leader = cand;
            x.votes.clear();
            let name = |g: &Game, p: PlayerId| g.store.profile(p).map(|p| p.name.clone()).unwrap_or_default();
            let (old, new) = (name(self, a.leader), name(self, cand));
            let label = format!("[{}] {}", a.tag, a.name);
            // The leader's own vote is a handover, not a coup.
            if !backers.contains(&a.leader) {
                let names = backers.iter().map(|p| name(self, *p)).collect();
                self.history.record(self.unix, DomainEvent::LeaderOverthrown { alliance: label.clone(), old: old.clone(), new: new.clone(), backers: names });
            }
            for m in &a.members {
                self.toast(*m, format!("{label}의 새 맹주: {new} ({old} 축출)"));
            }
        } else {
            self.toast(pid, format!("{}을(를) 맹주로 추대 — {}/{needed}명", self.store.profile(cand).map(|p| p.name.clone()).unwrap_or_default(), backers.len()));
        }
        self.broadcast_alliances();
    }

    /// Once an hour: infamy fades, truces and carried influence run out.
    pub(super) fn hourly_betrayal(&mut self) {
        let (k, unix) = (self.tuning.social.betrayal.infamy_decay_hourly, self.unix);
        for a in &mut self.store.data.alliances {
            a.infamy *= k;
            if a.infamy < 0.05 {
                a.infamy = 0.0;
            }
            a.truces.retain(|(_, end)| *end > unix);
        }
        self.store.data.defectors.retain(|_, (_, _, until)| *until >= unix);
    }
}
