//! WebSocket link on a background thread (tokio). The Bevy side talks to it through channels.

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};
use futures_util::{SinkExt, StreamExt};
use shared::protocol::{ClientMsg, ServerMsg, decode, encode};
use tokio_tungstenite::tungstenite::Message;

pub enum NetEvent {
    Connected,
    Msg(Box<ServerMsg>),
    Disconnected(String),
}

#[derive(Resource)]
pub struct Net {
    tx: Sender<ClientMsg>,
    pub rx: Receiver<NetEvent>,
}

impl Net {
    pub fn send(&self, m: ClientMsg) {
        let _ = self.tx.send(m);
    }
}

pub fn connect(url: String) -> Net {
    let (out_tx, out_rx) = unbounded::<ClientMsg>();
    let (in_tx, in_rx) = unbounded::<NetEvent>();
    std::thread::Builder::new()
        .name("net".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("tokio");
            rt.block_on(async move {
                let ws = match tokio_tungstenite::connect_async(&url).await {
                    Ok((ws, _)) => ws,
                    Err(e) => {
                        let _ = in_tx.send(NetEvent::Disconnected(format!("{url} 연결 실패: {e}")));
                        return;
                    }
                };
                let _ = in_tx.send(NetEvent::Connected);
                let (mut sink, mut source) = ws.split();
                // Outgoing: poll the crossbeam channel from a blocking task.
                let (atx, mut arx) = tokio::sync::mpsc::unbounded_channel::<ClientMsg>();
                std::thread::spawn(move || {
                    while let Ok(m) = out_rx.recv() {
                        if atx.send(m).is_err() {
                            break;
                        }
                    }
                });
                let writer = tokio::spawn(async move {
                    while let Some(m) = arx.recv().await {
                        if sink.send(Message::text(encode(&m))).await.is_err() {
                            break;
                        }
                    }
                });
                let reason = loop {
                    match source.next().await {
                        Some(Ok(Message::Text(t))) => match decode::<ServerMsg>(&t) {
                            Ok(m) => {
                                if in_tx.send(NetEvent::Msg(Box::new(m))).is_err() {
                                    break "client closed".to_string();
                                }
                            }
                            Err(e) => eprintln!("bad server frame: {e}"),
                        },
                        Some(Ok(Message::Close(_))) | None => break "서버가 연결을 종료했습니다".to_string(),
                        Some(Err(e)) => break e.to_string(),
                        _ => {}
                    }
                };
                writer.abort();
                let _ = in_tx.send(NetEvent::Disconnected(reason));
            });
        })
        .expect("spawn net thread");
    Net { tx: out_tx, rx: in_rx }
}
