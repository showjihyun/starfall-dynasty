//! 한 WebSocket 연결(= 한 세션)의 수명. 봇의 "행동"은 전부 여기 있고,
//! **계측은 [`crate::ledger`] 가 한다** — 행동과 계측을 섞지 않아야 계측을 따로 테스트할 수 있다.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt, stream::SplitSink, stream::SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

use crate::ledger::{Ledger, SessionRecord};
use crate::snapshot::SnapshotLedger;
use crate::wire::{self, Inbound, PingServerCommand, SetShipControlCommand};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// 원장 표지 — 재개 1구간의 마지막 실제 입력.
pub const MARK_LAST_INPUT: &str = "last-input";
/// 원장 표지 — 재개 2구간의 `input_seq = 1` 명령.
pub const MARK_SEQ1: &str = "seq1";
type WsSink = SplitSink<Ws, Message>;
type WsStreamHalf = SplitStream<Ws>;

/// 모든 봇이 공유하는 단조 시계 + 그 기준점의 벽시계.
///
/// 왕복 지연은 단조 시계로만 재고(`us`), 벽시계는 **다른 도구(docker stop, psql)의 시각과
/// 맞추기 위해서만** 쓴다(AC-19 의 중단 구간 슬라이싱). 두 용도를 섞지 않는다.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub base: Instant,
    pub wall_base_unix_ms: u64,
}

