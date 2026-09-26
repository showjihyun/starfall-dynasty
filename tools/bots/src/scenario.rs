//! 스펙 §7 의 부하 단계 A·B·C·D 와 서버 단독 항목용 probe.
//!
//! 시드는 **초기 위상 지터**에만 쓴다. 30봇이 같은 순간에 ping 을 몰아 보내면 우리가 만든
//! 인공적 동시성을 측정하게 된다. 시드를 인자로 받고 요약에 남기므로 재현 가능하다(스펙 §8).

use std::path::PathBuf;
use std::time::Duration;

use std::sync::Arc;

use crate::conn::{
    Behavior, BotSpec, Clock, ConnectionOutcome, LiveCorrelationSink, run_connection,
};
use crate::token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// A 정상 상태 — 봇 30 + (밖에서 붙은) Unity 1
    Steady,
    /// B 회전 — 접속/종료를 반복해 세션 이벤트를 만든다
    Churn,
    /// C 백프레셔 — 정상 29 + 폭주 1
    Backpressure,
    /// D 기록 내구성 — A 와 같은 부하를 길게. 그 사이 QA 가 DB 를 멈춘다
    Durability,
    /// **p1-01 비행 부하** — 모든 봇이 `SET_SHIP_CONTROL` 로 전방 추력을 넣는다.
    Fly,
}

/// 봇 `i` 의 조작 송신 주기. 목록이 짧으면 마지막 값을 쓴다.
/// `0` 이하가 들어오면 `main` 이 이미 거부하므로 여기서는 방어만 한다.
pub fn send_interval(send_hz: &[f64], i: usize) -> Duration {
    let hz = send_hz
        .get(i)
        .or_else(|| send_hz.last())
        .copied()
        .unwrap_or(20.0);
    Duration::from_secs_f64(1.0 / if hz > 0.0 { hz } else { 20.0 })
}

impl Stage {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "a" | "steady" => Some(Self::Steady),
            "b" | "churn" => Some(Self::Churn),
            "c" | "backpressure" => Some(Self::Backpressure),
            "d" | "durability" => Some(Self::Durability),
            "e" | "fly" | "move" => Some(Self::Fly),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steady => "a-steady",
            Self::Churn => "b-churn",
            Self::Backpressure => "c-backpressure",
            Self::Durability => "d-durability",
            Self::Fly => "e-fly",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub stage: Stage,
    pub url: String,
    pub secret: String,
    pub bots: usize,
    pub seed: u64,
    pub duration: Duration,
    pub interval: Duration,
    pub ramp: Duration,
    pub cycles: u32,
    pub pings: u32,
    pub burst: u32,
    /// **봇별 `SET_SHIP_CONTROL` 송신 주기(Hz)** — 계약 §3.1 의 `--send-hz`, 기본 20.
    ///
    /// **한 실행 안에서 봇마다 다른 값을 줄 수 있어야 한다**(SC-25: 20 Hz 대 200 Hz).
    /// 실행을 둘로 나누면 서버 부하·tick 구간·`summary.json` 이 갈려, **거리 차이가 났을 때
    /// "전송률 때문"인지 "실행이 달라서"인지 구분할 근거가 사라진다** — SC-25 가 재려는 것은
    /// 속도 핵 방어이므로 두 봇은 같은 조건에 있어야 한다(리더 판정 R24).
    ///
    /// 봇 수보다 짧으면 **마지막 값이 나머지 봇에 적용된다**(`[20, 200]` + `--bots 2` 가 표준
    /// 사용법이고, `[20]` + `--bots 30` 이면 전원 20 Hz 다).
    pub send_hz: Vec<f64>,
    pub out: PathBuf,
    /// SC-61 용: SESSION_READY 를 받는 즉시 correlation 을 여기에 덧붙인다.
    /// 실행이 끝난 뒤 쓰는 `correlations.txt` 로는 "실행 중" 조회를 할 수 없다.
    pub live_corr: Option<PathBuf>,
}

/// SplitMix64. 표준 라이브러리만으로 결정적 지터를 만들기 위한 것이고,
/// **판정에 쓰이는 값은 하나도 여기서 나오지 않는다**(위상만 흔든다).
#[derive(Debug)]
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// 0..max (배타) 범위의 값.
    pub fn below(&mut self, max: u64) -> u64 {
        if max == 0 { 0 } else { self.next_u64() % max }
    }
}

