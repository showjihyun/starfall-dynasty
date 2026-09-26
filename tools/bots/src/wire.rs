//! 봇이 보는 와이어 타입 — **서버 코드와 독립으로 쓴 것**.
//!
//! # 왜 `starfall-contracts` 를 path 의존하지 않는가
//!
//! `01_architect_tasks.md` T12 는 "`starfall-contracts` 를 path 의존"이라고 적었다. 여기서는
//! 그렇게 하지 않고 같은 모양을 독립으로 썼다. 이유 두 가지:
//!
//! 1. **관측자의 독립성**(스펙 I-25). 봇은 3자 대조의 한 축이다. 서버가 쓰는 바로 그 serde
//!    타입으로 서버의 출력을 읽으면, 타입이 틀렸을 때 봇도 똑같이 틀려서 아무 차이도 나지 않는다.
//!    같은 실수를 두 곳에서 하면 대조가 통과한다.
//! 2. 계약이 코드보다 앞서 있어 `server/crates/contracts` 가 지금 바뀌는 중이다. 여기에 묶이면
//!    서버가 컴파일되지 않는 동안 QA 도구도 서지 않는다.
//!
//! 대신 **정본과의 일치는 `tests/wire_fixtures.rs` 가 계약 fixture 로 직접 검증한다** —
//! `contracts/fixtures/**` 를 이 타입들로 역직렬화 → 재직렬화 → 원본과 `Value` 비교한다.
//! 즉 봇은 서버가 아니라 **계약**에 맞춰져 있고, 그것이 독립 관측의 조건이다.
//!
//! 이 결정은 architect 통지 대상이다(`04_qa_report_r1.md` 에 기록).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// 계약 커버리지 스크립트(`check_contract_coverage.py`)가 `tools/bots/**/*.rs` 에서
// 이 리터럴들을 찾는다. 레지스트리의 `bots` 태그 4건이 여기에 대응한다.
pub const PING_SERVER: &str = "PING_SERVER";
pub const PING_REPLY: &str = "PING_REPLY";
pub const COMMAND_RESULT: &str = "COMMAND_RESULT";
pub const SESSION_READY: &str = "SESSION_READY";
// p1-01 신규 2종.
pub const SET_SHIP_CONTROL: &str = "SET_SHIP_CONTROL";
pub const WORLD_SNAPSHOT: &str = "WORLD_SNAPSHOT";

/// 명령 envelope (contracts/common/command-envelope.schema.json).
///
/// `client_sent_at` 은 **키가 항상 존재하고 값이 null 일 수 있다**(I-5). 그래서
/// `skip_serializing_if` 를 쓰지 않는다. 서버는 이 값을 판정·정렬에 쓰지 않는다(I-11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingServerCommand {
    pub command_id: Uuid,
    pub command_type: String,
    pub schema_version: u32,
    pub client_sent_at: Option<String>,
    pub payload: PingServerPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingServerPayload {
    pub probe_seq: u32,
}

impl PingServerCommand {
    pub fn new(command_id: Uuid, probe_seq: u32) -> Self {
        Self {
            command_id,
            command_type: PING_SERVER.to_owned(),
            schema_version: 1,
            // 봇은 실제 시각을 넣지 않는다. 왕복은 봇의 단조 시계로 재고(ADR-0006 §4),
            // 이 필드는 서버 로그 전용이다. null 을 넣어 "판정에 쓰지 않는다"를 구조로 만든다.
            client_sent_at: None,
            payload: PingServerPayload { probe_seq },
        }
    }
}