impl Clock {
    pub fn start() -> Self {
        Self {
            base: Instant::now(),
            wall_base_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    pub fn us(&self) -> u64 {
        self.base.elapsed().as_micros() as u64
    }

    pub fn wall_ms_at(&self, us: u64) -> u64 {
        self.wall_base_unix_ms + us / 1000
    }
}

/// 이 연결이 무엇을 하는가.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// A·D 단계: `interval` 마다 ping, `duration` 동안. 정상 Close 로 끝낸다.
    Steady {
        interval: Duration,
        duration: Duration,
        /// 시드에서 나온 초기 위상 지연. 30봇이 같은 순간에 몰려 보내는 인공적 부하를 없앤다.
        phase: Duration,
    },
    /// B 단계 1회분: ping N 건을 보내고 응답을 기다린 뒤 닫는다.
    Burst { pings: u32, grace: Duration },
    /// C 단계 폭주 봇: 최대 속도로 N 건. in-flight 상한에 부딪히는 것이 목적이다.
    /// **보내면서 계속 읽는다** — 읽지 않으면 설계대로 느린 소비자로 닫혀 AC-18(a)가 깨진다
    /// (server 주의, 2026-09-19).
    Flood { count: u32, grace: Duration },
    /// AC-7(b): 같은 `command_id` 를 2회.
    Duplicate,
    /// AC-8(a)(b): 위반 프레임을 `count` 회 보낸다(앱 한도 16 KiB 초과 / 바이너리).
    Violate { oversize: bool, count: u32 },
    /// AC-8(c): 아무것도 보내지 않고 **읽지도 않는다**. 서버 Ping 에 Pong 이 가지 않는다.
    Idle { wait: Duration },
    /// AC-7(c): 읽지 않으면서 명령만 쏟아붓는다 → 서버 송신 큐 포화.
    SlowConsumer { count: u32, wait: Duration },

    // ── p1-01 ───────────────────────────────────────────────────────────────
    /// 조작 입력을 `interval` 마다 보낸다. `thrust` 는 밀리 단위 로컬 축.
    Fly {
        interval: Duration,
        duration: Duration,
        thrust: (i32, i32, i32),
        /// 마지막 `brake_last` 동안 브레이크를 켠다(S-1·S-2 관측용).
        brake_last: Duration,
        phase: Duration,
    },
    /// **원문 프레임을 그대로** 보낸다 — 계약 반례 주입(위치·자세 필드 등).
    /// 봇의 타입을 거치지 않아야 "어휘에 없는 것"을 시험할 수 있다.
    CheatRaw {
        frames: Vec<String>,
        gap: Duration,
        grace: Duration,
    },
    /// `input_seq` 를 역행·반복시킨다 → `STALE_INPUT` 기대.
    CheatSeqRewind { rounds: u32 },
    /// `SESSION_READY` 를 기다리지 않고 먼저 보낸다.
    PreReady { count: u32 },
    /// SC-24 (c)(d) 를 한 연결에서: ① 유효 추력 `lead` → ② 범위 초과 `frames` 를 **유효 명령 대신
    /// 같은 주기로** → ②' 유효 재개 → ③ `aim_*` 극단값(180° 반대편을 번갈아) 을 `turn_hold` 씩.
    /// 분석은 `crate::range_turn`.
    RangeTurn {
        frames: Vec<String>,
        lead: Duration,
        turn_hold: Duration,
    },
    /// SC-11 재개 1구간: 추력 + 롤 + **보조 끔**으로 `fly` 동안 조작한 뒤 **입력을 멈추고** `settle`
    /// 만큼 받기만 하다가 닫는다. `settle` > 이월 창이어야 마지막 스냅샷(T0)이 휴면 구간에 있다.
    /// 보조를 끈 이유: 이월이 끝나 휴면(보조 켬)으로 넘어가는 순간부터 오토레벨이 돌기 시작해
    /// 잔류 구간이 5단계를 반드시 탄다(ADR-0011 §6.1 — 독립 계산이 갈리는 자리).
    ResumeLeg1 { fly: Duration, settle: Duration },
    /// 보내지 않고 `listen` 동안 받기만 한다(관측자·재개 2구간). `seq1_after` 가 있으면 그 시점에
    /// `input_seq = 1` 명령을 하나 보낸다(AC-3(e2): 재개 후 첫 입력이 ACCEPTED 여야 한다).
    Listen {
        listen: Duration,
        seq1_after: Option<Duration>,
    },
}

#[derive(Debug)]
pub struct ConnectionOutcome {
    pub ledger: Ledger,
    pub snapshots: SnapshotLedger,
    pub connect_ms: f64,
    pub ready_ms: Option<f64>,
    /// 서버가 업그레이드를 거절했을 때의 원인 문자열(클라이언트는 상태 코드를 구분할 수 없다 —
    /// ADR-0005 §6. 원인 판정의 증거는 언제나 서버 쪽이다).
    pub connect_error: Option<String>,
}

/// `SESSION_READY` 를 받는 **즉시** correlation 을 파일에 덧붙이는 싱크.
///
/// 왜 필요한가: SC-61(AC-17a)은 "A 단계가 **도는 동안**" 집합으로 조회해야 한다. 실행이 끝난 뒤
/// 쓰는 `correlations.txt` 로는 그 시점에 조회할 대상이 없어 **구조적으로 측정 불가능**해진다.
/// 한 연결에 한 줄, 그때 한 번만 쓴다(핫 경로가 아니다).
#[derive(Debug)]
pub struct LiveCorrelationSink {
    file: std::sync::Mutex<std::fs::File>,
}

impl LiveCorrelationSink {
    pub fn create(path: &std::path::Path) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            file: std::sync::Mutex::new(std::fs::File::create(path)?),
        })
    }

    pub fn record(&self, correlation_id: Uuid) {
        use std::io::Write as _;
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{correlation_id}");
            let _ = f.flush();
        }
    }
}

pub struct BotSpec {
    pub label: String,
    pub url: String,
    pub token: String,
    pub behavior: Behavior,
    pub clock: Clock,
    /// 있으면 `SESSION_READY` 수신 즉시 correlation 을 덧붙인다 (SC-61 용).
    pub live_corr: Option<std::sync::Arc<LiveCorrelationSink>>,
}

impl std::fmt::Debug for BotSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 토큰은 비밀이 아니지만(ADR-0008 §3) 로그를 오염시키지 않도록 찍지 않는다.
        f.debug_struct("BotSpec")
            .field("label", &self.label)
            .field("url", &self.url)
            .field("behavior", &self.behavior)
            .finish_non_exhaustive()
    }
}