/// 실행 결과와 그 실행의 시계를 함께 돌려준다 — 시계가 없으면 `*_us` 를 벽시계로 옮길 수 없고,
/// AC-19 의 "중단 구간" 슬라이싱이 불가능해진다.
pub async fn run(cfg: &RunConfig) -> (Clock, Vec<ConnectionOutcome>) {
    let clock = Clock::start();
    let mut rng = SplitMix64::new(cfg.seed);
    let mut handles = Vec::with_capacity(cfg.bots);
    let live = cfg
        .live_corr
        .as_ref()
        .and_then(|p| match LiveCorrelationSink::create(p) {
            Ok(sink) => Some(Arc::new(sink)),
            Err(e) => {
                eprintln!(
                    "[bots] live correlation 파일을 열 수 없다 {}: {e}",
                    p.display()
                );
                None
            }
        });

    for i in 0..cfg.bots {
        let label = token::bot_label(i);
        let (_subject, tok) = token::identity(&cfg.secret, &label);
        let phase = Duration::from_micros(rng.below(cfg.interval.as_micros() as u64));
        // 램프: 봇 i 는 ramp * i / bots 만큼 늦게 시작한다. A 단계의 "5초 안에 접속"이 여기서 나온다.
        let stagger = if cfg.bots > 1 {
            Duration::from_micros((cfg.ramp.as_micros() as u64) * i as u64 / cfg.bots as u64)
        } else {
            Duration::ZERO
        };

        let behavior = match cfg.stage {
            Stage::Steady | Stage::Durability => Behavior::Steady {
                interval: cfg.interval,
                duration: cfg.duration,
                phase,
            },
            Stage::Churn => Behavior::Burst {
                pings: cfg.pings,
                grace: Duration::from_millis(1500),
            },
            Stage::Fly => Behavior::Fly {
                // **`cfg.interval` 이 아니다** — 그것은 ping 주기이고 기본값이 500 ms(2 Hz) 라
                // 조작 송신에 쓰면 계약이 말하는 20 Hz 와 무관한 수를 재게 된다.
                interval: send_interval(&cfg.send_hz, i),
                duration: cfg.duration,
                // 전방(로컬 +Z) 최대 추력. ADR-0009 §1의 축 규약.
                thrust: (0, 0, 1000),
                brake_last: Duration::ZERO,
                phase,
            },
            Stage::Backpressure => {
                if i + 1 == cfg.bots {
                    Behavior::Flood {
                        count: cfg.burst,
                        grace: Duration::from_secs(5),
                    }
                } else {
                    Behavior::Steady {
                        interval: cfg.interval,
                        duration: cfg.duration,
                        phase,
                    }
                }
            }
        };

        let spec = BotSpec {
            label,
            url: cfg.url.clone(),
            token: tok,
            behavior,
            clock,
            live_corr: live.clone(),
        };
        let cycles = if cfg.stage == Stage::Churn {
            cfg.cycles
        } else {
            1
        };

        handles.push(tokio::spawn(async move {
            tokio::time::sleep(stagger).await;
            let mut outcomes = Vec::new();
            for _ in 0..cycles {
                let this = BotSpec {
                    label: spec.label.clone(),
                    url: spec.url.clone(),
                    token: spec.token.clone(),
                    behavior: spec.behavior.clone(),
                    clock: spec.clock,
                    live_corr: spec.live_corr.clone(),
                };
                outcomes.push(run_connection(this).await);
            }
            outcomes
        }));
    }

    let mut all = Vec::new();
    for h in handles {
        match h.await {
            Ok(mut v) => all.append(&mut v),
            Err(e) => eprintln!("bot task panicked: {e}"),
        }
    }
    (clock, all)
}

