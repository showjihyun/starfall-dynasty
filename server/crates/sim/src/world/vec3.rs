//! `f64` 3벡터. 성계 로컬 좌표(ADR-0009 §1) — 왼손, Y-up, +Z 전방.
//!
//! **초월함수를 쓰지 않는다**(ADR-0010 §3). `sqrt` 만 쓴다 — IEEE-754 가 정확히 규정한다.

/// 3성분 실수 벡터.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// +X = 함선 기준 오른쪽.
    pub x: f64,
    /// +Y = 위.
    pub y: f64,
    /// +Z = 전방(뱃머리).
    pub z: f64,
}

impl Vec3 {
    /// 영벡터.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// 세계 위(world up) — 오토레벨 기준(ADR-0010 §2 5단계).
    pub const WORLD_UP: Self = Self {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    /// 함선 로컬 전방(뱃머리) 축.
    pub const FORWARD: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    };

    /// 만든다.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// 스칼라 곱.
    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    /// 내적.
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// 외적.
    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    /// 길이 — `sqrt(x²+y²+z²)`.
    #[must_use]
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// 정규화. 길이가 `min_len` 미만이면 `None`(호출자가 퇴화를 처리한다).
    #[must_use]
    pub fn normalize(self, min_len: f64) -> Option<Self> {
        let len = self.length();
        if len < min_len {
            None
        } else {
            Some(self.scale(1.0 / len))
        }
    }
}

// `Add`/`Sub` 표준 트레이트로 구현한다 — 같은 이름의 고유 메서드는 clippy
// `should_implement_trait` 가 "표준 트레이트와 혼동될 수 있다"로 거부한다. 호출부의
// `.add(rhs)`/`.sub(rhs)` 문법은 트레이트 메서드로도 그대로 동작해 바뀌는 곳이 없다.
impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_product_is_right_handed_algebraically() {
        // 외적 자체는 좌표계 손 방향과 무관한 순수 대수 연산이다 — 손 방향이 새는 지점은
        // ADR-0010 §2.1 이 지적한 5단계의 부호 하나뿐이다.
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = x.cross(y);
        assert_eq!(z, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn length_and_normalize_round_trip() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert!((v.length() - 5.0).abs() < 1e-12);
        let n = v.normalize(1e-9).expect("정규화되어야 한다");
        assert!((n.length() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_rejects_below_threshold() {
        assert!(Vec3::new(1e-10, 0.0, 0.0).normalize(1e-9).is_none());
    }
}