pub async fn run_connection(spec: BotSpec) -> ConnectionOutcome {
    let mut ledger = Ledger::new(spec.label.clone());
    let t_connect_start = spec.clock.us();

    let mut request = match spec.url.as_str().into_client_request() {
        Ok(r) => r,
        Err(e) => {
            ledger.on_error(format!("bad url: {e}"));
            return ConnectionOutcome {
                ledger,
                snapshots: SnapshotLedger::new(None),
                connect_ms: 0.0,
                ready_ms: None,
                connect_error: Some(format!("bad url: {e}")),
            };
        }
    };
    match HeaderValue::from_str(&format!("Bearer {}", spec.token)) {
        Ok(v) => {
            request.headers_mut().insert("Authorization", v);
        }
        Err(e) => {
            ledger.on_error(format!("bad token header: {e}"));
        }
    }

    let ws = match tokio_tungstenite::connect_async(request).await {
        Ok((ws, _response)) => ws,
        Err(e) => {
            let msg = format!("connect failed: {e}");
            ledger.on_error(msg.clone());
            return ConnectionOutcome {
                ledger,
                snapshots: SnapshotLedger::new(None),
                connect_ms: (spec.clock.us() - t_connect_start) as f64 / 1000.0,
                ready_ms: None,
                connect_error: Some(msg),
            };
        }
    };
    let connect_us = spec.clock.us();
    let (sink, stream) = ws.split();

    let mut conn = Conn {
        sink,
        stream,
        ledger,
        snapshots: SnapshotLedger::new(None),
        clock: spec.clock,
        live_corr: spec.live_corr.clone(),
        next_seq: 0,
        connected_at_us: connect_us,
        ready_at_us: None,
        closed: false,
    };

    // SESSION_READY 는 그 연결의 첫 계약 메시지다(ADR-0005 §3). 읽지 않는 행동이라도
    // 먼저 이것만은 받는다 — 받지 못하면 correlation 집합에 넣을 것이 없다.
    conn.await_session_ready(Duration::from_secs(10)).await;

    match spec.behavior {
        Behavior::Steady {
            interval,
            duration,
            phase,
        } => conn.run_steady(interval, duration, phase).await,
        Behavior::Burst { pings, grace } => conn.run_burst(pings, grace).await,
        Behavior::Flood { count, grace } => conn.run_flood(count, grace).await,
        Behavior::Duplicate => conn.run_duplicate().await,
        Behavior::Violate { oversize, count } => conn.run_violate(oversize, count).await,
        Behavior::Idle { wait } => conn.run_idle(wait).await,
        Behavior::SlowConsumer { count, wait } => conn.run_slow_consumer(count, wait).await,
        Behavior::Fly {
            interval,
            duration,
            thrust,
            brake_last,
            phase,
        } => {
            conn.run_fly(interval, duration, thrust, brake_last, phase)
                .await
        }
        Behavior::CheatRaw { frames, gap, grace } => conn.run_cheat_raw(frames, gap, grace).await,
        Behavior::CheatSeqRewind { rounds } => conn.run_cheat_seq_rewind(rounds).await,
        Behavior::PreReady { count } => conn.run_pre_ready(count).await,
        Behavior::RangeTurn {
            frames,
            lead,
            turn_hold,
        } => conn.run_range_turn(frames, lead, turn_hold).await,
        Behavior::ResumeLeg1 { fly, settle } => conn.run_resume_leg1(fly, settle).await,
        Behavior::Listen { listen, seq1_after } => conn.run_listen(listen, seq1_after).await,
    }

    let ready_ms = conn
        .ready_at_us
        .map(|v| (v.saturating_sub(t_connect_start)) as f64 / 1000.0);
    ConnectionOutcome {
        ledger: conn.ledger,
        snapshots: conn.snapshots,
        connect_ms: (connect_us - t_connect_start) as f64 / 1000.0,
        ready_ms,
        connect_error: None,
    }
}

struct Conn {
    sink: WsSink,
    stream: WsStreamHalf,
    ledger: Ledger,
    /// p1-01: `WORLD_SNAPSHOT` 관측 계측. 명령 원장과 **분리**한다 — 두 축은 서로 다른 것을 잰다.
    pub snapshots: SnapshotLedger,
    clock: Clock,
    live_corr: Option<std::sync::Arc<LiveCorrelationSink>>,
    next_seq: u32,
    connected_at_us: u64,
    ready_at_us: Option<u64>,
    closed: bool,
}

