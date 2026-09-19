//! 고정 tick 시뮬레이션 코어 (ADR-0006).
//!
//! # 이 크레이트의 한 가지 약속
//!
//! **상태는 [`Simulation::step`] 안에서만 바뀐다** (I-13). WebSocket 수신 태스크는 여기에
//! [`Submission`] 을 넣을 뿐 상태 타입에 접근하지 못한다 — 크레이트 경계가 그것을 강제한다.
//!
//! # IO 도 시계도 난수도 없다
//!
//! `Cargo.toml` 의 의존성이 하나(`starfall-contracts`)뿐인 것이 그 증거다 (AC-1 / SC-04).
//!
//! **이 크레이트에 들어오면 안 되는 것과 이유** (금지 목록의 정본. `Cargo.toml` 주석에는
//! 이름을 적지 않는다 — QA 가 그 파일을 이름으로 grep 하므로 주석이 검사를 무의미하게
//! 만든다):
//!
//! | 금지 | 왜 |
//! |------|----|
//! | `axum` / `sqlx` / `redis` | IO 가 결정적 코어에 들어왔다는 뜻이다 |
//! | `rand` | 같은 입력이 같은 결과를 내지 않게 된다 (원칙 9). 이번 슬라이스에 난수는 없다 |
//! | `chrono` | tick 안에서 시계를 읽을 길이 생긴다. 달력 산술은 `starfall-contracts` 의 의존성 없는 정수 연산(`GameCalendar`)으로 한다 |
//! | `tokio` | tick 루프는 전용 OS 스레드에서 돌고(ADR-0006 §2.1) 채널은 게이트웨이가 소유한다. 그래서 AC-6(a) 의 수동 step 테스트가 **런타임도 소켓도 없이** 돈다 |
//!
//! - **시계를 읽지 않는다.** `occurred_at` 은 `tick` 에서만 파생하고(I-19), `recorded_at` 은
//!   영속화 단계가 채운다. 그래서 [`PendingEvent`] 에는 `recorded_at` 이 **없다**.
//! - **id 를 스스로 만들지 않는다.** UUIDv7 은 시계를 읽으므로 [`IdSource`] 로 주입받는다
//!   (ADR-0002 §5). 테스트는 결정적 생성기를 넣어 같은 입력에 같은 결과를 얻는다.
//! - **채널을 모른다.** [`Simulation::step`] 은 제출 목록을 받아 [`TickOutcome`] 을 돌려주는
//!   함수다. 보낼 메시지를 어디로 보낼지는 게이트웨이가 정한다 (ADR-0006 §4).
//!
//! # tick 루프는 여기 없다
//!
//! 루프(전용 OS 스레드 + `std::thread::sleep`)는 게이트웨이의 `runtime` 모듈에 있다.
//! 이 크레이트가 루프를 가지면 `sleep` 과 `Instant` 가 결정적 코어로 들어온다.

mod session;
mod simulation;

pub use session::{DEDUP_CAPACITY, SessionSnapshot};
pub use simulation::{
    DomainEventBody, IdSource, InboundCommand, Outbound, PendingEvent, PersistBatch, ServerMessage,
    Simulation, Submission, TickOutcome, WorldConstants,
};