/// p0-02 서버 단독 항목(p0-02 SC-14·19·20·21·22·24·25·26)의 재현용 — 괄호 안 번호는
/// p0-02 계약 번호이지 p1-01 번호가 아니다(architect·qa 조사, `_workspace/p1-01-ship-movement/01_architect_decisions.md` R23 후속).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCase {
    AuthOk,
    Order,
    Duplicate,
    InFlight,
    SlowConsumer,
    Oversize,
    Binary,
    Idle,
    // ── p1-01 치트 7종 ──────────────────────────────────────────────────────
    /// 조작 입력을 정상 주기로 보낸다(관측용).
    Fly,
    /// 위치 필드 주입 (계약 반례 원문 그대로).
    CheatPosition,
    /// 현재 자세 필드 주입.
    CheatAttitude,
    /// 조작 값 범위 초과 (`thrust_z_milli = 5000`).
    CheatRange,
    /// `input_seq` 역행·반복.
    CheatSeq,
    /// `SESSION_READY` 직후 폭주 (10배 속도).
    CheatFlood,
    /// **SC-89 (g) 양성 대조**: 한 tick 에 9건을 몰아 보내기를 반복해 위반 예산을 채운다.
    TickBurst,
    /// 접속 직후, `SESSION_READY` 를 기다리지 않고 전송.
    CheatPreReady,
    /// SC-24 (c)(d) 한 실행: 유효 → 범위 초과 → 유효 재개 → `aim_*` 극단값. 분석은 `range_turn`.
    CheatRangeTurn,
}

/// 이 probe 가 계약 항목에 대해 **무엇을 주장하는가** — 출처 게이트가 산문이 아니라 이
/// 타입을 읽는다(architect R28). `참고:`/`관측용:` 같은 접두 문자열을 게이트가 부분문자열로
/// 면제하던 옛 규칙은 설명 문장에 우연히 그 두 글자가 들어간 verdict 라벨도 조용히 면제하는
/// 구멍이었다 — 타입으로 가르면 오타는 컴파일러가 잡는다.
#[derive(Debug, Clone, Copy)]
pub enum ContractRef {
    /// 이 probe 가 그 항목의 **공식 verdict 도구**다 — 계약 §1 이 이 케이스 이름을
    /// (백틱으로) 적어야 한다.
    Verdict(&'static str),
    /// 관련 항목을 가리키는 표식. verdict 를 주장하지 않으므로 계약의 지명을 요구하지 않는다.
    Reference(&'static str),
}

impl std::fmt::Display for ContractRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Verdict(s) => write!(f, "[verdict] {s}"),
            Self::Reference(s) => write!(f, "[reference] {s}"),
        }
    }
}