/// 서버 메시지 envelope. `sequence`·`world_id`·`occurred_at` 은 **없다**
/// (도메인 이벤트 전용 — 스펙 §5.2). 여기 넣으면 계약과 어긋난다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: SessionReadyPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyPayload {
    pub session_id: Uuid,
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub tick_hz: u32,
    pub server_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandResultMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: CommandResultPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandResultPayload {
    pub command_id: Uuid,
    /// `ACCEPTED` | `REJECTED`. **닫힌 열거형으로 만들지 않는다** — 봇은 소비자이고,
    /// 값이 추가되어도 죽지 않아야 한다(ADR-0005 §4의 C# 쪽과 같은 이유).
    /// 모르는 값은 `summary.json` 의 `unknown_status` 로 드러난다.
    pub status: String,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingReplyMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: PingReplyPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PingReplyPayload {
    pub command_id: Uuid,
    pub probe_seq: u32,
}

/// 조작 명령 (p1-01). **위치·속도·현재 자세 필드가 없다** — 어휘에 없는 것이 I-26의 방어다.
///
/// 봇은 양자화 정수를 **직접** 만든다. 서버의 변환 헬퍼를 쓰면 서버가 자기 변환을 자기가
/// 확인하는 꼴이 되어 독립 출처가 아니게 된다(I-25).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SetShipControlCommand {
    pub command_id: Uuid,
    pub command_type: String,
    pub schema_version: u32,
    pub client_sent_at: Option<String>,
    pub payload: SetShipControlPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SetShipControlPayload {
    pub input_seq: u64,
    pub thrust_x_milli: i32,
    pub thrust_y_milli: i32,
    pub thrust_z_milli: i32,
    pub roll_milli: i32,
    pub aim_x_micro: i32,
    pub aim_y_micro: i32,
    pub aim_z_micro: i32,
    pub aim_w_micro: i32,
    pub brake: bool,
    pub flight_assist: bool,
}

/// 항등 자세(`w = 1.0`)의 마이크로 표기. ADR-0009 §2의 배율 1e6.
pub const AIM_IDENTITY_W_MICRO: i32 = 1_000_000;

impl SetShipControlCommand {
    /// 항등 자세·보조 켜짐·브레이크 없음의 기본 명령.
    pub fn new(command_id: Uuid, input_seq: u64) -> Self {
        Self {
            command_id,
            command_type: SET_SHIP_CONTROL.to_owned(),
            schema_version: 1,
            client_sent_at: None,
            payload: SetShipControlPayload {
                input_seq,
                thrust_x_milli: 0,
                thrust_y_milli: 0,
                thrust_z_milli: 0,
                roll_milli: 0,
                aim_x_micro: 0,
                aim_y_micro: 0,
                aim_z_micro: 0,
                aim_w_micro: AIM_IDENTITY_W_MICRO,
                brake: false,
                flight_assist: true,
            },
        }
    }

    pub fn with_thrust(mut self, x: i32, y: i32, z: i32) -> Self {
        self.payload.thrust_x_milli = x;
        self.payload.thrust_y_milli = y;
        self.payload.thrust_z_milli = z;
        self
    }

    pub fn with_brake(mut self, brake: bool) -> Self {
        self.payload.brake = brake;
        self
    }

    pub fn with_roll(mut self, roll_milli: i32) -> Self {
        self.payload.roll_milli = roll_milli;
        self
    }

    pub fn with_assist(mut self, flight_assist: bool) -> Self {
        self.payload.flight_assist = flight_assist;
        self
    }

    /// 목표 자세(마이크로 단위 쿼터니언 `x, y, z, w`).
    pub fn with_aim(mut self, x: i32, y: i32, z: i32, w: i32) -> Self {
        self.payload.aim_x_micro = x;
        self.payload.aim_y_micro = y;
        self.payload.aim_z_micro = z;
        self.payload.aim_w_micro = w;
        self
    }
}

/// 월드 스냅샷 (p1-01). `ships` 는 계약에서 **배열을 쓰는 첫 타입**이다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshotMessage {
    pub message_id: Uuid,
    pub message_type: String,
    pub schema_version: u32,
    pub tick: u64,
    pub correlation_id: Option<Uuid>,
    pub payload: WorldSnapshotPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshotPayload {
    pub star_system_id: String,
    pub soft_boundary_radius_mm: i64,
    pub hard_boundary_radius_mm: i64,
    pub snapshot_interval_ticks: u32,
    /// **널 가능**: 이 세션의 actor 가 아직 함선이 없으면 `null` 이다
    /// (`WORLD_SNAPSHOT/empty-nulls-and-bounds.json` 이 그 경우다).
    pub controlled_ship_id: Option<Uuid>,
    pub ack_input_seq: Option<u64>,
    pub ships: Vec<ShipState>,
}

/// 함선 1척의 상태. **18필드** — `angular_velocity_roll_mdeg_s` 가 별도 필드인 것이 핵심이다
/// (B-1). 월드 각속도 3성분에서 롤을 분해하면 손실 분해가 되어 선회 중 예측이 어긋난다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShipState {
    pub ship_id: Uuid,
    pub actor_id: Uuid,
    pub ship_class_id: String,
    /// `ACTIVE` | `LINGERING`. **닫힌 열거형으로 만들지 않는다** — 봇은 소비자이고 값이
    /// 추가되어도 죽지 않아야 한다(ADR-0005 §4).
    pub presence: String,
    pub position_x_mm: i64,
    pub position_y_mm: i64,
    pub position_z_mm: i64,
    pub velocity_x_mm_s: i32,
    pub velocity_y_mm_s: i32,
    pub velocity_z_mm_s: i32,
    pub orientation_x_micro: i32,
    pub orientation_y_micro: i32,
    pub orientation_z_micro: i32,
    pub orientation_w_micro: i32,
    pub angular_velocity_x_mdeg_s: i32,
    pub angular_velocity_y_mdeg_s: i32,
    pub angular_velocity_z_mdeg_s: i32,
    pub angular_velocity_roll_mdeg_s: i32,
}

