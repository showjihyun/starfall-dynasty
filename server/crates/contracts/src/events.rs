//! 도메인 이벤트 타입 — **세계에서 실제로 일어난 일**.
//!
//! envelope 을 펼쳐 쓰는 이유는 [`crate::commands`] 모듈 문서와 같다.
//!
//! # 메시지 envelope 과 이벤트 envelope 은 다른 타입이다
//!
//! 이벤트 envelope 의 `correlation_id` 는 **널이 아니다**(시스템 프로세스도 자기 correlation 을
//! 갖는다). 메시지 envelope 의 것은 널 가능이다. 둘을 같은 타입으로 만들면 널 가능성이
//! 한쪽으로 뭉개진다 (스프린트 계약 §2 "좁힘 2행").
//!
//! 그리고 `sequence`·`occurred_at`·`recorded_at`·`world_id`·`actor_id` 는 **도메인 이벤트에만**
//! 있다. 서버 메시지가 `sequence` 공간을 같이 쓰면 `domain_events` 의 `sequence` 에 구멍이
//! 생겨 AC-16(c) 가 항상 실패한다 (I-18).
//!
//! # 이 두 타입은 Historical Event 가 아니다
//!
//! 접속은 사건이 아니다 (스펙 §6, 중요도 Level 0). 여기 기록하는 것은 **존재 구간**이고,
//! 중요도 판정기의 기본값은 "판정하지 않음"이어야 한다.

use serde::{Deserialize, Serialize};

use crate::primitives::{
    ConstSchemaVersion, DataId, GameTime, PositionMm, QuaternionComponentMicro, RealTime, Sequence,
    Tick, UuidV7, required_nullable,
};

/// 세션이 수립된 전송 수단. 닫힌 집합(현재 1종).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionTransport {
    /// WebSocket (ADR-0005 §1).
    Websocket,
}

/// 세션이 닫힌 이유. **세계의 사실**이며 WebSocket close code 와 다른 어휘다 (ADR-0005 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionCloseReason {
    /// 클라이언트가 정상 종료했다.
    ClientClosed,
    /// 30초간 Pong·데이터 프레임이 없었다.
    IdleTimeout,
    /// 10초 창에서 프로토콜 위반 예산을 넘겼다.
    ProtocolViolation,
    /// 송신 큐가 가득 찼다 — 느린 소비자.
    SlowConsumer,
    /// 서버가 정상 종료했다.
    ServerShutdown,
    /// 소켓 오류.
    TransportError,
    /// 같은 actor 가 더 새 세션을 열어 이 세션의 함선을 같은 tick에 넘겨받았다(잔류 없이,
    /// 사용자 결정 5, ADR-0011 §6.3). **이 사유만** `causation_id` 가 비-null이고, 그 값은
    /// 넘겨받은 `SESSION_OPENED.event_id` 다.
    Superseded,
}

/// `SESSION_OPENED` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SessionOpenedType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SESSION_OPENED")]
    SessionOpened,
}

/// `SESSION_CLOSED` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SessionClosedType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SESSION_CLOSED")]
    SessionClosed,
}

/// `SESSION_OPENED` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionOpenedPayload {
    /// 이 이벤트가 여는 세션. `SESSION_READY` 가 클라이언트에게 알린 값과 같다.
    pub session_id: UuidV7,
    /// 전송 수단.
    pub transport: SessionTransport,
}

/// `SESSION_CLOSED` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionClosedPayload {
    /// 닫히는 세션.
    pub session_id: UuidV7,
    /// 닫힌 이유. "떠났다"와 "잘렸다"는 다른 사실이다.
    pub close_reason: SessionCloseReason,
}

/// `SESSION_OPENED` — 인증된 행위자가 tick T부터 세계에 존재했다.
///
/// 대응 스키마: `contracts/events/domain/SESSION_OPENED.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionOpenedEvent {
    /// 저장·전달 멱등 키. 재생·결정성 비교는 `(world_id, tick, sequence)` 로 한다.
    pub event_id: UuidV7,
    /// 언제나 `SESSION_OPENED`.
    pub event_type: SessionOpenedType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 발행한 tick.
    pub tick: Tick,
    /// 그 tick 안의 발행 순서. 0부터 빈틈없이 (I-18).
    pub sequence: Sequence,
    /// `tick` 에서 결정적으로 파생한 게임 시간 (I-19).
    pub occurred_at: GameTime,
    /// 영속화 단계의 실제 시각. 감사 전용.
    pub recorded_at: RealTime,
    /// 세션 correlation. **널이 아니다.**
    pub correlation_id: UuidV7,
    /// 이 슬라이스에서는 언제나 `null`.
    #[serde(deserialize_with = "required_nullable")]
    pub causation_id: Option<UuidV7>,
    /// 행위자. **스키마가 비-null 로 좁혔다** — 세션에는 언제나 인증된 행위자가 있다.
    pub actor_id: UuidV7,
    /// 타입별 payload.
    pub payload: SessionOpenedPayload,
}

/// `SESSION_CLOSED` — 존재 구간의 끝과 그 이유.
///
/// 대응 스키마: `contracts/events/domain/SESSION_CLOSED.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionClosedEvent {
    /// 저장·전달 멱등 키.
    pub event_id: UuidV7,
    /// 언제나 `SESSION_CLOSED`.
    pub event_type: SessionClosedType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 발행한 tick.
    pub tick: Tick,
    /// 그 tick 안의 발행 순서.
    pub sequence: Sequence,
    /// 게임 시간.
    pub occurred_at: GameTime,
    /// 실제 시각. 감사 전용.
    pub recorded_at: RealTime,
    /// 짝이 되는 `SESSION_OPENED` 와 **같은** correlation (I-16).
    pub correlation_id: UuidV7,
    /// 이 슬라이스에서는 언제나 `null`.
    #[serde(deserialize_with = "required_nullable")]
    pub causation_id: Option<UuidV7>,
    /// 행위자. 비-null 로 좁혀졌다.
    pub actor_id: UuidV7,
    /// 타입별 payload.
    pub payload: SessionClosedPayload,
}

