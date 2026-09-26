//! 함선의 물리 상태 — ADR-0010 §2 적분기가 읽고 쓰는 값 전부.

use super::quat::Quat;
use super::vec3::Vec3;

/// 함선 한 척의 물리 상태. `f64` — 양자화는 스냅샷을 만들 때 한 번만 일어난다(ADR-0009 §2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipPhysicsState {
    /// 위치(m).
    pub p: Vec3,
    /// 속도(m/s).
    pub v: Vec3,
    /// 자세.
    pub q: Quat,
    /// 자세 제어기의 월드 프레임 각속도(deg/s) — 롤을 포함하지 않는다(ADR-0010 §1.1).
    pub omega_aim: Vec3,
    /// 롤 각속도(deg/s) — 전방축 스칼라.
    pub omega_roll: f64,
}

impl ShipPhysicsState {
    /// 정지 상태(스폰 직후) — 속도 0, 각속도 0.
    #[must_use]
    pub const fn at_rest(p: Vec3, q: Quat) -> Self {
        Self {
            p,
            v: Vec3::ZERO,
            q,
            omega_aim: Vec3::ZERO,
            omega_roll: 0.0,
        }
    }
}
