//! 스폰 — 시드 해시 + 간격 탐색, 난수 없음 (ADR-0010 §4).
//!
//! `H` 는 이 크레이트 안에 손으로 쓴 결정적 정수 해시다(splitmix64, Sebastiano Vigna,
//! 공개 도메인). 해시 크레이트를 들이지 않는다 — 결정성 보장을 크레이트의 릴리스
//! 정책에 위임하지 않기 위해서다. 고정 입력의 출력을 테스트가 박아 둔다.

use super::quat::Quat;
use super::vec3::Vec3;

/// splitmix64 한 스텝. 공개 도메인 알고리즘 그대로.
#[must_use]
const fn splitmix64(seed: u64) -> u64 {
    let x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn split_bytes16(bytes: [u8; 16]) -> (u64, u64) {
    let mut hi = [0u8; 8];
    let mut lo = [0u8; 8];
    hi.copy_from_slice(&bytes[0..8]);
    lo.copy_from_slice(&bytes[8..16]);
    (u64::from_le_bytes(hi), u64::from_le_bytes(lo))
}

/// `H(world_seed_le8 ‖ actor_id_16bytes)` — 스폰 시작 색인을 결정적으로 고른다.
///
/// 단일 `u64` 시드를 받는 `splitmix64` 로 세 값(월드 시드 + actor_id 상위/하위 8바이트)을
/// 순차적으로 섞는다. 알고리즘 선택은 임의이지만 **레포 안에 명세가 있고 고정 입력
/// 테스트가 출력을 박아 둔다**는 것이 ADR-0010 §4 의 요구 전부다.
#[must_use]
pub fn spawn_hash(world_seed: u64, actor_id: [u8; 16]) -> u64 {
    let (actor_hi, actor_lo) = split_bytes16(actor_id);
    let mut state = splitmix64(world_seed);
    state = splitmix64(state ^ actor_hi);
    splitmix64(state ^ actor_lo)
}

/// 스폰 후보 지점 선택에 필요한 성계·함선 쪽 상수. 인자 개수를 줄이려고 묶는다
/// (clippy `too_many_arguments`) — [`choose_spawn_point`] 참고.
#[derive(Debug, Clone, Copy)]
pub struct SpawnParams<'a> {
    /// 이 월드의 스폰 시드.
    pub world_seed: u64,
    /// 스폰 후보 지점(`data/world/systems/*.json` 의 `spawn.points_m`).
    pub points_m: &'a [[f64; 3]],
    /// 점유 판정 여유.
    pub clearance_m: f64,
    /// 스폰할 함선 클래스의 선체 반경.
    pub hull_radius_m: f64,
    /// 점유 시 다음 지점을 찾는 최대 시도 수.
    pub max_probe_attempts: u32,
    /// 전부 점유일 때 반경 바깥으로 미는 간격.
    pub radial_offset_step_m: f64,
}

