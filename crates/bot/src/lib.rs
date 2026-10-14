//! Headless client over the real WebSocket protocol. Used for end-to-end tests and load tests.

use futures_util::{SinkExt, StreamExt};
use shared::galaxy::SystemId;
use shared::glam::{Quat, Vec3};
use shared::protocol::*;
use std::collections::HashMap;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct Bot {
    ws: Ws,
    pub name: String,
    pub token: Option<String>,
    pub profile: Option<Profile>,
    pub me: Option<EntityId>,
    pub system: SystemId,
    pub ents: HashMap<EntityId, EntityState>,
    pub log: Vec<ServerMsg>,
    pub dead: bool,
}

impl Bot {
    pub async fn connect(url: &str, name: &str) -> Result<Bot, String> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| e.to_string())?;
        Ok(Bot { ws, name: name.into(), token: None, profile: None, me: None, system: 0, ents: HashMap::new(), log: vec![], dead: false })
    }

    pub async fn send(&mut self, m: ClientMsg) {
        let _ = self.ws.send(Message::text(encode(&m))).await;
    }

    fn absorb(&mut self, m: &ServerMsg) {
        match m {
            ServerMsg::Welcome { token, profile, .. } => {
                self.token = Some(token.clone());
                self.profile = profile.clone();
            }
            ServerMsg::Profile(p) => self.profile = Some(p.clone()),
            ServerMsg::EnterSystem { system, you } => {
                self.system = *system;
                self.me = Some(*you);
                self.ents.clear();
                self.dead = false;
            }
            ServerMsg::Snapshot { entities, .. } => {
                self.ents = entities.iter().map(|e| (e.id, e.clone())).collect();
            }
            ServerMsg::Died { .. } => {
                self.dead = true;
                self.me = None;
            }
            _ => {}
        }
    }

    /// Read frames until `pred` matches one (returned) or the timeout elapses.
    pub async fn until<F: FnMut(&ServerMsg) -> bool>(&mut self, secs: f32, mut pred: F) -> Option<ServerMsg> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs_f32(secs);
        loop {
            let left = deadline.saturating_duration_since(tokio::time::Instant::now());
            if left.is_zero() {
                return None;
            }
            match tokio::time::timeout(left, self.ws.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => {
                    if let Ok(m) = decode::<ServerMsg>(&t) {
                        self.absorb(&m);
                        let hit = pred(&m);
                        if !matches!(m, ServerMsg::Snapshot { .. } | ServerMsg::Fx(_)) {
                            self.log.push(m.clone());
                        }
                        if hit {
                            return Some(m);
                        }
                    }
                }
                Ok(Some(Ok(_))) => {}
                _ => return None,
            }
        }
    }

    /// Drain frames for `secs`, keeping state fresh.
    pub async fn pump(&mut self, secs: f32) {
        self.until(secs, |_| false).await;
    }

    pub async fn login_and_create(&mut self, hull: shared::ships::HullClass, species: shared::galaxy::Species, prof: Profession) -> Result<(), String> {
        self.send(ClientMsg::Hello { version: PROTOCOL_VERSION, name: self.name.clone(), token: None }).await;
        match self.until(5.0, |m| matches!(m, ServerMsg::Welcome { .. } | ServerMsg::Error { .. })).await {
            Some(ServerMsg::Welcome { .. }) => {}
            other => return Err(format!("login failed: {other:?}")),
        }
        if self.profile.is_none() {
            self.send(ClientMsg::CreateCharacter { species, profession: prof, hull, ship_name: format!("{}호", self.name), livery: 3 }).await;
            self.until(5.0, |m| matches!(m, ServerMsg::EnterSystem { .. })).await.ok_or("no EnterSystem")?;
        }
        Ok(())
    }

    pub fn my_state(&self) -> Option<&EntityState> {
        self.me.and_then(|id| self.ents.get(&id))
    }

    pub async fn warp_to(&mut self, sys: SystemId) -> Result<(), String> {
        self.send(ClientMsg::Undock).await;
        self.pump(0.3).await;
        self.send(ClientMsg::Warp { system: sys }).await;
        match self.until(8.0, |m| matches!(m, ServerMsg::EnterSystem { .. } | ServerMsg::Error { .. })).await {
            Some(ServerMsg::EnterSystem { system, .. }) if system == sys => Ok(()),
            other => Err(format!("warp to {sys} failed: {other:?}")),
        }
    }

    /// Steering helper: controls that point the nose at `target`.
    pub fn aim_controls(&self, target: Vec3, thrust_if_far: f32, stop_dist: f32) -> Controls {
        let Some(me) = self.my_state() else { return Controls::default() };
        let pos = Vec3::from(me.pos);
        let rot = Quat::from_array(me.rot);
        let to = target - pos;
        let local = rot.inverse() * to.normalize_or_zero();
        let mut c = Controls { yaw: (local.x * 4.0).clamp(-1.0, 1.0), pitch: (local.y * 4.0).clamp(-1.0, 1.0), ..Default::default() };
        if local.z > 0.0 {
            c.yaw = if local.x >= 0.0 { 1.0 } else { -1.0 };
        }
        c.thrust = if to.length() > stop_dist { thrust_if_far } else { 0.0 };
        c
    }
}
