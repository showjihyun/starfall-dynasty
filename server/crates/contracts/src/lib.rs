//! `contracts/` 에 대응하는 serde 타입.
//!
//! # 단일 진실은 `contracts/` 다 (I-1)
//!
//! 와이어 데이터의 모양은 `contracts/` 의 JSON Schema 에만 정의된다. 이 크레이트의 타입은
//! 그 모양을 Rust 에서 **따라 쓴 것**이고, 계약을 바꿀 권한이 없다. 모양이 바뀌어야 하면
//! architect 에게 요청한다 (스키마·레지스트리·fixture 를 함께 고쳐야 한다).
//!
//! # 왜 생성하지 않고 손으로 쓰는가 (ADR-0002 §3)
//!
//! `typify` 는 정수 `minimum`/`maximum` 을 타입으로 강제하지 못한다. 우리 계약은
//! `MoneyMinor`·`Tick`·`probe_seq` 처럼 **범위가 곧 의미**인 정수가 핵심이라, 범위를 못 지키는
//! 생성기는 우리 문제를 풀지 못한다. `schemars` 는 반대로 계약의 진실을 Rust 코드로 옮겨
//! 클라이언트를 서버 구현에 종속시킨다. 대신 드리프트는 레지스트리 주도 계약 테스트 9종이 잡는다
//! (`tests/contract_tests.rs`).
//!
//! # 드리프트를 막는 것은 테스트다
//!
//! ```text
//! cd server && cargo test -p starfall-contracts --locked
//! ```
//!
//! 새 타입을 추가하면 [`registry::CONTRACT_TYPES`] 에 한 줄만 넣으면 되고, 테스트가
//! 왕복·반례 거부·`required` 변이·정수 상한을 자동으로 덮는다.

pub mod commands;
pub mod dispatch;
pub mod events;
pub mod messages;
pub mod primitives;
pub mod registry;

pub use commands::{PingServerCommand, PingServerPayload, PingServerType};
pub use events::{
    SessionCloseReason, SessionClosedEvent, SessionClosedPayload, SessionClosedType,
    SessionOpenedEvent, SessionOpenedPayload, SessionOpenedType, SessionTransport,
};
pub use messages::{
    CommandResultMessage, CommandResultPayload, CommandResultType, CommandStatus, PingReplyMessage,
    PingReplyPayload, PingReplyType, RejectReasonCode, SessionReadyMessage, SessionReadyPayload,
    SessionReadyType,
};
pub use primitives::{
    ConstSchemaVersion, GameCalendar, GameTime, MAX_SAFE_INTEGER, ProbeSeq, RealTime, Sequence,
    ServerVersion, Tick, TickHz, UuidV7,
};
pub use registry::{
    COMMAND_RESULT, CONTRACT_TYPES, ContractType, PING_REPLY, PING_SERVER, SESSION_CLOSED,
    SESSION_OPENED, SESSION_READY,
};
