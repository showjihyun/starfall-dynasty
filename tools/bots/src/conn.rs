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
use crate::mining::{MiningObs, NavTrace, Observed, ObservedResult, SentMine};
use crate::nav;
use crate::snapshot::SnapshotLedger;
use crate::wire::{
    self, Inbound, MineResourceCommand, PingServerCommand, SetShipControlCommand, ShipState,
};

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
    /// p1-02 계약 채굴 케이스: 비행 방식 하나 + 단계 목록(`mine_cases` 가 조립한다).
    MineScript {
        deposit_id: String,
        target_m: [f64; 3],
        /// 정지·통과 판정 반경(광맥 중심에서, m).
        stop_radius_m: f64,
        fly: FlyMode,
        steps: Vec<MineStep>,
        grace: Duration,
    },
    /// p1-02: 광맥 사거리 안으로 날아가 멈춘 뒤 `MINE_RESOURCE` 를 `mines` 회(간격 `gap`) 보낸다.
    /// 목표 좌표는 호출자가 준다(봇은 `data/` 의 광맥 위치를 **길 찾기**에만 쓴다 — 기대값이 아니다).
    MineAt {
        deposit_id: String,
        target_m: [f64; 3],
        stop_radius_m: f64,
        arrive_timeout: Duration,
        mines: u32,
        gap: Duration,
        /// 마지막 채굴의 `command_id` 를 한 번 더 보낸다(같은 세션 재전송, SC-78 (a) 경로).
        resend_last: bool,
        grace: Duration,
    },
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
    /// **SC-89 (g) 양성 대조**: 한 서버 tick 안에 `per_tick` 건을 **사이 간격 없이** 몰아 보내고
    /// `gap` 만큼 쉬는 것을 `rounds` 회 반복한다.
    ///
    /// 왜 이 모양인가 — 두 문턱을 갈라야 하기 때문이다(ADR-0011 §5.2):
    /// * **tick 당 상한(8)** 은 *한 tick 안의 건수* 를 본다 → 몰아 보내기가 이것을 넘긴다.
    /// * **`rate_limit_hz`(40)** 는 *평균 속도* 를 본다 → `gap` 이 평균을 낮춰 이쪽은 **건드리지 않는다**.
    ///
    /// 평균 속도를 낮추지 않으면 `RATE_LIMITED` 가 먼저 발동해 **위반 경로를 밟기 전에 막힌다** —
    /// 그러면 이 대조는 "프로토콜 위반이 계수된다"를 증명하지 못하고 다른 것을 증명하게 된다.
    ///
    /// 위반 예산은 10초 창에 8건이므로 `rounds >= 8` 이고 `rounds * gap < 10s` 여야 서버가 닫는다.
    TickBurst {
        per_tick: u32,
        rounds: u32,
        gap: Duration,
        grace: Duration,
    },
    /// 보내지 않고 `listen` 동안 받기만 한다(관측자·재개 2구간). `seq1_after` 가 있으면 그 시점에
    /// `input_seq = 1` 명령을 하나 보낸다(AC-3(e2): 재개 후 첫 입력이 ACCEPTED 여야 한다).
    Listen {
        listen: Duration,
        seq1_after: Option<Duration>,
    },
}

/// 채굴 케이스의 비행 방식.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlyMode {
    /// 스폰 자리에 머문다(첫 스냅샷만 기다린다).
    Stay,
    /// 광맥 반경 안으로 가서 멈춘다(`fly_to`). 도착 못 하면 단계를 실행하지 않는다.
    Stop { timeout: Duration },
    /// 멈추지 않고 광맥 쪽으로 `speed_mps` 로 날다가, 반경 안에 **들어온 순간** 단계를 실행한다.
    PassThrough { speed_mps: f64, timeout: Duration },
}