impl Conn {
    async fn await_session_ready(&mut self, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let next = tokio::time::timeout_at(deadline, self.stream.next()).await;
            match next {
                Err(_) => {
                    self.ledger
                        .on_error("timed out waiting for SESSION_READY".to_owned());
                    return;
                }
                Ok(None) => {
                    self.ledger
                        .on_error("connection closed before SESSION_READY".to_owned());
                    self.closed = true;
                    return;
                }
                Ok(Some(Err(e))) => {
                    self.ledger.on_error(format!("socket error: {e}"));
                    self.closed = true;
                    return;
                }
                Ok(Some(Ok(msg))) => {
                    let ready = self.handle_message(msg);
                    if self.ready_at_us.is_some() || self.closed {
                        let _ = ready;
                        return;
                    }
                }
            }
        }
    }

    /// 프레임 1건 처리. `true` 를 돌려주면 연결이 끝났다는 뜻이다.
    fn handle_message(&mut self, msg: Message) -> bool {
        let at = self.clock.us();
        match msg {
            Message::Text(text) => {
                match wire::parse_inbound(text.as_str()) {
                    Inbound::SessionReady(m) => {
                        if self.ready_at_us.is_none() {
                            self.ready_at_us = Some(at);
                            self.snapshots.set_observer(m.payload.actor_id);
                            self.ledger.on_session_ready(SessionRecord {
                                bot: self.ledger.bot.clone(),
                                session_id: m.payload.session_id,
                                correlation_id: m.correlation_id,
                                actor_id: m.payload.actor_id,
                                tick_hz: m.payload.tick_hz,
                                server_version: m.payload.server_version.clone(),
                                ready_tick: m.tick,
                                connected_at_us: self.connected_at_us,
                                ready_at_us: at,
                                closed_at_us: None,
                                close_code: None,
                                peer_close_code: None,
                                close_reason_text: None,
                                close_initiator: "none".to_owned(),
                            });
                            match m.correlation_id {
                                // SC-61: 실행 중에 조회하려면 지금 기록해야 한다.
                                Some(c) => {
                                    if let Some(sink) = &self.live_corr {
                                        sink.record(c);
                                    }
                                }
                                // 스펙 §5.2: SESSION_READY 의 correlation_id 는 그 세션의
                                // correlation 이다. null 이면 집합 대조가 불가능해진다 —
                                // 조용히 넘기지 않고 오류로 남긴다.
                                None => self.ledger.on_error(
                                    "SESSION_READY.correlation_id is null (session set cannot be built)"
                                        .to_owned(),
                                ),
                            }
                        } else {
                            self.ledger.on_session_ready(SessionRecord {
                                bot: self.ledger.bot.clone(),
                                session_id: m.payload.session_id,
                                correlation_id: m.correlation_id,
                                actor_id: m.payload.actor_id,
                                tick_hz: m.payload.tick_hz,
                                server_version: m.payload.server_version.clone(),
                                ready_tick: m.tick,
                                connected_at_us: self.connected_at_us,
                                ready_at_us: at,
                                closed_at_us: None,
                                close_code: None,
                                peer_close_code: None,
                                close_reason_text: None,
                                close_initiator: "none".to_owned(),
                            });
                        }
                    }
                    Inbound::CommandResult(m) => {
                        if self.ready_at_us.is_none() {
                            self.ledger.on_error(
                                "first contract message was COMMAND_RESULT, not SESSION_READY"
                                    .to_owned(),
                            );
                        }
                        self.ledger.on_command_result(
                            m.payload.command_id,
                            &m.payload.status,
                            m.payload.reason_code.as_deref(),
                            at,
                            m.tick,
                        );
                    }
                    Inbound::WorldSnapshot(m) => {
                        if self.ready_at_us.is_none() {
                            self.ledger.on_error(
                                "first contract message was WORLD_SNAPSHOT, not SESSION_READY"
                                    .to_owned(),
                            );
                        }
                        self.snapshots.on_snapshot(&m, text.len());
                    }
                    Inbound::PingReply(m) => {
                        if self.ready_at_us.is_none() {
                            self.ledger.on_error(
                                "first contract message was PING_REPLY, not SESSION_READY"
                                    .to_owned(),
                            );
                        }
                        self.ledger.on_ping_reply(
                            m.payload.command_id,
                            m.payload.probe_seq,
                            at,
                            m.tick,
                        );
                    }
                    Inbound::Unknown { message_type } => {
                        self.ledger.on_unknown_message(&message_type);
                    }
                    Inbound::Malformed { reason, raw } => {
                        self.ledger.on_wire_error(format!("{reason} | raw={raw}"));
                    }
                }
                false
            }
            Message::Binary(_) => {
                self.ledger
                    .on_wire_error("server sent a binary frame (contract is text-only)".to_owned());
                false
            }
            Message::Ping(_) | Message::Pong(_) => false,
            Message::Close(frame) => {
                let (code, reason) = match frame {
                    Some(CloseFrame { code, reason }) => {
                        (Some(u16::from(code)), Some(reason.as_str().to_owned()))
                    }
                    None => (None, None),
                };
                self.ledger.on_close(at, code, reason, "server");
                self.closed = true;
                true
            }
            Message::Frame(_) => false,
        }
    }

    async fn send_ping(&mut self, command_id: Uuid) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let cmd = PingServerCommand::new(command_id, seq);
        let text = match serde_json::to_string(&cmd) {
            Ok(t) => t,
            Err(e) => {
                self.ledger.on_error(format!("serialize failed: {e}"));
                return;
            }
        };
        // 송신 시각은 **소켓에 넣기 직전**에 찍는다. 그래야 왕복에 우리 직렬화 비용이 섞이지 않는다.
        let at = self.clock.us();
        match self.sink.send(Message::text(text)).await {
            Ok(()) => self.ledger.on_sent(command_id, seq, at),
            Err(e) => {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
            }
        }
    }

    async fn pump_for(&mut self, dur: Duration) {
        let deadline = tokio::time::Instant::now() + dur;
        while !self.closed {
            match tokio::time::timeout_at(deadline, self.stream.next()).await {
                Err(_) => return,
                Ok(None) => {
                    self.closed = true;
                    return;
                }
                Ok(Some(Err(e))) => {
                    self.ledger.on_error(format!("socket error: {e}"));
                    self.closed = true;
                    return;
                }
                Ok(Some(Ok(msg))) => {
                    if self.handle_message(msg) {
                        return;
                    }
                }
            }
        }
    }

    async fn run_steady(&mut self, interval: Duration, duration: Duration, phase: Duration) {
        if !phase.is_zero() {
            self.pump_for(phase).await;
        }
        let end = tokio::time::Instant::now() + duration;
        while !self.closed && tokio::time::Instant::now() < end {
            self.send_ping(Uuid::now_v7()).await;
            let next = (tokio::time::Instant::now() + interval).min(end);
            let gap = next.saturating_duration_since(tokio::time::Instant::now());
            if gap.is_zero() {
                break;
            }
            self.pump_for(gap).await;
        }
        // 마지막 왕복이 돌아올 시간을 준다. 이 유예가 없으면 "손실"이 우리 조급함이 된다.
        self.pump_for(Duration::from_millis(1500)).await;
        self.close_client_side().await;
    }

    async fn run_burst(&mut self, pings: u32, grace: Duration) {
        for _ in 0..pings {
            if self.closed {
                break;
            }
            self.send_ping(Uuid::now_v7()).await;
            self.pump_for(Duration::from_millis(30)).await;
        }
        self.pump_for(grace).await;
        self.close_client_side().await;
    }

    async fn run_flood(&mut self, count: u32, grace: Duration) {
        for i in 0..count {
            if self.closed {
                break;
            }
            self.send_ping(Uuid::now_v7()).await;
            // 16건마다 읽는다. 명령 1건이 응답 2건(COMMAND_RESULT + PING_REPLY)을 만들므로
            // 너무 드물게 읽으면 우리가 느린 소비자가 되어 서버가 연결을 닫는다 — 그러면
            // AC-18(a)의 "연결이 유지된다"를 우리 손으로 깨뜨린다.
            if i % 16 == 15 {
                self.pump_for(Duration::from_millis(2)).await;
            }
        }
        self.pump_for(grace).await;
        self.close_client_side().await;
    }

    async fn run_duplicate(&mut self) {
        let id = Uuid::now_v7();
        self.send_ping(id).await;
        self.pump_for(Duration::from_millis(300)).await;
        // 같은 command_id 를 그대로 다시 보낸다. probe_seq 는 증가하지만 서버는
        // command_id 로만 중복 제거한다(ADR-0006 §6).
        self.send_ping(id).await;
        self.pump_for(Duration::from_millis(1500)).await;
        self.close_client_side().await;
    }

    async fn run_violate(&mut self, oversize: bool, count: u32) {
        for _ in 0..count {
            if self.closed {
                break;
            }
            let msg = if oversize {
                // 앱 한도 16 KiB 초과, 라이브러리 한도 64 KiB 미만. 이 사이여야 서버가 셀 수 있다
                // (ADR-0005 §2 — 같은 값이면 tungstenite 가 먼저 끊어 계수 자체가 불가능하다).
                let filler = "x".repeat(20 * 1024);
                Message::text(format!(
                    "{{\"message_type\":\"PING_SERVER\",\"pad\":\"{filler}\"}}"
                ))
            } else {
                Message::binary(vec![0u8; 64])
            };
            if let Err(e) = self.sink.send(msg).await {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
                break;
            }
            self.pump_for(Duration::from_millis(200)).await;
        }
        self.pump_for(Duration::from_secs(3)).await;
        if !self.closed {
            self.close_client_side().await;
        }
    }

    async fn run_idle(&mut self, wait: Duration) {
        // 읽지 않는다 → tungstenite 가 서버 Ping 에 Pong 을 보내지 않는다.
        tokio::time::sleep(wait).await;
        // 그 뒤에 버퍼에 쌓인 것(서버의 Close 포함)을 비운다.
        self.pump_for(Duration::from_secs(5)).await;
    }

    async fn run_slow_consumer(&mut self, count: u32, wait: Duration) {
        for _ in 0..count {
            let text = self.ping_text();
            if self.sink.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        tokio::time::sleep(wait).await;
        self.pump_for(Duration::from_secs(5)).await;
    }

    /// 읽지 않는 행동용 — 원장에 기록하지 않는다(응답을 받지 않을 것이므로 1:1 대조 대상이 아니다).
    fn ping_text(&self) -> String {
        let cmd = PingServerCommand::new(Uuid::now_v7(), 0);
        serde_json::to_string(&cmd).unwrap_or_else(|_| "{}".to_owned())
    }

    /// `SET_SHIP_CONTROL` 하나를 보낸다. 원장에는 **명령으로** 기록된다(1:1 대조 대상).
    async fn send_control(&mut self, cmd: SetShipControlCommand) {
        let seq = cmd.payload.input_seq;
        let command_id = cmd.command_id;
        let text = match serde_json::to_string(&cmd) {
            Ok(t) => t,
            Err(e) => {
                self.ledger.on_error(format!("serialize failed: {e}"));
                return;
            }
        };
        let at = self.clock.us();
        match self.sink.send(Message::text(text)).await {
            // probe_seq 자리에 input_seq 를 넣어 두면 응답 대조가 그대로 동작한다.
            Ok(()) => self.ledger.on_sent_no_reply(command_id, seq as u32, at),
            Err(e) => {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
            }
        }
    }

    async fn run_fly(
        &mut self,
        interval: Duration,
        duration: Duration,
        thrust: (i32, i32, i32),
        brake_last: Duration,
        phase: Duration,
    ) {
        if !phase.is_zero() {
            self.pump_for(phase).await;
        }
        let start = tokio::time::Instant::now();
        let end = start + duration;
        let brake_from = end.checked_sub(brake_last).unwrap_or(end);
        let mut seq: u64 = 0;
        while !self.closed && tokio::time::Instant::now() < end {
            seq += 1;
            let braking = tokio::time::Instant::now() >= brake_from;
            let cmd = SetShipControlCommand::new(Uuid::now_v7(), seq)
                .with_thrust(thrust.0, thrust.1, thrust.2)
                .with_brake(braking);
            self.send_control(cmd).await;
            let next = (tokio::time::Instant::now() + interval).min(end);
            let gap = next.saturating_duration_since(tokio::time::Instant::now());
            if gap.is_zero() {
                break;
            }
            self.pump_for(gap).await;
        }
        // 마지막 스냅샷·응답이 돌아올 시간을 준다.
        self.pump_for(Duration::from_millis(1500)).await;
        self.close_client_side().await;
    }

    async fn run_cheat_raw(&mut self, frames: Vec<String>, gap: Duration, grace: Duration) {
        for frame in frames {
            if self.closed {
                break;
            }
            // 원장에 "보냈다"로 기록하지 않는다 — command_id 를 우리가 모를 수 있고,
            // 이 시나리오의 판정은 1:1 이 아니라 **거부 수와 상태 불변**이다.
            if let Err(e) = self.sink.send(Message::text(frame)).await {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
                break;
            }
            self.pump_for(gap).await;
        }
        self.pump_for(grace).await;
        if !self.closed {
            self.close_client_side().await;
        }
    }

    async fn run_cheat_seq_rewind(&mut self, rounds: u32) {
        // 5 → 3 → 5 → 1 … 순서로 보낸다. 서버는 후퇴를 STALE_INPUT 으로 거부해야 하고
        // ack_input_seq 는 세션 안에서 되돌아가지 않아야 한다(SC-31·SC-67).
        let pattern = [5u64, 3, 5, 1, 7, 2];
        for r in 0..rounds.max(1) {
            for seq in pattern {
                if self.closed {
                    break;
                }
                let cmd = SetShipControlCommand::new(Uuid::now_v7(), seq + u64::from(r) * 10)
                    .with_thrust(0, 0, 300);
                self.send_control(cmd).await;
                self.pump_for(Duration::from_millis(60)).await;
            }
        }
        self.pump_for(Duration::from_millis(1500)).await;
        self.close_client_side().await;
    }

    async fn run_pre_ready(&mut self, count: u32) {
        // await_session_ready 가 이미 돌았으므로 여기서는 "READY 직후"가 된다.
        // 진짜 pre-ready 는 connect 직후 전송이어야 하므로 run_connection 이 아니라
        // 이 행동을 **SESSION_READY 대기 전에** 호출해야 한다(scenario 에서 처리).
        for i in 0..count {
            let cmd = SetShipControlCommand::new(Uuid::now_v7(), u64::from(i) + 1)
                .with_thrust(0, 0, 1000);
            self.send_control(cmd).await;
        }
        self.pump_for(Duration::from_secs(2)).await;
        self.close_client_side().await;
    }

    async fn run_range_turn(&mut self, frames: Vec<String>, lead: Duration, turn_hold: Duration) {
        use crate::range_turn::{MARK_INJECTED, MARK_TURN_START};
        // 20 Hz — tick 당 입력 1건(client_send_hz). 주입도 같은 주기로 넣어야 "그 tick 에 유효 입력이
        // 없어 이월된다"가 성립한다.
        let interval = Duration::from_millis(50);
        let mut seq: u64 = 0;

        // ① 유효 추력. 아직 최고 속도 전이어야(0→최고 4초) 이월이 끊기면 속도가 **줄어드는 것**이 보인다.
        let end = tokio::time::Instant::now() + lead;
        while !self.closed && tokio::time::Instant::now() < end {
            seq += 1;
            let cmd = SetShipControlCommand::new(Uuid::now_v7(), seq).with_thrust(0, 0, 1000);
            self.send_control(cmd).await;
            self.pump_for(interval).await;
        }

        // ② 범위 초과. **input_seq 를 다음 번호로 바꿔 넣는다** — 서버가 클램프해서 적용했다면
        // `ack_input_seq` 가 그 번호가 되므로 "클램프 흔적 없음"이 스냅샷으로 판정된다.
        // 위반 예산(10초에 8건)을 넘지 않게 frames 는 4건 안팎으로 둔다.
        for frame in frames {
            if self.closed {
                break;
            }
            seq += 1;
            let command_id = Uuid::now_v7();
            let text = match serde_json::from_str::<serde_json::Value>(&frame) {
                Ok(mut v) => {
                    v["command_id"] = serde_json::json!(command_id.to_string());
                    v["payload"]["input_seq"] = serde_json::json!(seq);
                    v.to_string()
                }
                Err(e) => {
                    self.ledger
                        .on_error(format!("cheat frame parse failed: {e}"));
                    continue;
                }
            };
            let at = self.clock.us();
            match self.sink.send(Message::text(text)).await {
                Ok(()) => {
                    self.ledger.on_sent_no_reply(command_id, seq as u32, at);
                    self.ledger.mark(MARK_INJECTED, command_id);
                }
                Err(e) => {
                    self.ledger.on_error(format!("send failed: {e}"));
                    self.closed = true;
                }
            }
            self.pump_for(interval).await;
        }

        // ②' 이월 창(10 tick) 안에 유효 입력을 재개한다.
        for _ in 0..10 {
            if self.closed {
                break;
            }
            seq += 1;
            let cmd = SetShipControlCommand::new(Uuid::now_v7(), seq).with_thrust(0, 0, 1000);
            self.send_control(cmd).await;
            self.pump_for(interval).await;
        }

        // ③ aim 극단값: 성분 최댓값으로 **180° 반대편**(Y 축 = 위, ADR-0009 §1)을 번갈아 준다.
        // Y 축 선회라 오토레벨(롤)이 끼어들 이유가 없다 — 쿼터니언 차분이 조준 회전만 잰다.
        // 추력 0: 경계 근처 가속이 섞이지 않게.
        let targets = [
            (0, 1_000_000, 0, 0),
            (0, 0, 0, 1_000_000),
            (0, 1_000_000, 0, 0),
        ];
        let mut first = true;
        for (x, y, z, w) in targets {
            let end = tokio::time::Instant::now() + turn_hold;
            while !self.closed && tokio::time::Instant::now() < end {
                seq += 1;
                let command_id = Uuid::now_v7();
                let cmd = SetShipControlCommand::new(command_id, seq).with_aim(x, y, z, w);
                self.send_control(cmd).await;
                if first {
                    self.ledger.mark(MARK_TURN_START, command_id);
                    first = false;
                }
                self.pump_for(interval).await;
            }
        }
        self.pump_for(Duration::from_millis(1500)).await;
        self.close_client_side().await;
    }

    async fn run_resume_leg1(&mut self, fly: Duration, settle: Duration) {
        let interval = Duration::from_millis(50);
        let end = tokio::time::Instant::now() + fly;
        let mut seq: u64 = 0;
        let mut last = None;
        while !self.closed && tokio::time::Instant::now() < end {
            seq += 1;
            let command_id = Uuid::now_v7();
            let cmd = SetShipControlCommand::new(command_id, seq)
                .with_thrust(0, 0, 1000)
                .with_roll(1000)
                .with_assist(false);
            self.send_control(cmd).await;
            last = Some(command_id);
            self.pump_for(interval).await;
        }
        // 마지막 실제 입력의 적용 tick 을 분석이 알게 한다(T0 가 이월 창 뒤인지 확인용).
        if let Some(id) = last {
            self.ledger.mark(MARK_LAST_INPUT, id);
        }
        self.pump_for(settle).await;
        self.close_client_side().await;
    }

    async fn run_listen(&mut self, listen: Duration, seq1_after: Option<Duration>) {
        match seq1_after {
            Some(at) if at < listen => {
                self.pump_for(at).await;
                if !self.closed {
                    let command_id = Uuid::now_v7();
                    let cmd = SetShipControlCommand::new(command_id, 1);
                    self.send_control(cmd).await;
                    self.ledger.mark(MARK_SEQ1, command_id);
                }
                self.pump_for(listen - at).await;
            }
            _ => self.pump_for(listen).await,
        }
        self.close_client_side().await;
    }

    async fn close_client_side(&mut self) {
        if self.closed {
            return;
        }
        let frame = CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        };
        if self.sink.send(Message::Close(Some(frame))).await.is_err() {
            return;
        }
        let at = self.clock.us();
        self.ledger.on_close(at, Some(1000), None, "client");
        // 서버의 Close 응답을 기다린다(양방향 Close 교환 — ADR-0005 §2).
        self.pump_for(Duration::from_secs(3)).await;
    }
}