impl ProbeCase {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "auth-ok" => Some(Self::AuthOk),
            "order" => Some(Self::Order),
            "duplicate" => Some(Self::Duplicate),
            "inflight" | "in-flight" => Some(Self::InFlight),
            "slow-consumer" => Some(Self::SlowConsumer),
            "oversize" => Some(Self::Oversize),
            "binary" => Some(Self::Binary),
            "idle" => Some(Self::Idle),
            "fly" => Some(Self::Fly),
            "cheat-position" => Some(Self::CheatPosition),
            "cheat-attitude" => Some(Self::CheatAttitude),
            "cheat-range" => Some(Self::CheatRange),
            "cheat-seq" => Some(Self::CheatSeq),
            "cheat-flood" => Some(Self::CheatFlood),
            "tick-burst" => Some(Self::TickBurst),
            "pre-ready" => Some(Self::CheatPreReady),
            "cheat-range-turn" => Some(Self::CheatRangeTurn),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AuthOk => "auth-ok",
            Self::Order => "order",
            Self::Duplicate => "duplicate",
            Self::InFlight => "inflight",
            Self::SlowConsumer => "slow-consumer",
            Self::Oversize => "oversize",
            Self::Binary => "binary",
            Self::Idle => "idle",
            Self::Fly => "fly",
            Self::CheatPosition => "cheat-position",
            Self::CheatAttitude => "cheat-attitude",
            Self::CheatRange => "cheat-range",
            Self::CheatSeq => "cheat-seq",
            Self::CheatFlood => "cheat-flood",
            Self::TickBurst => "tick-burst",
            Self::CheatPreReady => "pre-ready",
            Self::CheatRangeTurn => "cheat-range-turn",
        }
    }

    /// 이 probe 가 어떤 스프린트 계약 항목의 증거인가 — 출력에 같이 찍어 리포트로 옮기기 쉽게.
    ///
    /// **architect R28**: 팔마다 문자열 리터럴을 **정확히 하나**만 둔다(`format!`·연결 금지) —
    /// 게이트가 팔의 첫 리터럴만 읽으므로 둘째 리터럴의 `SC-`번호는 보이지 않고 조용히
    /// 통과한다(실측으로 드러난 구멍). `ContractRef::{Verdict, Reference}` 가 verdict 여부를
    /// 들고, `참고:` 산문 접두는 **사람이 읽으라고** 그대로 두되 게이트는 의존하지 않는다.
    pub fn contract_item(self) -> ContractRef {
        match self {
            Self::AuthOk => ContractRef::Reference(
                "참고: 첫 메시지가 SESSION_READY, actor_id == 토큰 주체 (p0-02 SC-14/AC-5a 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::Order => ContractRef::Reference(
                "참고: COMMAND_RESULT → PING_REPLY 순서, 같은 tick (p0-02 SC-19/AC-6c 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::Duplicate => ContractRef::Reference(
                "참고: ACCEPTED 1 + PING_REPLY 1 + DUPLICATE_COMMAND_ID 1 (p0-02 SC-21/AC-7b 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::InFlight => ContractRef::Reference(
                "참고: TOO_MANY_IN_FLIGHT, 연결 유지, 보낸 수 == 받은 수 (p0-02 SC-20/AC-7a 재현용 — p1-01 에서는 tick 당 상한(MAX_COMMANDS_PER_SESSION_PER_TICK)이 먼저 걸려 구조적 미도달, 계약 §1 D절 머리말)",
            ),
            Self::SlowConsumer => ContractRef::Reference(
                "참고: DB 의 close_reason=SLOW_CONSUMER 가 정본, close code 1011 은 RST 로 유실될 수 있다 (p0-02 SC-22/AC-7c 재현용 — p1-01 SC-22 는 퇴화 쿼터니언 항목으로 무관, p1-01 계약에 대응 항목 없음)",
            ),
            Self::Oversize => ContractRef::Reference(
                "참고: 위반 계수 후 close 1002 + PROTOCOL_VIOLATION (p0-02 SC-24/AC-8a 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::Binary => ContractRef::Reference(
                "참고: 바이너리 프레임도 같은 예산, close 1002 (p0-02 SC-25/AC-8b 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::Idle => ContractRef::Reference(
                "참고: Pong 없음 → close 1001 + IDLE_TIMEOUT (p0-02 SC-26/AC-8c 재현용 — p1-01 계약에 대응 항목 없음)",
            ),
            Self::Fly => ContractRef::Reference("SC-61/62 관측용: SET_SHIP_CONTROL 정상 주기 전송"),
            // 계약 §1 SC-23·SC-66 이 `cheat-position`·`cheat-attitude` 를 **각각 백틱으로**
            // 지명한다(architect R28 §6 계약 수정) — 진짜 verdict 다.
            Self::CheatPosition => ContractRef::Verdict(
                "SC-23/SC-66 (AC-5a/AC-17a): 위치 필드 주입 → MALFORMED_COMMAND, 상태 변화 0",
            ),
            Self::CheatAttitude => ContractRef::Verdict(
                "SC-23/SC-66 (AC-5a/AC-17a): 현재 자세 필드 주입 → MALFORMED_COMMAND",
            ),
            // 계약 §1 SC-24 검증 방법 칸이 `cheat-range` 를 명시적으로 지명한다 — 진짜 verdict.
            // (예전에 "참고:" 로 강등했던 것은 출처 게이트가 `.rs` 를 못 읽던 시절의 임시
            // 우회였고, R28 의 `ContractRef` 타입 구분으로 더는 필요 없다.) SC-67 은 별개
            // 항목(공식 verdict 도구 = `cargo test ack_input_seq`)이라 겸해 내지 않는다.
            Self::CheatRange => ContractRef::Verdict(
                "SC-24 (c)(d)(AC-5c/AC-5d): 범위 초과 → 거부 + 이월로 조작 유지",
            ),
            // 내용은 p1-01 SC-67 (input_seq 역행) 과 같지만, 계약이 지명한 공식 verdict 도구는
            // `cargo test -p starfall-sim --locked ack_input_seq` 다. 이 probe 는 재현용이라
            // SC-67 을 verdict 로 내지 않는다.
            Self::CheatSeq => ContractRef::Reference(
                "참고: input_seq 역행 → STALE_INPUT, ack 후퇴 없음 (p1-01 SC-67/AC-17d 재현용 — 공식 verdict 도구는 cargo test)",
            ),
            // `SC-20` 은 p0-02 번호다(p0-02 SC-20 = in-flight 폭주, 연결 유지 — 내용이 일치한다.
            // p1-01 SC-20 은 브레이크 감쇠 항목이라 무관). `SC-25` 는 **어느 계약에도 없는
            // 번호**다 — p1-01 SC-25 는 속도 핵 거리 대조, p0-02 SC-25 는 바이너리 프레임
            // 예산으로 **둘 다 이 라벨의 내용(SLOW_CONSUMER 로 안 끊긴다)과 무관하다**.
            // 내용상 p0-02 SC-22(SLOW_CONSUMER 판정 정본)의 오타로 보이지만 **추정이고
            // 확인되지 않았다** — 다음 사람이 "p0-02 번호구나"로 뭉개지 않도록 남긴다.
            Self::CheatFlood => ContractRef::Reference(
                "참고: 폭주 — 큰 burst 가 SLOW_CONSUMER 로 끊기지 않는다 (SC-20 은 p0-02 SC-20/AC-6 재현용. SC-25 는 p1-01·p0-02 어느 계약에도 없는 번호 — 내용상 p0-02 SC-22 의 오타로 추정되나 미확인. p1-01 계약에 대응 항목 없음)",
            ),
            // 계약 §1 SC-89 (g) 가 `probe --case tick-burst` 를 백틱으로 지명한다.
            Self::TickBurst => ContractRef::Verdict(
                "SC-89 (g) 양성 대조: 한 tick 에 9건 × N회 → 서버가 위반을 계수하고 예산이 차면 1002 로 닫는다",
            ),
            // 계약 §1 SC-68 (g) 가 `= pre-ready` 를 백틱으로 지명한다.
            Self::CheatPreReady => ContractRef::Verdict(
                "SC-68 (AC-17g): SESSION_READY 이전 전송이 함선을 만들거나 움직이지 않는다",
            ),
            // 계약 §1 SC-24 검증 방법 칸이 `cheat-range-turn` 도 지명한다(cheat-range 와 함께).
            // **SC-67 을 내지 않는다** — `CheatRange`·`CheatSeq` 두 형제 팔이 이미 "SC-67 의
            // 공식 verdict 도구는 cargo test" 라고 정했는데 이 팔만 SC-67 을 같이 내면 같은
            // 파일 안에서 같은 번호에 세 팔이 다르게 행동하는 모순이 된다(architect R28 §2).
            // (b) 범위 초과를 실제로 돌린다는 사실은 이 주석과 아래 문구에 남긴다.
            Self::CheatRangeTurn => ContractRef::Verdict(
                "SC-24 (c)(d) 재확인: 유효 → 범위 초과 → 유효 재개 → aim 극단값 한 실행에서, 거부 + 이월 지속과 tick 당 회전 ≤ 한도를 함께 본다",
            ),
        }
    }
}