/// 채굴 케이스의 한 단계.
#[derive(Debug, Clone)]
pub enum MineStep {
    /// 새 `command_id` 로 이 광맥에 `MINE_RESOURCE`.
    Mine,
    /// `k` 번째로 보낸 채굴(0 부터)의 `command_id` 를 그대로 다시 보낸다.
    Resend(usize),
    /// 다른 `deposit_id` 로(새 id).
    MineOther(String),
    /// 원문 프레임 그대로. `{COMMAND_ID}` 는 새 id 로 바꿔 보낸다.
    Raw(String),
    Wait(Duration),
    /// `every` 간격으로 `total` 동안 새 id 채굴을 보낸다.
    Burst {
        every: Duration,
        total: Duration,
    },
    /// 같은 프로세스의 다른 봇에게 신호(허가 1 개를 남긴다 — 먼저 와도 잃지 않는다).
    Signal(std::sync::Arc<tokio::sync::Notify>),
    /// 신호를 기다린다. 기다리는 동안에도 받은 메시지는 처리한다.
    WaitSignal {
        notify: std::sync::Arc<tokio::sync::Notify>,
        timeout: Duration,
    },
    /// 스냅샷에서 그 actor 의 함선이 **보였다가 사라질 때까지** 기다린다(잔류 만료 디스폰 관측).
    WaitActorGone {
        actor: Uuid,
        timeout: Duration,
    },
    /// 정해 둔 `command_id` 로 채굴(재접속·재기동을 넘는 재전송 — 연결 밖에서 id 를 들고 온다).
    MineId(Uuid),
    /// 오케스트레이터와의 손잡기: 파일을 만든다 / 파일이 생길 때까지 기다린다(그동안 메시지 처리).
    TouchFile(std::path::PathBuf),
    WaitFile {
        path: std::path::PathBuf,
        timeout: Duration,
    },
    /// 코디네이터가 보내는 명령을 받아 그때그때 채굴한다(`Stop` 이나 채널 닫힘에서 끝).
    /// 기다리는 동안에도 받은 메시지를 처리한다.
    Remote(RemoteRx),
}

/// 원격 조종 명령(`MineStep::Remote`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCmd {
    Mine,
    /// 제자리 유지 조작 한 프레임(브레이크) — 기록 지연 중에도 이동 명령이 수락되는지(SC-30).
    Control,
    Stop,
}

/// 원격 조종 수신단 — 한 봇만 소비한다.
pub type RemoteRx =
    std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<RemoteCmd>>>;

