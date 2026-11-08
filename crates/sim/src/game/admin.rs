//! Server console (live operations): the operator types commands into the server's terminal; they
//! arrive as `ClientMsg::Admin` on `ADMIN_CONN`, so they are recorded and replayed like everything
//! else. Replies go to `Game::admin_replies` (the server prints them).
//!
//! `players` · `kick NAME` · `ban NAME` · `unban NAME` · `bans` · `reports` · `say TEXT` · `save`
//!
//! The players' side of moderation is typed into chat: `/block NAME`, `/unblock NAME`,
//! `/report NAME REASON`.

use super::*;

/// The console's connection id (sockets are numbered from 1).
pub const ADMIN_CONN: ConnId = 0;

/// Chat lines kept per captain for a report, reports kept, and how often one captain may report the
/// same captain (s). Moderation limits, not game rules.
const REPORT_LINES: usize = 5;
const REPORTS_KEPT: usize = 1000;
const REPORT_AGAIN_SECS: u64 = 3600;

impl Game {
    pub(super) fn admin(&mut self, command: &str) {
        let mut words = command.split_whitespace();
        let verb = words.next().unwrap_or("");
        let rest: Vec<&str> = words.collect();
        let arg = rest.join(" ");
        let reply = match verb {
            "players" => {
                let mut rows: Vec<String> = self.conn_of.iter().filter_map(|(pid, c)| {
                    let name = self.store.profile(*pid)?.name.clone();
                    Some(format!("{name} ({})", galaxy::system(self.sessions.get(c)?.system).name))
                }).collect();
                rows.sort();
                format!("{} online: {}", rows.len(), rows.join(", "))
            }
            "kick" | "ban" => {
                let Some(pid) = self.store.find_by_name(&arg) else { return self.admin_reply(format!("no captain named {arg:?}")) };
                let name = self.store.profile(pid).map_or(arg.clone(), |p| p.name.clone());
                if verb == "ban" {
                    self.store.data.banned.insert(name.to_lowercase());
                    self.store.dirty = true;
                }
                let online = self.conn_of.get(&pid).copied();
                if let Some(c) = online {
                    let why = if verb == "ban" { "이 계정은 운영 정책에 따라 정지되었습니다" } else { "운영자에 의해 연결이 종료되었습니다" };
                    self.send(c, ServerMsg::Error { msg: why.into() });
                    self.disconnect(c);
                    self.kicked.push(c);
                }
                format!("{verb}: {name}{}", if online.is_some() { " (disconnected)" } else { " (offline)" })
            }
            "unban" => {
                let had = self.store.data.banned.remove(&arg.to_lowercase());
                self.store.dirty = true;
                format!("unban {arg}: {}", if had { "done" } else { "was not banned" })
            }
            "bans" => format!("banned: {}", self.store.data.banned.iter().cloned().collect::<Vec<_>>().join(", ")),
            "reports" => {
                let rows: Vec<String> = self.store.data.reports.iter().rev().take(20).map(|r| format!("{} -> {}: {} [{}]", r.reporter, r.target, r.reason, r.lines.join(" / "))).collect();
                format!("{} reports, newest first: {}", self.store.data.reports.len(), rows.join(" | "))
            }
            "say" if !arg.is_empty() => {
                let online: Vec<PlayerId> = self.conn_of.keys().copied().collect();
                for p in &online {
                    self.toast(*p, format!("📢 운영: {arg}"));
                }
                format!("said to {} captains", online.len())
            }
            "save" => {
                self.store.dirty = true;
                self.persist_tick();
                "saved".into()
            }
            _ => "commands: players · kick NAME · ban NAME · unban NAME · bans · reports · say TEXT · save".into(),
        };
        self.admin_reply(reply);
    }

    fn admin_reply(&mut self, text: String) {
        self.admin_replies.push(text);
    }

    /// `/block`, `/unblock`, `/report` typed into chat; true when the line was one of them (it is not
    /// chat, so nobody else sees it and the spam guard does not count it).
    pub(super) fn chat_command(&mut self, pid: PlayerId, text: &str) -> bool {
        let mut words = text.split_whitespace();
        let verb = words.next().unwrap_or("");
        if !matches!(verb, "/block" | "/unblock" | "/report") {
            return false;
        }
        let name = words.next().unwrap_or("");
        let reason = words.collect::<Vec<_>>().join(" ");
        let Some(target) = self.store.find_by_name(name).filter(|t| *t != pid) else {
            self.toast(pid, format!("그런 함장은 없습니다: {name}"));
            return true;
        };
        let (me, them) = (self.store.profile(pid).map_or(String::new(), |p| p.name.clone()), self.store.profile(target).map_or(name.to_string(), |p| p.name.clone()));
        match verb {
            "/block" => {
                self.store.data.blocks.entry(pid).or_default().insert(target);
                self.store.dirty = true;
                self.toast(pid, format!("{them}의 채팅을 더는 받지 않습니다 (/unblock으로 해제)"));
            }
            "/unblock" => {
                if let Some(b) = self.store.data.blocks.get_mut(&pid) {
                    b.remove(&target);
                    if b.is_empty() {
                        self.store.data.blocks.remove(&pid);
                    }
                }
                self.store.dirty = true;
                self.toast(pid, format!("{them}의 차단을 해제했습니다"));
            }
            _ => {
                let unix = self.unix;
                if self.store.data.reports.iter().any(|r| r.reporter == me && r.target == them && unix.saturating_sub(r.unix) < REPORT_AGAIN_SECS) {
                    self.toast(pid, format!("{them}은(는) 이미 신고했습니다"));
                    return true;
                }
                let lines = self.recent_chat.get(&target).cloned().unwrap_or_default();
                self.store.data.reports.push(crate::store::Report { unix, reporter: me, target: them.clone(), reason, lines });
                if self.store.data.reports.len() > REPORTS_KEPT {
                    self.store.data.reports.remove(0);
                }
                self.store.dirty = true;
                self.toast(pid, format!("{them}을(를) 신고했습니다 — 운영자가 확인합니다"));
            }
        }
        true
    }

    /// Keep a captain's last chat lines for a report against them.
    pub(super) fn remember_chat(&mut self, pid: PlayerId, text: &str) {
        let lines = self.recent_chat.entry(pid).or_default();
        lines.push(text.to_string());
        if lines.len() > REPORT_LINES {
            lines.remove(0);
        }
    }

    /// Banned names never log in (`ban`).
    pub(super) fn is_banned(&self, name: &str) -> bool {
        self.store.data.banned.contains(&name.trim().to_lowercase())
    }
}