impl ShipState {
    /// 원점 기준 거리 (mm). 부동소수를 쓰지 않고 정수로 제곱합을 구한 뒤 한 번만 변환한다.
    pub fn radius_mm(&self) -> f64 {
        let x = self.position_x_mm as f64;
        let y = self.position_y_mm as f64;
        let z = self.position_z_mm as f64;
        (x * x + y * y + z * z).sqrt()
    }

    /// 두 상태 사이의 이동 거리 (mm).
    pub fn distance_mm(&self, other: &ShipState) -> f64 {
        let dx = (self.position_x_mm - other.position_x_mm) as f64;
        let dy = (self.position_y_mm - other.position_y_mm) as f64;
        let dz = (self.position_z_mm - other.position_z_mm) as f64;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn speed_mm_s(&self) -> f64 {
        let x = self.velocity_x_mm_s as f64;
        let y = self.velocity_y_mm_s as f64;
        let z = self.velocity_z_mm_s as f64;
        (x * x + y * y + z * z).sqrt()
    }

    pub fn is_lingering(&self) -> bool {
        self.presence == "LINGERING"
    }
}

/// 수신 프레임 1개를 해석한 결과.
///
/// `Unknown` 은 **경고이지 오류가 아니다**(ADR-0005 §5 와 같은 관용). `Malformed` 는
/// 계수되어 `summary.json` 의 `wire_errors` 로 드러난다 — 조용히 버리면 손실 계측이 거짓이 된다.
#[derive(Debug, Clone)]
pub enum Inbound {
    SessionReady(Box<SessionReadyMessage>),
    CommandResult(Box<CommandResultMessage>),
    PingReply(Box<PingReplyMessage>),
    WorldSnapshot(Box<WorldSnapshotMessage>),
    Unknown { message_type: String },
    Malformed { reason: String, raw: String },
}

/// `message_type` 을 먼저 보고(peek) 그 타입으로만 역직렬화한다.
/// `#[serde(flatten)]` 이나 내부 태그 열거형을 쓰지 않는 이유는 계약 규약과 같다
/// (ADR-0002 §3: `deny_unknown_fields` 가 무력화된다).
pub fn parse_inbound(text: &str) -> Inbound {
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Inbound::Malformed {
                reason: format!("not json: {e}"),
                raw: truncate(text),
            };
        }
    };
    let Some(mt) = value.get("message_type").and_then(|v| v.as_str()) else {
        return Inbound::Malformed {
            reason: "no message_type".to_owned(),
            raw: truncate(text),
        };
    };
    match mt {
        SESSION_READY => match serde_json::from_value(value) {
            Ok(m) => Inbound::SessionReady(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("SESSION_READY: {e}"),
                raw: truncate(text),
            },
        },
        COMMAND_RESULT => match serde_json::from_value(value) {
            Ok(m) => Inbound::CommandResult(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("COMMAND_RESULT: {e}"),
                raw: truncate(text),
            },
        },
        PING_REPLY => match serde_json::from_value(value) {
            Ok(m) => Inbound::PingReply(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("PING_REPLY: {e}"),
                raw: truncate(text),
            },
        },
        WORLD_SNAPSHOT => match serde_json::from_value(value) {
            Ok(m) => Inbound::WorldSnapshot(Box::new(m)),
            Err(e) => Inbound::Malformed {
                reason: format!("WORLD_SNAPSHOT: {e}"),
                raw: truncate(text),
            },
        },
        other => Inbound::Unknown {
            message_type: other.to_owned(),
        },
    }
}

fn truncate(s: &str) -> String {
    const MAX: usize = 400;
    if s.len() <= MAX {
        s.to_owned()
    } else {
        let mut end = MAX;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}
