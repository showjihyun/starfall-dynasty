//! 쿼터니언 `(x, y, z, w)`, Hamilton 곱 (ADR-0009 §1, ADR-0010 §2).
//!
//! 초월함수를 쓰지 않는다(ADR-0010 §3). `sqrt` 만 쓴다.

use super::vec3::Vec3;

/// 성분 순서 `(x, y, z, w)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    /// x.
    pub x: f64,
    /// y.
    pub y: f64,
    /// z.
    pub z: f64,
    /// 스칼라부.
    pub w: f64,
}

impl Quat {
    /// 항등 회전.
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// 만든다.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        Self { x, y, z, w }
    }

    /// 순수 쿼터니언(스칼라부 0) — `q_pure(w)`. 각속도를 Hamilton 곱에 쓸 때.
    #[must_use]
    pub const fn pure(v: Vec3) -> Self {
        Self::new(v.x, v.y, v.z, 0.0)
    }

    /// 벡터부(x, y, z) — `vec(q)`.
    #[must_use]
    pub const fn vec(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// 켤레 — `conj(q)`.
    #[must_use]
    pub const fn conj(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }

    /// 전체 부호 반전. `q_err.w < 0` 일 때 최단 경로를 고르는 데 쓴다(ADR-0010 §2 2단계).
    #[must_use]
    pub const fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }

    /// 스칼라 곱.
    #[must_use]
    pub fn scale(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s, self.w * s)
    }

    /// 노름 — `sqrt(x²+y²+z²+w²)`.
    #[must_use]
    pub fn norm(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    /// 재정규화. 노름이 `min_norm` 미만이면 항등 회전으로 대체한다(4·5단계 적분 뒤에만
    /// 쓴다 — 그 경로에서는 노름이 1 근처이므로 사실상 항상 정상 분기를 탄다).
    #[must_use]
    pub fn renormalize(self, min_norm: f64) -> Self {
        let n = self.norm();
        if n < min_norm {
            Self::IDENTITY
        } else {
            self.scale(1.0 / n)
        }
    }

    /// 회전 적용 — `rot(q, v) = vec(q ⊗ (v,0) ⊗ conj(q))`.
    #[must_use]
    pub fn rotate(self, v: Vec3) -> Vec3 {
        (self * Self::pure(v) * self.conj()).vec()
    }
}

// `Add`/`Mul` 표준 트레이트로 구현한다(vec3.rs 와 같은 이유 — clippy
// `should_implement_trait`). `Mul` 이 Hamilton 곱이다: **곱 순서가 의미를 가진다** —
// 교환 법칙이 없다. `self * other` 는 `self ⊗ other` 다.
impl std::ops::Add for Quat {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(
            self.x + other.x,
            self.y + other.y,
            self.z + other.z,
            self.w + other.w,
        )
    }
}

impl std::ops::Mul for Quat {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        Self::new(
            self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
            self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Mul;

    use super::*;

    #[test]
    fn identity_rotation_is_a_no_op() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let rotated = Quat::IDENTITY.rotate(v);
        assert!((rotated.x - v.x).abs() < 1e-12);
        assert!((rotated.y - v.y).abs() < 1e-12);
        assert!((rotated.z - v.z).abs() < 1e-12);
    }

    #[test]
    fn ninety_degree_yaw_rotates_forward_to_right() {
        // 월드 +Y 축 둘레 +90도 회전: 반각 45도에서 sin=cos=sqrt(1/2) — 이 특정 각도에서만
        // 성립하는 항등식이라 `sqrt` 만으로 구하고 삼각함수를 쓰지 않는다(SC-03 은 테스트
        // 코드도 포함해 `crates/sim/src` 전체를 grep 한다).
        let half_sqrt2 = 0.5_f64.sqrt();
        let q = Quat::new(0.0, half_sqrt2, 0.0, half_sqrt2);
        let forward = Vec3::new(0.0, 0.0, 1.0);
        let rotated = q.rotate(forward);
        // 왼손 좌표계에서 Y축 둘레 +90도는 +Z 를 +X 로 보낸다.
        assert!((rotated.x - 1.0).abs() < 1e-9, "{rotated:?}");
        assert!(rotated.y.abs() < 1e-9);
        assert!(rotated.z.abs() < 1e-9);
    }

    #[test]
    fn hamilton_product_matches_hand_computation() {
        // i * j = k (표준 쿼터니언 대수).
        let i = Quat::new(1.0, 0.0, 0.0, 0.0);
        let j = Quat::new(0.0, 1.0, 0.0, 0.0);
        let k = i.mul(j);
        assert_eq!(k, Quat::new(0.0, 0.0, 1.0, 0.0));
    }

    #[test]
    fn renormalize_falls_back_to_identity_when_degenerate() {
        let degenerate = Quat::new(0.0, 0.0, 0.0, 0.0);
        assert_eq!(degenerate.renormalize(1e-9), Quat::IDENTITY);
    }
}