#[derive(Debug)]
pub struct ConnectionOutcome {
    pub ledger: Ledger,
    pub snapshots: SnapshotLedger,
    /// p1-02: 채굴·역사 메시지 관측(수신 순번 포함).
    pub mining: MiningObs,
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
    /// p1-02 누출 검사(SC-40): 받은 텍스트 프레임 원문을 전부 저장한다.
    pub capture_raw: bool,
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
                mining: MiningObs::default(),
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
                mining: MiningObs::default(),
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
        mining: if spec.capture_raw {
            MiningObs::with_raw_capture()
        } else {
            MiningObs::default()
        },
        frame_seq: 0,
        clock: spec.clock,
        live_corr: spec.live_corr.clone(),
        next_seq: 0,
        connected_at_us: connect_us,
        ready_at_us: None,
        closed: false,
        own: None,
        last_actors: std::collections::BTreeSet::new(),
        last_actors_tick: None,
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
        Behavior::TickBurst {
            per_tick,
            rounds,
            gap,
            grace,
        } => conn.run_tick_burst(per_tick, rounds, gap, grace).await,
        Behavior::Listen { listen, seq1_after } => conn.run_listen(listen, seq1_after).await,
        Behavior::MineScript {
            deposit_id,
            target_m,
            stop_radius_m,
            fly,
            steps,
            grace,
        } => {
            conn.run_mine_script(&deposit_id, target_m, stop_radius_m, fly, &steps, grace)
                .await
        }
        Behavior::MineAt {
            deposit_id,
            target_m,
            stop_radius_m,
            arrive_timeout,
            mines,
            gap,
            resend_last,
            grace,
        } => {
            conn.run_mine_at(
                &deposit_id,
                target_m,
                stop_radius_m,
                arrive_timeout,
                mines,
                gap,
                resend_last,
                grace,
            )
            .await
        }
    }

    let ready_ms = conn
        .ready_at_us
        .map(|v| (v.saturating_sub(t_connect_start)) as f64 / 1000.0);
    ConnectionOutcome {
        ledger: conn.ledger,
        snapshots: conn.snapshots,
        mining: conn.mining,
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
    /// p1-02: 채굴·역사 관측.
    mining: MiningObs,
    /// 이 연결에서 받은 텍스트 프레임 순번 — 채굴 관측과 명령 결과의 순서를 잇는다.
    frame_seq: u64,
    clock: Clock,
    live_corr: Option<std::sync::Arc<LiveCorrelationSink>>,
    next_seq: u32,
    connected_at_us: u64,
    ready_at_us: Option<u64>,
    closed: bool,
    /// p1-02 비행용 — 마지막 스냅샷의 자기 함선과 그 tick.
    own: Option<(u64, ShipState)>,
    /// 마지막 스냅샷에 함선이 있던 actor 들과 그 tick(디스폰 관측).
    last_actors: std::collections::BTreeSet<Uuid>,
    last_actors_tick: Option<u64>,
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
                let frame_seq = self.frame_seq;
                self.frame_seq += 1;
                self.mining.capture_raw(frame_seq, text.as_str());
                match wire::parse_inbound(text.as_str()) {
                    Inbound::SessionReady(m) => {
                        if self.ready_at_us.is_none() {
                            self.ready_at_us = Some(at);
                            self.mining.session_ready_frame_seq = Some(frame_seq);
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
                        self.mining.results.push(ObservedResult {
                            frame_seq,
                            tick: m.tick,
                            command_id: m.payload.command_id,
                            status: m.payload.status.clone(),
                            reason_code: m.payload.reason_code.clone(),
                        });
                    }
                    Inbound::InventoryState(m) => {
                        self.mining.inventory.push(Observed {
                            frame_seq,
                            at_us: at,
                            tick: m.tick,
                            msg: *m,
                        });
                    }
                    Inbound::DepositFieldState(m) => {
                        self.mining.deposits.push(Observed {
                            frame_seq,
                            at_us: at,
                            tick: m.tick,
                            msg: *m,
                        });
                    }
                    Inbound::HistoricalEventNotice(m) => {
                        self.mining.notices.push(Observed {
                            frame_seq,
                            at_us: at,
                            tick: m.tick,
                            msg: *m,
                        });
                    }
                    Inbound::WorldSnapshot(m) => {
                        if self.ready_at_us.is_none() {
                            self.ledger.on_error(
                                "first contract message was WORLD_SNAPSHOT, not SESSION_READY"
                                    .to_owned(),
                            );
                        }
                        self.snapshots.on_snapshot(&m, text.len());
                        self.last_actors = m.payload.ships.iter().map(|s| s.actor_id).collect();
                        self.last_actors_tick = Some(m.tick);
                        self.mining.snapshot_at_us.push(at);
                        if let Some(c) = m.payload.controlled_ship_id
                            && let Some(ship) = m.payload.ships.iter().find(|s| s.ship_id == c)
                        {
                            self.own = Some((m.tick, ship.clone()));
                            self.mining.own_ship_id = Some(ship.ship_id);
                        }
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

    async fn send_mine(&mut self, command_id: Uuid, deposit_id: &str, target_m: [f64; 3]) {
        let cmd = MineResourceCommand::new(command_id, deposit_id);
        let text = match serde_json::to_string(&cmd) {
            Ok(t) => t,
            Err(e) => {
                self.ledger.on_error(format!("serialize failed: {e}"));
                return;
            }
        };
        let (tick, dist, speed) = match &self.own {
            Some((t, s)) => (
                Some(*t),
                Some(dist_m(own_pos_m(s), target_m)),
                Some(s.speed_mm_s() / 1000.0),
            ),
            None => (None, None, None),
        };
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let at = self.clock.us();
        match self.sink.send(Message::text(text)).await {
            Ok(()) => {
                self.ledger.on_sent_no_reply(command_id, seq, at);
                self.mining.mine_sent.push(SentMine {
                    command_id,
                    deposit_id: deposit_id.to_owned(),
                    at_us: at,
                    frame_seq_at_send: self.frame_seq,
                    last_snapshot_tick: tick,
                    dist_m: dist,
                    speed_mps: speed,
                });
            }
            Err(e) => {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
            }
        }
    }

    /// 광맥으로 날아가 멈춘다. 도착하면 `true`. 조작 간격 50 ms, 판단은 마지막 스냅샷으로.
    async fn fly_to(
        &mut self,
        deposit_id: &str,
        target_m: [f64; 3],
        stop_radius_m: f64,
        timeout: Duration,
    ) -> bool {
        let tuning = nav::NavTuning::default();
        let mut trace = NavTrace {
            deposit_id: deposit_id.to_owned(),
            ..NavTrace::default()
        };
        let deadline = tokio::time::Instant::now() + timeout;
        let mut seq: u64 = 0;
        // 첫 스냅샷을 기다린다.
        while self.own.is_none() && !self.closed && tokio::time::Instant::now() < deadline {
            self.pump_for(Duration::from_millis(100)).await;
        }
        let mut arrived = false;
        while !self.closed && tokio::time::Instant::now() < deadline {
            let Some((_, ship)) = self.own.clone() else {
                break;
            };
            let p = own_pos_m(&ship);
            let v = [
                f64::from(ship.velocity_x_mm_s) / 1000.0,
                f64::from(ship.velocity_y_mm_s) / 1000.0,
                f64::from(ship.velocity_z_mm_s) / 1000.0,
            ];
            let q = [
                f64::from(ship.orientation_x_micro) / 1e6,
                f64::from(ship.orientation_y_micro) / 1e6,
                f64::from(ship.orientation_z_micro) / 1e6,
                f64::from(ship.orientation_w_micro) / 1e6,
            ];
            let dist = dist_m(p, target_m);
            let speed = ship.speed_mm_s() / 1000.0;
            if trace.start_dist_m.is_none() {
                trace.start_dist_m = Some(dist);
            }
            trace.max_speed_mps = trace.max_speed_mps.max(speed);
            let cmd = nav::plan(p, v, q, target_m, stop_radius_m, &tuning);
            if cmd.arrived {
                trace.arrived_at_us = Some(self.clock.us());
                trace.arrive_dist_m = Some(dist);
                trace.arrive_speed_mps = Some(speed);
                arrived = true;
            }
            seq += 1;
            let [ax, ay, az, aw] = cmd.aim_micro;
            let ctl = SetShipControlCommand::new(Uuid::now_v7(), seq)
                .with_thrust(0, 0, cmd.thrust_z_milli)
                .with_brake(cmd.brake || arrived)
                .with_aim(ax, ay, az, aw);
            self.send_control(ctl).await;
            trace.control_frames += 1;
            if arrived {
                break;
            }
            self.pump_for(Duration::from_millis(50)).await;
        }
        self.mining.nav = Some(trace);
        arrived
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_mine_at(
        &mut self,
        deposit_id: &str,
        target_m: [f64; 3],
        stop_radius_m: f64,
        arrive_timeout: Duration,
        mines: u32,
        gap: Duration,
        resend_last: bool,
        grace: Duration,
    ) {
        if self
            .fly_to(deposit_id, target_m, stop_radius_m, arrive_timeout)
            .await
        {
            // 도착 뒤 한 박자 — 브레이크가 적용된 상태의 스냅샷을 한 장 더 받는다.
            self.pump_for(Duration::from_millis(300)).await;
            let mut last = None;
            for i in 0..mines {
                if self.closed {
                    break;
                }
                let id = Uuid::now_v7();
                self.send_mine(id, deposit_id, target_m).await;
                last = Some(id);
                if i + 1 < mines {
                    self.pump_for(gap).await;
                }
            }
            if resend_last && let Some(id) = last {
                self.pump_for(Duration::from_millis(500)).await;
                self.send_mine(id, deposit_id, target_m).await;
            }
        }
        self.pump_for(grace).await;
        if !self.closed {
            self.close_client_side().await;
        }
    }

    /// 원문 프레임을 보낸다. 채굴 원장(`mine_sent`)에 id 와 원문을 남긴다.
    async fn send_raw_mine(
        &mut self,
        command_id: Uuid,
        text: String,
        deposit_id: &str,
        target_m: [f64; 3],
    ) {
        let (tick, dist, speed) = self.own_fix(target_m);
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        let at = self.clock.us();
        match self.sink.send(Message::text(text.clone())).await {
            Ok(()) => {
                self.ledger.on_sent_no_reply(command_id, seq, at);
                self.mining.mine_sent.push(SentMine {
                    command_id,
                    deposit_id: deposit_id.to_owned(),
                    at_us: at,
                    frame_seq_at_send: self.frame_seq,
                    last_snapshot_tick: tick,
                    dist_m: dist,
                    speed_mps: speed,
                });
                self.mining.raw_sent.push((command_id, text));
            }
            Err(e) => {
                self.ledger.on_error(format!("send failed: {e}"));
                self.closed = true;
            }
        }
    }

    fn own_fix(&self, target_m: [f64; 3]) -> (Option<u64>, Option<f64>, Option<f64>) {
        match &self.own {
            Some((t, s)) => (
                Some(*t),
                Some(dist_m(own_pos_m(s), target_m)),
                Some(s.speed_mm_s() / 1000.0),
            ),
            None => (None, None, None),
        }
    }

    /// 멈추지 않고 광맥 쪽으로 날다가 반경 안에 들어오면 `true`.
    async fn pass_through(
        &mut self,
        target_m: [f64; 3],
        radius_m: f64,
        speed_mps: f64,
        timeout: Duration,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        while self.own.is_none() && !self.closed && tokio::time::Instant::now() < deadline {
            self.pump_for(Duration::from_millis(100)).await;
        }
        let mut seq: u64 = 0;
        let mut trace = NavTrace::default();
        while !self.closed && tokio::time::Instant::now() < deadline {
            let Some((_, ship)) = self.own.clone() else {
                break;
            };
            let p = own_pos_m(&ship);
            let d = dist_m(p, target_m);
            let v = ship.speed_mm_s() / 1000.0;
            if trace.start_dist_m.is_none() {
                trace.start_dist_m = Some(d);
            }
            trace.max_speed_mps = trace.max_speed_mps.max(v);
            if d <= radius_m {
                trace.arrived_at_us = Some(self.clock.us());
                trace.arrive_dist_m = Some(d);
                trace.arrive_speed_mps = Some(v);
                self.mining.nav = Some(trace);
                return true;
            }
            let rel = [target_m[0] - p[0], target_m[1] - p[1], target_m[2] - p[2]];
            let aim = nav::aim_toward(rel);
            let q = [
                f64::from(ship.orientation_x_micro) / 1e6,
                f64::from(ship.orientation_y_micro) / 1e6,
                f64::from(ship.orientation_z_micro) / 1e6,
                f64::from(ship.orientation_w_micro) / 1e6,
            ];
            let f = nav::forward_of(q);
            let cos = (f[0] * rel[0] + f[1] * rel[1] + f[2] * rel[2]) / d.max(1e-9);
            let thrust = if v < speed_mps && cos > 15f64.to_radians().cos() {
                1000
            } else {
                0
            };
            // 목표 속도를 크게 넘으면 브레이크(선회 중 과속 방지).
            let brake = v > speed_mps * 1.5;
            let a = aim.map(|c| (c * 1_000_000.0).round() as i32);
            seq += 1;
            let ctl = SetShipControlCommand::new(Uuid::now_v7(), seq)
                .with_thrust(0, 0, thrust)
                .with_brake(brake)
                .with_aim(a[0], a[1], a[2], a[3]);
            self.send_control(ctl).await;
            trace.control_frames += 1;
            self.pump_for(Duration::from_millis(50)).await;
        }
        self.mining.nav = Some(trace);
        false
    }

    async fn run_mine_script(
        &mut self,
        deposit_id: &str,
        target_m: [f64; 3],
        stop_radius_m: f64,
        fly: FlyMode,
        steps: &[MineStep],
        grace: Duration,
    ) {
        let ready = match fly {
            FlyMode::Stay => {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
                while self.own.is_none() && !self.closed && tokio::time::Instant::now() < deadline {
                    self.pump_for(Duration::from_millis(100)).await;
                }
                self.own.is_some()
            }
            FlyMode::Stop { timeout } => {
                let ok = self
                    .fly_to(deposit_id, target_m, stop_radius_m, timeout)
                    .await;
                if ok {
                    self.pump_for(Duration::from_millis(300)).await;
                }
                ok
            }
            FlyMode::PassThrough { speed_mps, timeout } => {
                self.pass_through(target_m, stop_radius_m, speed_mps, timeout)
                    .await
            }
        };
        if ready {
            let mut ids: Vec<Uuid> = Vec::new();
            for step in steps {
                if self.closed {
                    break;
                }
                match step {
                    MineStep::Mine => {
                        let id = Uuid::now_v7();
                        self.send_mine(id, deposit_id, target_m).await;
                        ids.push(id);
                    }
                    MineStep::Resend(k) => {
                        if let Some(id) = ids.get(*k).copied() {
                            self.send_mine(id, deposit_id, target_m).await;
                        }
                    }
                    MineStep::MineOther(other) => {
                        let id = Uuid::now_v7();
                        self.send_mine(id, other, target_m).await;
                        ids.push(id);
                    }
                    MineStep::Raw(template) => {
                        let id = Uuid::now_v7();
                        let text = template.replace("{COMMAND_ID}", &id.to_string());
                        self.send_raw_mine(id, text, deposit_id, target_m).await;
                        ids.push(id);
                    }
                    MineStep::Wait(d) => self.pump_for(*d).await,
                    MineStep::Signal(n) => n.notify_one(),
                    MineStep::Remote(rx) => {
                        let rx = rx.clone();
                        let mut rx = rx.lock().await;
                        let mut remote_seq: u64 = 0;
                        loop {
                            if self.closed {
                                break;
                            }
                            tokio::select! {
                                cmd = rx.recv() => match cmd {
                                    Some(RemoteCmd::Mine) => {
                                        let id = Uuid::now_v7();
                                        self.send_mine(id, deposit_id, target_m).await;
                                        ids.push(id);
                                    }
                                    Some(RemoteCmd::Control) => {
                                        // fly_to 의 input_seq(작은 수)와 겹치지 않는 큰 수에서 시작한다.
                                        remote_seq += 1;
                                        let cid = Uuid::now_v7();
                                        let ctl = SetShipControlCommand::new(cid, 10_000_000 + remote_seq)
                                            .with_brake(true);
                                        let at = self.clock.us();
                                        self.send_control(ctl).await;
                                        self.mining.control_sent.push((cid, at));
                                    }
                                    Some(RemoteCmd::Stop) | None => break,
                                },
                                () = self.pump_for(Duration::from_millis(50)) => {}
                            }
                        }
                    }
                    MineStep::MineId(id) => {
                        self.send_mine(*id, deposit_id, target_m).await;
                        ids.push(*id);
                    }
                    MineStep::TouchFile(path) => {
                        if let Err(e) = std::fs::write(path, b"ready\n") {
                            self.ledger
                                .on_error(format!("touch {} 실패: {e}", path.display()));
                        }
                    }
                    MineStep::WaitFile { path, timeout } => {
                        let deadline = tokio::time::Instant::now() + *timeout;
                        while !path.exists()
                            && !self.closed
                            && tokio::time::Instant::now() < deadline
                        {
                            self.pump_for(Duration::from_millis(100)).await;
                        }
                        if !path.exists() {
                            self.mining.signal_timeouts += 1;
                        }
                    }
                    MineStep::WaitSignal { notify, timeout } => {
                        let deadline = tokio::time::Instant::now() + *timeout;
                        let notified = notify.notified();
                        tokio::pin!(notified);
                        loop {
                            if self.closed || tokio::time::Instant::now() >= deadline {
                                self.mining.signal_timeouts += 1;
                                break;
                            }
                            tokio::select! {
                                () = &mut notified => break,
                                () = self.pump_for(Duration::from_millis(100)) => {}
                            }
                        }
                    }
                    MineStep::WaitActorGone { actor, timeout } => {
                        let deadline = tokio::time::Instant::now() + *timeout;
                        while !self.closed && tokio::time::Instant::now() < deadline {
                            let present = self.last_actors.contains(actor);
                            if present {
                                self.mining.actor_seen = true;
                            } else if self.mining.actor_seen && self.last_actors_tick.is_some() {
                                self.mining.actor_gone_at_us = Some(self.clock.us());
                                self.mining.actor_gone_tick = self.last_actors_tick;
                                break;
                            }
                            self.pump_for(Duration::from_millis(100)).await;
                        }
                    }
                    MineStep::Burst { every, total } => {
                        let end = tokio::time::Instant::now() + *total;
                        while !self.closed && tokio::time::Instant::now() < end {
                            let id = Uuid::now_v7();
                            self.send_mine(id, deposit_id, target_m).await;
                            ids.push(id);
                            self.pump_for(*every).await;
                        }
                    }
                }
            }
        }
        self.pump_for(grace).await;
        // 연결이 끝까지 열려 있었는지는 판정 입력이다(SC-76) — 닫기 전에 기록한다.
        self.mining.open_at_end = Some(!self.closed);
        if !self.closed {
            self.close_client_side().await;
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

    /// SC-89 (g): 한 tick 에 `per_tick` 건 → `gap` 휴식 → `rounds` 회.
    ///
    /// **서버가 먼저 닫기를 기다린다** — 이 대조가 보이려는 것이 *서버의 종료* 이므로
    /// 클라이언트 쪽에서 닫으면 `close_reason` 이 `CLIENT_CLOSED` 가 되어 아무것도 증명하지 못한다.
    async fn run_tick_burst(&mut self, per_tick: u32, rounds: u32, gap: Duration, grace: Duration) {
        let mut seq: u64 = 0;
        for _round in 0..rounds {
            if self.closed {
                break;
            }
            // 사이에 await 지점을 두지 않는다(pump 하지 않는다) — 같은 서버 tick 에 얹히는 것이 목적이다.
            for _ in 0..per_tick {
                seq += 1;
                let cmd = SetShipControlCommand::new(Uuid::now_v7(), seq).with_thrust(0, 0, 1000);
                self.send_control(cmd).await;
                if self.closed {
                    break;
                }
            }
            self.pump_for(gap).await;
        }
        // 서버가 닫을 시간을 준다. 닫지 않으면 그 사실 자체가 대조의 결과다.
        self.pump_for(grace).await;
        if !self.closed {
            self.close_client_side().await;
        }
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

/// 스냅샷 좌표(mm) → m.
fn own_pos_m(s: &ShipState) -> [f64; 3] {
    [
        s.position_x_mm as f64 / 1000.0,
        s.position_y_mm as f64 / 1000.0,
        s.position_z_mm as f64 / 1000.0,
    ]
}

fn dist_m(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
