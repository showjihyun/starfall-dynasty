//! 서버 메시지 타입 — 서버 → 클라이언트 실시간 메시지.
//!
//! envelope 을 펼쳐 쓰는 이유는 [`crate::commands`] 모듈 문서와 같다.

use serde::{Deserialize, Deserializer, Serialize};

use crate::primitives::{
    AngularVelocityMdegPerSecond, ConstSchemaVersion, DataId, InputSeq, PositionMm, ProbeSeq,
    QuaternionComponentMicro, ServerVersion, Tick, TickHz, UuidV7, VelocityMmPerSecond,
    required_nullable,
};

/// `PING_REPLY` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PingReplyType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "PING_REPLY")]
    PingReply,
}

/// `PING_REPLY` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingReplyPayload {
    /// 응답 대상 `PING_SERVER` 의 `command_id`.
    ///
    /// 응답 대응은 envelope 의 `correlation_id` 가 아니라 이 필드로 한다 — envelope 쪽은
    /// "하나의 게임플레이 트랜잭션"을 묶는 용도다.
    pub command_id: UuidV7,
    /// `PING_SERVER` payload 에서 그대로 복사한 값.
    pub probe_seq: ProbeSeq,
}

/// `PING_REPLY` — `PING_SERVER` 에 대한 서버 응답.
///
/// 대응 스키마: `contracts/messages/PING_REPLY.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PingReplyMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `PING_REPLY`.
    pub message_type: PingReplyType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 이 메시지를 만든 서버 시뮬레이션 tick.
    pub tick: Tick,
    /// 이 메시지가 속한 게임플레이 트랜잭션, 또는 null.
    ///
    /// 키는 항상 존재한다 (I-5).
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: PingReplyPayload,
}

// ---------------------------------------------------------------------------
// COMMAND_RESULT
// ---------------------------------------------------------------------------

/// `COMMAND_RESULT` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CommandResultType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "COMMAND_RESULT")]
    CommandResult,
}

/// 명령 판정 결과.
///
/// `ACCEPTED` 는 **시뮬레이션에 들어갔다**는 뜻이지 의도가 완료됐다는 뜻이 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandStatus {
    /// envelope 의 tick 에서 시뮬레이션에 접수됐다.
    Accepted,
    /// 접수되지 않았다. `reason_code` 가 이유이고 월드 상태는 바뀌지 않았다.
    Rejected,
}

/// 거부 사유. `schema_version` 1의 닫힌 집합.
///
/// # 왜 Rust 는 닫힌 열거형이고 C# 은 `string` 인가 (ADR-0005 §4)
///
/// 서버는 모르는 값을 **거부**해야 하고(클라이언트를 믿지 않는다), 클라이언트는 모르는 값을
/// 받아도 **죽지 않아야 한다**(서버가 먼저 배포된다). 값 추가는 같은 `schema_version` 의
/// 호환 변경이다 — 그래서 여기 변형을 추가하는 것은 계약 변경이 아니라 계약 **따라가기**다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectReasonCode {
    /// 프레임을 계약 타입으로 읽을 수 없었다.
    MalformedCommand,
    /// 레지스트리에 없는 `command_type`.
    UnknownCommandType,
    /// 서버가 지원하지 않는 `schema_version`.
    SchemaVersionUnsupported,
    /// 세션 안에서 이미 처리한 `command_id`.
    DuplicateCommandId,
    /// 전역 명령 큐가 가득 찼다.
    ServerBusy,
    /// 세션의 미응답 명령 상한을 넘었다.
    TooManyInFlight,
    /// 지속 초과 전송(`rate_limit_hz` 초과). p1-01 신규.
    RateLimited,
    /// `input_seq` 가 이미 적용한 값보다 크지 않다. p1-01 신규(ADR-0011 §4).
    StaleInput,
}

/// `COMMAND_RESULT` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResultPayload {
    /// 답하는 명령의 `command_id`. 서버는 이 값을 **멱등 키로만** 쓴다 (I-11).
    pub command_id: UuidV7,
    /// 판정 결과.
    pub status: CommandStatus,
    /// `ACCEPTED` 면 `null`, `REJECTED` 면 사유 (I-14).
    ///
    /// 스키마는 이 상관을 표현하지 못한다(ADR-0005 §4). 생성 측은
    /// [`CommandResultPayload::accepted`] / [`CommandResultPayload::rejected`] 만 쓴다.
    #[serde(deserialize_with = "required_nullable")]
    pub reason_code: Option<RejectReasonCode>,
}