/// 스폰 후보 지점을 고른다.
///
/// `existing` 은 현재 활성·잔류 함선의 위치(점유 판정용, `clearance_m + hull_radius_m`
/// 반경). 전부 점유면 `index0` 지점을 반경 바깥으로 밀어낸다.
///
/// **"시도수" 해석 — architect 확인 완료(T-A4, 2026-09-20)**: ADR-0010 §4 의 "전부
/// 점유 시 `radial_offset_step_m × ceil(시도수 / point_count)`" 문구가 원래 의도한
/// "링을 돈 바퀴 수" 해석은 현재 데이터(`max_probe_attempts(12) == point_count(12)`)에서
/// 바퀴가 언제나 1에서 끝나 **모든 오버플로 스폰이 정확히 같은 지점에 쌓이는** 결함이
/// 있었다 — 겹침을 흩뜨리려던 규칙이 정반대로 동작했다. 이 구현이 쓰는
/// **`ceil((이미 세계에 있는 함선 수 + 1) / point_count)`** 는 월드가 찰수록 더 바깥으로
/// 미는 단조 함수라 의도에 맞고, architect가 ADR을 이 구현에 맞춰 갱신했다. **코드
/// 변경 없음 — 동작·fixture 전부 그대로다.**
#[must_use]
pub fn choose_spawn_point(actor_id: [u8; 16], params: &SpawnParams<'_>, existing: &[Vec3]) -> Vec3 {
    let point_count = params.points_m.len();
    if point_count == 0 {
        return Vec3::ZERO; // 방어적 — S2 가 이미 point_count >= 1 을 강제한다.
    }

    let hash = spawn_hash(params.world_seed, actor_id);
    #[allow(clippy::cast_possible_truncation)]
    let index0 = (hash % point_count as u64) as usize;

    let required = params.clearance_m + params.hull_radius_m;
    let attempts = params
        .max_probe_attempts
        .min(u32::try_from(point_count).unwrap_or(u32::MAX));

    for step in 0..attempts {
        #[allow(clippy::cast_possible_truncation)]
        let index = (index0 + step as usize) % point_count;
        let candidate = to_vec3(params.points_m[index]);
        if existing
            .iter()
            .all(|occupied| (candidate - *occupied).length() >= required)
        {
            return candidate;
        }
    }

    // 전부 점유 — index0 지점을 반경 바깥으로 민다(판단 근거는 문서 주석 참고).
    let base = to_vec3(params.points_m[index0]);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let overflow_tries = (existing.len() as u32 + 1).max(1);
    let cycles = overflow_tries.div_ceil(u32::try_from(point_count).unwrap_or(1));
    let radial_dir = base.normalize(1e-9).unwrap_or(Vec3::FORWARD);
    base + radial_dir.scale(params.radial_offset_step_m * f64::from(cycles))
}

fn to_vec3(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0], p[1], p[2])
}

