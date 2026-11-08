//! Server console (live operations): the operator types commands into the server's terminal; they
//! arrive as `ClientMsg::Admin` on `ADMIN_CONN`, so they are recorded and replayed like everything
//! else. Replies go to `Game::admin_replies` (the server prints them).
//!
//! `players` · `kick NAME` · `ban NAME` · `unban NAME` · `bans` · `say TEXT` · `save`

use super::*;

/// The console's connection id (sockets are numbered from 1).
pub const ADMIN_CONN: ConnId = 0;

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
            _ => "commands: players · kick NAME · ban NAME · unban NAME · bans · say TEXT · save".into(),
        };
        self.admin_reply(reply);
    }

    fn admin_reply(&mut self, text: String) {
        self.admin_replies.push(text);
    }

    /// Banned names never log in (`ban`).
    pub(super) fn is_banned(&self, name: &str) -> bool {
        self.store.data.banned.contains(&name.trim().to_lowercase())
    }
}