impl CommandResultPayload {
    /// 접수 결과. `reason_code` 는 언제나 `None` 이다 (I-14).
    #[must_use]
    pub const fn accepted(command_id: UuidV7) -> Self {
        Self {
            command_id,
            status: CommandStatus::Accepted,
            reason_code: None,
        }
    }

    /// 거부 결과. `reason_code` 는 언제나 `Some` 이다 (I-14).
    #[must_use]
    pub const fn rejected(command_id: UuidV7, reason: RejectReasonCode) -> Self {
        Self {
            command_id,
            status: CommandStatus::Rejected,
            reason_code: Some(reason),
        }
    }

    /// I-14 를 만족하는가. 테스트와 발행 직전 단언에 쓴다.
    #[must_use]
    pub const fn invariant_holds(&self) -> bool {
        matches!(
            (self.status, self.reason_code),
            (CommandStatus::Accepted, None) | (CommandStatus::Rejected, Some(_))
        )
    }
}

/// `COMMAND_RESULT` — 명령 1건의 판정 결과.
///
/// 대응 스키마: `contracts/messages/COMMAND_RESULT.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandResultMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `COMMAND_RESULT`.
    pub message_type: CommandResultType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 판정한 tick. 큐에 넣지 못해 게이트웨이가 만든 거부는 게이트웨이가 읽은 현재 tick.
    pub tick: Tick,
    /// **이 슬라이스에서는 언제나 `null`** (스펙 §5.2). 명령은 게임플레이 트랜잭션을
    /// 시작하지 않았고, 세션 correlation 을 여기 넣으면 I-12 가 경고한 오용의 여지가 생긴다.
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: CommandResultPayload,
}

// ---------------------------------------------------------------------------
// SESSION_READY
// ---------------------------------------------------------------------------

/// `SESSION_READY` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SessionReadyType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SESSION_READY")]
    SessionReady,
}

/// `SESSION_READY` payload.
///
/// 클라이언트는 여기 있는 값 중 **어느 것도 주장하지 않는다.** 전부 통보받는다 (I-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyPayload {
    /// 이 연결의 세션 id. 재연결은 **새 세션**이다 (I-23).
    pub session_id: UuidV7,
    /// 세션이 붙은 월드.
    pub world_id: UuidV7,
    /// 검증된 자격 증명에서 **서버가 정한** 행위자 (I-10).
    pub actor_id: UuidV7,
    /// 이 월드의 tick 주기. 월드 수명 동안 불변 (I-19).
    pub tick_hz: TickHz,
    /// 서버 빌드 버전. 로그·버그 리포트 전용.
    pub server_version: ServerVersion,
}

/// `SESSION_READY` — 인증된 실시간 연결의 **첫 계약 메시지**.
///
/// 대응 스키마: `contracts/messages/SESSION_READY.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionReadyMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `SESSION_READY`.
    pub message_type: SessionReadyType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 세션을 수락한 tick. `SESSION_OPENED` 의 tick 과 같다.
    pub tick: Tick,
    /// 세션 correlation. `SESSION_OPENED`·`SESSION_CLOSED` 와 **같은 값**이다.
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: SessionReadyPayload,
}

// ---------------------------------------------------------------------------
// WORLD_SNAPSHOT
// ---------------------------------------------------------------------------

/// `WORLD_SNAPSHOT` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WorldSnapshotType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "WORLD_SNAPSHOT")]
    WorldSnapshot,
}

/// 함선의 존재 상태. `ACTIVE` 는 조종사가 있다, `LINGERING` 은 세션이 끝나고 잔류 중이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShipPresence {
    /// 살아 있는 세션이 조종 중이다.
    Active,
    /// 세션이 끝나고 잔류 창 안에서 표류 중이다.
    Lingering,
}