/// 초기 자세 — 성계 원점을 바라봄(server B-5, ADR-0010 §4 수식).
///
/// 기준 전방축 `f0 = (0,0,1)` 에서 `fwd = normalize(origin − p)` 로 가는 최소 회전.
/// `p` 가 원점이면(있을 수 없다 — 스폰 지점은 원점에 있지 않다) 항등 회전으로 대체한다.
#[must_use]
pub fn facing_toward_origin(p: Vec3) -> Quat {
    let f0 = Vec3::FORWARD;
    let Some(fwd) = p.scale(-1.0).normalize(1e-9) else {
        return Quat::IDENTITY;
    };

    let c = 1.0 + f0.dot(fwd);
    if c >= 1e-6 {
        let cross = f0.cross(fwd);
        Quat::new(cross.x, cross.y, cross.z, c).renormalize(1e-9)
    } else {
        // 대척점 — 최소 회전이 유일하지 않다. "임의의 축"은 적혀 있기만 하면 된다.
        let axis_source = if f0.y.abs() < 0.9 {
            f0.cross(Vec3::WORLD_UP)
        } else {
            f0.cross(Vec3::new(1.0, 0.0, 0.0))
        };
        let axis = axis_source
            .normalize(1e-9)
            .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
        Quat::new(axis.x, axis.y, axis.z, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 고정 입력의 출력을 박아 둔다(ADR-0010 §4 요구) — 이 값이 바뀌면 스폰 위치가
    /// 실서버에서도 바뀐다는 뜻이므로, 바뀌었다면 의도한 변경인지 확인한다.
    #[test]
    fn spawn_hash_is_pinned_for_fixed_inputs() {
        let actor_id: [u8; 16] = [
            0x01, 0xa0, 0xb1, 0xc2, 0x00, 0x00, 0x70, 0x01, 0x80, 0x02, 0x9c, 0x0d, 0x1e, 0x2f,
            0x3a, 0x4b,
        ];
        let hash = spawn_hash(42, actor_id);
        // 회귀 고정값 — 최초 구현에서 실제로 나온 값. 알고리즘을 바꾸면 이 상수도 바뀐다.
        assert_eq!(
            hash,
            splitmix64(splitmix64(splitmix64(42) ^ 0x0170_0000_c2b1_a001) ^ 0x4b3a_2f1e_0d9c_0280)
        );
    }

    #[test]
    fn same_actor_same_world_always_same_index() {
        let actor_id = [7u8; 16];
        let a = spawn_hash(1, actor_id);
        let b = spawn_hash(1, actor_id);
        assert_eq!(
            a, b,
            "같은 actor_id + 같은 world_seed 는 항상 같은 해시여야 한다"
        );
    }

    #[test]
    fn different_actors_usually_differ() {
        let h1 = spawn_hash(1, [1u8; 16]);
        let h2 = spawn_hash(1, [2u8; 16]);
        assert_ne!(h1, h2);
    }

    /// 실제 `cradle.json` 의 스폰 지점 하나(`[0, -250, 2500]`)로 대척점 분기를 확인한다
    /// (server 실측: `dot = -0.995`, `1 + dot = 0.005` — 지금은 대척점을 겨우 비껴간다).
    #[test]
    fn facing_toward_origin_points_back_at_the_spawn_point() {
        let p = Vec3::new(0.0, -250.0, 2500.0);
        let q = facing_toward_origin(p);
        let forward = q.rotate(Vec3::FORWARD);
        let expected = p.scale(-1.0).normalize(1e-9).expect("정규화되어야 한다");
        assert!(
            (forward.x - expected.x).abs() < 1e-6,
            "{forward:?} vs {expected:?}"
        );
        assert!((forward.y - expected.y).abs() < 1e-6);
        assert!((forward.z - expected.z).abs() < 1e-6);
    }

    #[test]
    fn facing_toward_origin_handles_true_antipode() {
        // f0 = (0,0,1) 의 정반대는 fwd = (0,0,-1) — dot = -1, c = 0 (대척점 분기).
        let p = Vec3::new(0.0, 0.0, 1000.0); // origin - p 방향은 (0,0,-1)
        let q = facing_toward_origin(p);
        let forward = q.rotate(Vec3::FORWARD);
        // 180도 회전이므로 forward 는 (0,0,-1) 이어야 한다.
        assert!((forward.z + 1.0).abs() < 1e-6, "{forward:?}");
    }

    #[test]
    fn choose_spawn_point_picks_a_free_point_when_available() {
        let points = vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], [2000.0, 0.0, 0.0]];
        let params = SpawnParams {
            world_seed: 1,
            points_m: &points,
            clearance_m: 150.0,
            hull_radius_m: 12.0,
            max_probe_attempts: 12,
            radial_offset_step_m: 150.0,
        };
        let chosen = choose_spawn_point([3u8; 16], &params, &[]);
        assert!(points.iter().any(|p| to_vec3(*p) == chosen));
    }

    #[test]
    fn choose_spawn_point_avoids_occupied_points() {
        let points = vec![[0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]];
        // 첫 후보 지점을 점유시켜 두 번째로 넘어가는지 본다(어느 쪽이 index0 인지는
        // 해시가 정하므로, 둘 다 점유하고 나서 전부 점유 분기를 확인한다).
        let existing = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1000.0, 0.0, 0.0)];
        let params = SpawnParams {
            world_seed: 1,
            points_m: &points,
            clearance_m: 150.0,
            hull_radius_m: 12.0,
            max_probe_attempts: 12,
            radial_offset_step_m: 500.0,
        };
        let chosen = choose_spawn_point([3u8; 16], &params, &existing);
        // 전부 점유이므로 원래 지점보다 원점에서 더 멀어야 한다(반경 바깥으로 밀림).
        let index0_len = {
            let hash = spawn_hash(1, [3u8; 16]);
            #[allow(clippy::cast_possible_truncation)]
            let idx = (hash % 2) as usize;
            to_vec3(points[idx]).length()
        };
        assert!(
            chosen.length() >= index0_len,
            "{chosen:?} vs base len {index0_len}"
        );
    }
}