/// 치트 프레임 원문. **계약 반례 파일을 그대로** 보낸다 — 봇의 타입을 거치면
/// "어휘에 없는 필드"를 시험할 수 없다(직렬화 단계에서 우리가 먼저 막아 버린다).
fn cheat_frames(case: ProbeCase, count: u32) -> Vec<String> {
    let rel = match case {
        ProbeCase::CheatPosition => {
            "contracts/fixtures/SET_SHIP_CONTROL/invalid/position-field-injected.json"
        }
        ProbeCase::CheatAttitude => {
            "contracts/fixtures/SET_SHIP_CONTROL/invalid/attitude-field-injected.json"
        }
        ProbeCase::CheatRange => {
            "contracts/fixtures/SET_SHIP_CONTROL/invalid/thrust-above-range.json"
        }
        _ => return Vec::new(),
    };
    let Some(root) = repo_root() else {
        eprintln!("[bots] 레포 루트를 찾지 못했다 — 치트 프레임을 읽을 수 없다");
        return Vec::new();
    };
    let path = root.join(rel);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            // command_id 만 매번 새로 만든다. 나머지 필드는 원문 그대로 둔다.
            (0..count)
                .map(|_| match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(mut v) => {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "command_id".to_owned(),
                                serde_json::json!(uuid::Uuid::now_v7().to_string()),
                            );
                        }
                        v.to_string()
                    }
                    Err(_) => text.clone(),
                })
                .collect()
        }
        Err(e) => {
            eprintln!("[bots] 치트 프레임을 읽지 못했다 {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// `contracts/registry/types.json` 을 마커로 레포 루트를 찾는다(p0-02 테스트와 같은 방식).
fn repo_root() -> Option<PathBuf> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("contracts/registry/types.json").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub async fn run_probe(
    case: ProbeCase,
    url: &str,
    secret: &str,
    label: &str,
    count: u32,
) -> ConnectionOutcome {
    let clock = Clock::start();
    let (_subject, tok) = token::identity(secret, label);
    let behavior = match case {
        ProbeCase::AuthOk => Behavior::Burst {
            pings: 0,
            grace: Duration::from_millis(500),
        },
        ProbeCase::Order => Behavior::Burst {
            pings: count.max(1),
            grace: Duration::from_millis(1500),
        },
        ProbeCase::Duplicate => Behavior::Duplicate,
        ProbeCase::InFlight => Behavior::Flood {
            count: count.max(500),
            grace: Duration::from_secs(5),
        },
        // server 실측: 2261건에서 닫혔다. 여유를 두고 3000 이상 보낸다.
        // 판정 정본은 close code 1011 이 아니라 **DB 의 close_reason** 이다(RST 로 유실될 수 있다).
        ProbeCase::SlowConsumer => Behavior::SlowConsumer {
            count: count.max(3000),
            wait: Duration::from_secs(20),
        },
        ProbeCase::Oversize => Behavior::Violate {
            oversize: true,
            count: count.max(9),
        },
        ProbeCase::Binary => Behavior::Violate {
            oversize: false,
            count: count.max(9),
        },
        // tokio-tungstenite 는 폴링하면 자동 Pong 을 보낸다(U-9 = 참). 30초 이상
        // **폴링 자체를 멈춰야** 서버의 유휴 타임아웃이 발동한다(server 주의).
        ProbeCase::Idle => Behavior::Idle {
            wait: Duration::from_secs(45),
        },
        ProbeCase::Fly => Behavior::Fly {
            interval: Duration::from_millis(50),
            duration: Duration::from_secs(u64::from(count.max(10))),
            thrust: (0, 0, 1000),
            brake_last: Duration::ZERO,
            phase: Duration::ZERO,
        },
        ProbeCase::CheatPosition | ProbeCase::CheatAttitude => Behavior::CheatRaw {
            frames: cheat_frames(case, count.max(5)),
            gap: Duration::from_millis(120),
            grace: Duration::from_secs(2),
        },
        ProbeCase::CheatRange => Behavior::CheatRaw {
            frames: cheat_frames(case, count.max(5)),
            gap: Duration::from_millis(120),
            grace: Duration::from_secs(2),
        },
        ProbeCase::CheatSeq => Behavior::CheatSeqRewind {
            rounds: count.max(3),
        },
        ProbeCase::CheatFlood => Behavior::Fly {
            // 정상 주기의 10배. AC-6의 "빨리 보내도 더 가지 못한다".
            interval: Duration::from_millis(5),
            duration: Duration::from_secs(u64::from(count.max(30))),
            thrust: (0, 0, 1000),
            brake_last: Duration::ZERO,
            phase: Duration::ZERO,
        },
        ProbeCase::CheatPreReady => Behavior::PreReady {
            count: count.max(5),
        },
        ProbeCase::TickBurst => {
            let plan = tick_burst_plan(count);
            Behavior::TickBurst {
                per_tick: plan.per_tick,
                rounds: plan.rounds,
                gap: plan.gap,
                grace: plan.grace,
            }
        }
        // count = 주입 프레임 수. 위반 예산(10초 8건) 아래로 묶는다.
        ProbeCase::CheatRangeTurn => Behavior::RangeTurn {
            frames: cheat_frames(ProbeCase::CheatRange, count.clamp(1, 6)),
            lead: Duration::from_millis(1000),
            turn_hold: Duration::from_millis(3000),
        },
    };
    run_connection(BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior,
        clock,
        live_corr: None,
    })
    .await
}

// ── SC-89 (g) 양성 대조의 계획 ────────────────────────────────────────────────

/// 서버 쪽 문턱. **여기서 읽고, 테스트도 여기서 읽는다** — 숫자를 테스트에 복사해 두면
/// 그 테스트는 사본을 검사하게 된다(계약 §7a, SC-89 (c) 와 같은 규율).
///
/// **⚠ 두 층이 있고 둘 다 "tick 당 건수"를 본다**(qa3 가 2026-09-23 실측 뒤 코드로 확인):
///
/// | 층 | 상한 | 초과하면 |
/// |---|---|---|
/// | **게이트웨이** | `MAX_COMMANDS_PER_SESSION_PER_TICK = 8` | 제출하지 않는다. `COMMAND_RESULT` 도 없다. `commands_dropped_over_tick_cap_total`++ 와 **그 tick 에 프로토콜 위반 1회**(명령 1건당이 아니다) |
/// | **시뮬레이션** | `rate_limit_per_tick_cap = rate_limit_hz.div_ceil(tick_hz)` = 40/20 = **2** | `RATE_LIMITED` 거부 + `COMMAND_RESULT` |
///
/// **그래서 `RATE_LIMITED` 는 "평균 속도" 문턱이 아니다.** 한 tick 에 몰아 보내면 **두 층이
/// 반드시 함께 걸린다** — 간격을 벌려 평균을 낮춰도 갈라지지 않는다. `RATE_LIMITED` 가
/// 틱상한 드롭보다 **많이 나오는 것이 정상**이고, 그 비는 아래에서 유도된다.
pub const SERVER_TICK_COMMAND_CAP: u32 = 8;
pub const VIOLATION_BUDGET: u32 = 8;
pub const VIOLATION_WINDOW: Duration = Duration::from_secs(10);
/// 시뮬레이션 층의 tick 당 처리 상한 (`rate_limit_hz` 40 / `tick_hz` 20).
pub const SIM_RATE_LIMIT_PER_TICK_CAP: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickBurstPlan {
    pub per_tick: u32,
    pub rounds: u32,
    pub gap: Duration,
    pub grace: Duration,
}

impl TickBurstPlan {
    /// 이 계획이 **한 tick 안에서** 상한을 넘기는가.
    pub fn exceeds_tick_cap(self) -> bool {
        self.per_tick > SERVER_TICK_COMMAND_CAP
    }

    /// 위반 창 안에 예산을 **채우는가** — 라운드가 충분하고 창 안에 들어가는가.
    pub fn fills_violation_budget(self) -> bool {
        self.rounds >= VIOLATION_BUDGET
            && self.gap.saturating_mul(VIOLATION_BUDGET.saturating_sub(1)) < VIOLATION_WINDOW
    }

    /// 한 라운드가 **게이트웨이 층에서 버려지는** 건수 = 위반 계수 1회의 근거.
    pub fn expected_tick_cap_drops_per_round(self) -> u32 {
        self.per_tick.saturating_sub(SERVER_TICK_COMMAND_CAP)
    }

    /// 한 라운드가 **시뮬레이션 층에서 `RATE_LIMITED`** 로 거부되는 건수.
    /// 게이트웨이를 통과한 것(= `min(per_tick, 8)`) 중 `2` 를 넘는 것이다.
    pub fn expected_rate_limited_per_round(self) -> u32 {
        self.per_tick
            .min(SERVER_TICK_COMMAND_CAP)
            .saturating_sub(SIM_RATE_LIMIT_PER_TICK_CAP)
    }

    /// **이 대조가 위반 경로를 밟는가** — 게이트웨이 상한을 넘겨 위반이 계수되는가.
    ///
    /// *`RATE_LIMITED` 가 함께 나는 것은 결함이 아니다.* 두 층이 모두 tick 당 건수를 보므로
    /// 몰아 보내기는 **반드시 둘 다** 건드린다. 갈라야 하는 것은 두 층이 아니라
    /// **"위반이 실제로 계수됐는가"** 이고, 그것은 `protocol_violations_total` 로 직접 본다.
    pub fn charges_protocol_violations(self) -> bool {
        self.expected_tick_cap_drops_per_round() > 0
    }
}

/// `--count` 를 계획으로 바꾼다.
pub fn tick_burst_plan(count: u32) -> TickBurstPlan {
    TickBurstPlan {
        // 상한 8 이므로 9 건이면 그 tick 에 위반 1 건이 매겨진다.
        per_tick: SERVER_TICK_COMMAND_CAP + 1,
        // 예산이 정확히 8 이라 문턱에 걸치지 않도록 여유를 둔다.
        rounds: count.clamp(1, 40).max(VIOLATION_BUDGET + 2),
        // 라운드를 서로 다른 tick 에 떨어뜨리고, 예산 창(10 s) 안에 `rounds` 회가 들어가게 한다.
        // **평균 속도를 낮추려는 값이 아니다** — 두 층 다 tick 당 건수를 보므로 간격으로는
        // 갈라지지 않는다(위 상수 주석).
        gap: Duration::from_millis(500),
        grace: Duration::from_secs(3),
    }
}
