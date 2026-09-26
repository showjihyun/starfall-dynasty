//! 월드 상태 모델 — 함선 물리(ADR-0010), 좌표·양자화(ADR-0009), 결정적 스폰(ADR-0010 §4).
//!
//! 이 모듈 트리에 IO·시계·난수가 없다는 것은 `sim` 크레이트 전체의 약속이다
//! (`src/lib.rs` 모듈 문서 참고). 여기 있는 것은 순수 함수와 값 타입뿐이다.

pub mod integrate;
pub mod quantise;
pub mod quat;
pub mod ship;
pub mod spawn;
pub mod vec3;

pub use integrate::{BoundaryConstants, ControlInput, IntegrateOutcome, ShipClassConstants};
pub use quat::Quat;
pub use ship::ShipPhysicsState;
pub use spawn::{SpawnParams, choose_spawn_point, facing_toward_origin, spawn_hash};
pub use vec3::Vec3;
