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
}

impl Stage {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "a" | "steady" => Some(Self::Steady),
            "b" | "churn" => Some(Self::Churn),
            "c" | "backpressure" => Some(Self::Backpressure),
            "d" | "durability" => Some(Self::Durability),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Steady => "a-steady",
            Self::Churn => "b-churn",
            Self::Backpressure => "c-backpressure",
            Self::Durability => "d-durability",
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

/// 서버 단독 항목(SC-14·19·21·22·24·25·26)의 재현용.
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
        }
    }

    /// 이 probe 가 어떤 스프린트 계약 항목의 증거인가 — 출력에 같이 찍어 리포트로 옮기기 쉽게.
    pub fn contract_item(self) -> &'static str {
        match self {
            Self::AuthOk => "SC-14 (AC-5a): 첫 메시지가 SESSION_READY, actor_id == 토큰 주체",
            Self::Order => "SC-19 (AC-6c): COMMAND_RESULT → PING_REPLY 순서, 같은 tick",
            Self::Duplicate => "SC-21 (AC-7b): ACCEPTED 1 + PING_REPLY 1 + DUPLICATE_COMMAND_ID 1",
            Self::InFlight => "SC-20 (AC-7a): TOO_MANY_IN_FLIGHT, 연결 유지, 보낸 수 == 받은 수",
            Self::SlowConsumer => {
                "SC-22 (AC-7c): DB 의 close_reason=SLOW_CONSUMER 가 정본 (close code 1011 은 RST 로 유실될 수 있다)"
            }
            Self::Oversize => "SC-24 (AC-8a): 위반 계수 후 close 1002 + PROTOCOL_VIOLATION",
            Self::Binary => "SC-25 (AC-8b): 바이너리 프레임도 같은 예산, close 1002",
            Self::Idle => "SC-26 (AC-8c): Pong 없음 → close 1001 + IDLE_TIMEOUT",
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