// ---------------------------------------------------------------------------
// SHIP_SPAWNED / SHIP_DESPAWNED
// ---------------------------------------------------------------------------

/// `SHIP_SPAWNED` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShipSpawnedType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SHIP_SPAWNED")]
    ShipSpawned,
}

/// `SHIP_DESPAWNED` 의 타입 상수.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ShipDespawnedType {
    /// 유일한 값.
    #[default]
    #[serde(rename = "SHIP_DESPAWNED")]
    ShipDespawned,
}

/// 함선이 사라진 이유. 닫힌 집합(현재 2종).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DespawnReason {
    /// 잔류 창이 만료됐다.
    LingerExpired,
    /// 서버가 정상 종료했다.
    ServerShutdown,
}

/// `SHIP_SPAWNED` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipSpawnedPayload {
    /// 새 함선 엔티티.
    pub ship_id: UuidV7,
    /// 함선이 스폰된 세션. `SESSION_OPENED` 와 같은 값.
    pub session_id: UuidV7,
    /// `SHIP_CLASS` 테이블의 행.
    pub ship_class_id: DataId,
    /// 위치가 속한 성계.
    pub star_system_id: DataId,
    /// 스폰 위치 X.
    pub position_x_mm: PositionMm,
    /// 스폰 위치 Y.
    pub position_y_mm: PositionMm,
    /// 스폰 위치 Z.
    pub position_z_mm: PositionMm,
    /// 스폰 자세 x.
    pub orientation_x_micro: QuaternionComponentMicro,
    /// 스폰 자세 y.
    pub orientation_y_micro: QuaternionComponentMicro,
    /// 스폰 자세 z.
    pub orientation_z_micro: QuaternionComponentMicro,
    /// 스폰 자세 w.
    pub orientation_w_micro: QuaternionComponentMicro,
}

/// `SHIP_DESPAWNED` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipDespawnedPayload {
    /// 사라진 함선. `SHIP_SPAWNED` 와 같은 `ship_id` 로 짝짓는다(correlation 아님, I-41).
    pub ship_id: UuidV7,
    /// 이 함선을 마지막으로 몰았던 세션.
    pub last_session_id: UuidV7,
    /// 사라진 이유.
    pub despawn_reason: DespawnReason,
    /// 마지막 위치 X.
    pub position_x_mm: PositionMm,
    /// 마지막 위치 Y.
    pub position_y_mm: PositionMm,
    /// 마지막 위치 Z.
    pub position_z_mm: PositionMm,
}

/// `SHIP_SPAWNED` — 함선 엔티티가 존재를 시작했다.
///
/// 대응 스키마: `contracts/events/domain/SHIP_SPAWNED.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipSpawnedEvent {
    /// 저장·전달 멱등 키.
    pub event_id: UuidV7,
    /// 언제나 `SHIP_SPAWNED`.
    pub event_type: ShipSpawnedType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 발행한 tick.
    pub tick: Tick,
    /// 그 tick 안의 발행 순서.
    pub sequence: Sequence,
    /// 게임 시간.
    pub occurred_at: GameTime,
    /// 실제 시각. 감사 전용.
    pub recorded_at: RealTime,
    /// 이 스폰을 일으킨 세션의 correlation.
    pub correlation_id: UuidV7,
    /// **좁혀졌다 — 비-null**. 언제나 같은 tick의 `SESSION_OPENED.event_id`.
    pub causation_id: UuidV7,
    /// **좁혀졌다 — 비-null**. 이 함선이 속한 행위자.
    pub actor_id: UuidV7,
    /// 타입별 payload.
    pub payload: ShipSpawnedPayload,
}

/// `SHIP_DESPAWNED` — 함선 엔티티가 존재를 마쳤다.
///
/// 대응 스키마: `contracts/events/domain/SHIP_DESPAWNED.schema.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipDespawnedEvent {
    /// 저장·전달 멱등 키.
    pub event_id: UuidV7,
    /// 언제나 `SHIP_DESPAWNED`.
    pub event_type: ShipDespawnedType,
    /// 언제나 1.
    pub schema_version: ConstSchemaVersion<1>,
    /// 월드(샤드) id.
    pub world_id: UuidV7,
    /// 발행한 tick.
    pub tick: Tick,
    /// 그 tick 안의 발행 순서.
    pub sequence: Sequence,
    /// 게임 시간.
    pub occurred_at: GameTime,
    /// 실제 시각. 감사 전용.
    pub recorded_at: RealTime,
    /// 이 잔류를 시작시킨 세션의 correlation.
    pub correlation_id: UuidV7,
    /// **좁혀졌다 — 비-null**. 그 잔류를 시작시킨 `SESSION_CLOSED.event_id`(여러 tick 전일 수
    /// 있다 — 상관과 인과가 다른 시각을 가리킨다, HSE §102).
    pub causation_id: UuidV7,
    /// **좁혀졌다 — 비-null**. 이 함선이 속했던 행위자.
    pub actor_id: UuidV7,
    /// 타입별 payload.
    pub payload: ShipDespawnedPayload,
}