/// 배열 원소 하나 — 함선 한 척의 권위 상태(양자화됨, ADR-0009).
///
/// 각속도는 **둘**이다 — `angular_velocity_{x,y,z}_mdeg_s`(`ω_aim`, 월드 프레임)와
/// `angular_velocity_roll_mdeg_s`(`ω_roll`, 전방축 스칼라). 합에서 복원할 수 없다
/// (ADR-0010 §1.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipState {
    /// 함선 엔티티 식별자. 캐릭터 ≠ 함선(GDD §4) — 잔류 재개에도 이 값이 그대로다.
    pub ship_id: UuidV7,
    /// 이 함선이 속한 행위자.
    pub actor_id: UuidV7,
    /// `SHIP_CLASS` 테이블의 행.
    pub ship_class_id: DataId,
    /// `ACTIVE` 또는 `LINGERING`.
    pub presence: ShipPresence,
    /// 위치 X.
    pub position_x_mm: PositionMm,
    /// 위치 Y.
    pub position_y_mm: PositionMm,
    /// 위치 Z.
    pub position_z_mm: PositionMm,
    /// 속도 X. 타 함선 외삽에 필요하다.
    pub velocity_x_mm_s: VelocityMmPerSecond,
    /// 속도 Y.
    pub velocity_y_mm_s: VelocityMmPerSecond,
    /// 속도 Z.
    pub velocity_z_mm_s: VelocityMmPerSecond,
    /// 자세 x.
    pub orientation_x_micro: QuaternionComponentMicro,
    /// 자세 y.
    pub orientation_y_micro: QuaternionComponentMicro,
    /// 자세 z.
    pub orientation_z_micro: QuaternionComponentMicro,
    /// 자세 w.
    pub orientation_w_micro: QuaternionComponentMicro,
    /// `ω_aim` 월드 프레임 각속도 x. **롤을 포함하지 않는다.**
    pub angular_velocity_x_mdeg_s: AngularVelocityMdegPerSecond,
    /// `ω_aim` y.
    pub angular_velocity_y_mdeg_s: AngularVelocityMdegPerSecond,
    /// `ω_aim` z.
    pub angular_velocity_z_mdeg_s: AngularVelocityMdegPerSecond,
    /// `ω_roll` — 전방축 둘레 롤 각속도. 별도 스칼라로 싣는다(ADR-0010 §2).
    pub angular_velocity_roll_mdeg_s: AngularVelocityMdegPerSecond,
}

/// `1 ..= 20_000_000` (mm) — float32 예산이 강제하는 경계 상한(ADR-0009 §3).
fn de_boundary_radius_mm<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = i64::deserialize(deserializer)?;
    if (1..=20_000_000).contains(&raw) {
        Ok(raw)
    } else {
        Err(serde::de::Error::custom(format!(
            "경계 반경은 1 ..= 20000000 mm 여야 한다 (받음: {raw})"
        )))
    }
}

/// `1 ..= 255` — `snapshot_interval_ticks`.
fn de_snapshot_interval_ticks<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = u16::deserialize(deserializer)?;
    if (1..=255).contains(&raw) {
        Ok(raw)
    } else {
        Err(serde::de::Error::custom(format!(
            "snapshot_interval_ticks 는 1 ..= 255 여야 한다 (받음: {raw})"
        )))
    }
}

/// `WORLD_SNAPSHOT` payload. 월드부(수신자 공통) + 세션부(수신자별).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshotPayload {
    /// 이 좌표가 속한 성계.
    pub star_system_id: DataId,
    /// soft 경계. 이 밖이면 원점 방향 당김이 더해진다.
    #[serde(deserialize_with = "de_boundary_radius_mm")]
    pub soft_boundary_radius_mm: i64,
    /// hard 경계. 벽. 상한은 float32 렌더 예산이지 설계 선택이 아니다(ADR-0009 §3).
    #[serde(deserialize_with = "de_boundary_radius_mm")]
    pub hard_boundary_radius_mm: i64,
    /// 연속 스냅샷 사이의 tick 간격.
    #[serde(deserialize_with = "de_snapshot_interval_ticks")]
    pub snapshot_interval_ticks: u16,
    /// 이 세션이 조종 중인 함선, 또는 없으면 `null`.
    #[serde(deserialize_with = "required_nullable")]
    pub controlled_ship_id: Option<UuidV7>,
    /// 서버가 이 세션에 대해 마지막으로 적용한 `input_seq`, 또는 아직 없으면 `null`.
    #[serde(deserialize_with = "required_nullable")]
    pub ack_input_seq: Option<InputSeq>,
    /// 월드의 모든 함선(잔류 포함), `ship_id` 오름차순(I-37).
    pub ships: Vec<ShipState>,
}

/// `WORLD_SNAPSHOT` — 그 tick 의 모든 함선의 권위 상태.
///
/// 대응 스키마: `contracts/messages/WORLD_SNAPSHOT.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldSnapshotMessage {
    /// 추적·로그 상관용 서버 생성 UUIDv7.
    pub message_id: UuidV7,
    /// 언제나 `WORLD_SNAPSHOT`.
    pub message_type: WorldSnapshotType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 이 스냅샷을 만든 tick.
    pub tick: Tick,
    /// 이 슬라이스에서는 언제나 `null`(스펙 §5.2).
    #[serde(deserialize_with = "required_nullable")]
    pub correlation_id: Option<UuidV7>,
    /// 타입별 payload.
    pub payload: WorldSnapshotPayload,
}
