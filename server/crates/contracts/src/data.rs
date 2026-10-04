//! `data/` 테이블 3종의 모양 — `contracts/data/*.schema.json` 에 대응한다.
//!
//! # 이 파일의 타입은 서버 내부 게임 규칙에 쓰이지 않는다
//!
//! 여기 있는 것은 **계약 모양**뿐이다(범위 안이면 유효한 데이터, 범위 밖이면 깨진
//! 시뮬레이션). "무엇이 재미있는가"는 designer 의 `data/` 값이 결정하고, 이 타입은
//! 그 값이 "이 모양을 벗어나지 않았는가"만 검사한다(스펙 §5.5).
//!
//! `tick_hz/snapshot_hz` 정수 나눗셈, `point_count == len(points_m)`, 스폰 지점이 하드
//! 경계 안, `main_thrust ≥ lateral/reverse`, `resume ≤ linger` 같은 **필드 간 유도값
//! 검산**은 여기 없다 — 그것은 `bins/game-server/src/data.rs`(S2)의 몫이다(rule=derived).
//! 여기서 하는 것은 필드 하나하나의 범위(rule=schema)뿐이다.
//!
//! # 데이터 테이블은 SI 실수다 (ADR-0009 §2의 명시적 예외)
//!
//! 와이어와 달리 정수 양자화가 없다. 왕복 비교는 `serde_json::Value::as_f64()` 정규화 후
//! 정확 비교로 한다(AC-9(a), U-20) — 이 파일의 타입은 그 비교의 대상일 뿐 비교 방식을
//! 정하지 않는다.

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

use crate::historical::HistoricalVisibility;
use crate::primitives::{DataId, MassKg, RuleVersion, SchemaVersion, de_positive_mass_kg};

// ---------------------------------------------------------------------------
// 범위 검증 f64 역직렬화 — 필드마다 스키마의 min/max 그대로
// ---------------------------------------------------------------------------

/// `$fn` 이름으로 `f64` 범위 검증 `deserialize_with` 함수를 만든다.
///
/// 구조체 필드는 평범한 `f64` 로 남는다 — 직렬화는 그대로 두고 **역직렬화만** 스키마의
/// `minimum`/`maximum`/`exclusiveMinimum`/`exclusiveMaximum` 을 강제한다. 값 하나가 범위를
/// 벗어나면 "깨진 시뮬레이션"이라는 뜻이므로(이 파일 모듈 문서), 그 값을 들고 있는
/// 것보다 역직렬화 시점에 거부하는 것이 맞다.
macro_rules! de_f64_range {
    ($fn_name:ident, min = $min:expr, min_excl = $min_excl:expr, max = $max:expr) => {
        #[allow(clippy::redundant_closure_call)]
        fn $fn_name<'de, D>(deserializer: D) -> Result<f64, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw = f64::deserialize(deserializer)?;
            let min_ok = if $min_excl { raw > $min } else { raw >= $min };
            let max_ok = raw <= $max;
            if min_ok && max_ok && raw.is_finite() {
                Ok(raw)
            } else {
                Err(D::Error::custom(format!(
                    "{} 는 {}{} .. {} 범위를 벗어난다 (받음: {raw})",
                    stringify!($fn_name),
                    if $min_excl { "(" } else { "[" },
                    $min,
                    $max
                )))
            }
        }
    };
}

de_f64_range!(
    de_accel_mps2,
    min = 0.0,
    min_excl = false,
    max = 1_000_000.0
);
de_f64_range!(
    de_max_speed_mps,
    min = 0.0,
    min_excl = true,
    max = 100_000.0
);
de_f64_range!(de_rate_deg_s, min = 0.0, min_excl = true, max = 3600.0);
de_f64_range!(de_accel_deg_s2, min = 0.0, min_excl = true, max = 36_000.0);
de_f64_range!(de_deadzone_sin, min = 0.0, min_excl = false, max = 0.5);
de_f64_range!(de_f64_0_3600, min = 0.0, min_excl = false, max = 3600.0);
de_f64_range!(
    de_positive_upto_100000,
    min = 0.0,
    min_excl = true,
    max = 100_000.0
);
de_f64_range!(
    de_boundary_radius_m,
    min = 0.0,
    min_excl = true,
    max = 20_000.0
);
de_f64_range!(
    de_spawn_radius_m,
    min = 0.0,
    min_excl = false,
    max = 20_000.0
);
de_f64_range!(
    de_remote_interp_delay_ms,
    min = 0.0,
    min_excl = false,
    max = 2000.0
);
de_f64_range!(
    de_remote_extrapolate_max_ms,
    min = 0.0,
    min_excl = false,
    max = 5000.0
);
de_f64_range!(
    de_reconcile_ignore_threshold_m,
    min = 0.0,
    min_excl = false,
    max = 1.0
);
de_f64_range!(
    de_reconcile_smooth_threshold_m,
    min = 0.0,
    min_excl = true,
    max = 1000.0
);
de_f64_range!(
    de_reconcile_smooth_duration_ms,
    min = 0.0,
    min_excl = true,
    max = 5000.0
);
de_f64_range!(
    de_orientation_ignore_threshold_deg,
    min = 0.0,
    min_excl = false,
    max = 10.0
);
de_f64_range!(
    de_orientation_deg_0_180,
    min = 0.0,
    min_excl = true,
    max = 180.0
);

/// `Option<f64>` 버전 — 값이 있으면 검증하고, `null`/부재면 `None`.
macro_rules! de_f64_range_opt {
    ($fn_name:ident, $inner:ident) => {
        #[allow(dead_code)]
        fn $fn_name<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(Deserialize)]
            #[serde(transparent)]
            struct Wrapper(#[serde(deserialize_with = "de_wrapped")] f64);
            fn de_wrapped<'de2, D2: Deserializer<'de2>>(d: D2) -> Result<f64, D2::Error> {
                $inner(d)
            }
            Option::<Wrapper>::deserialize(deserializer).map(|w| w.map(|Wrapper(v)| v))
        }
    };
}
de_f64_range_opt!(de_opt_spawn_radius_m, de_spawn_radius_m);
de_f64_range_opt!(de_opt_f64_0_3600, de_f64_0_3600);

// ---------------------------------------------------------------------------
// 정수 범위 역직렬화
// ---------------------------------------------------------------------------

macro_rules! de_i64_range {
    ($fn_name:ident, $min:expr, $max:expr) => {
        fn $fn_name<'de, D>(deserializer: D) -> Result<i64, D::Error>
        where
            D: Deserializer<'de>,
        {
            let raw = i64::deserialize(deserializer)?;
            if ($min..=$max).contains(&raw) {
                Ok(raw)
            } else {
                Err(D::Error::custom(format!(
                    "{} 는 {} ..= {} 범위여야 한다 (받음: {raw})",
                    stringify!($fn_name),
                    $min,
                    $max
                )))
            }
        }
    };
}

de_i64_range!(de_point_count, 1i64, 64i64);
de_i64_range!(de_max_probe_attempts, 1i64, 1024i64);
de_i64_range!(de_presence_seconds_i, 0i64, 3600i64);
de_i64_range!(de_hz_1_60, 1i64, 60i64);
de_i64_range!(de_count_1_64, 1i64, 64i64);
de_i64_range!(de_client_send_hz, 1i64, 240i64);
de_i64_range!(de_carry_forward_max_ticks, 0i64, 200i64);
de_i64_range!(de_rate_limit_hz, 1i64, 1000i64);
de_i64_range!(de_protocol_violation_hz, 1i64, 10_000i64);

fn de_opt_hz_1_60<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Wrapper(#[serde(deserialize_with = "de_hz_1_60")] i64);
    Option::<Wrapper>::deserialize(deserializer).map(|w| w.map(|Wrapper(v)| v))
}

/// `[x, y, z]` 3성분 벡터.
type Triplet = [f64; 3];

/// `points_m` — `1 ..= 64` 개, 각각 3성분.
fn de_points_m<'de, D>(deserializer: D) -> Result<Vec<Triplet>, D::Error>
where
    D: Deserializer<'de>,
{
    let points = Vec::<Triplet>::deserialize(deserializer)?;
    if points.is_empty() || points.len() > 64 {
        Err(D::Error::custom(format!(
            "points_m 은 1 ..= 64 개여야 한다 (받음: {})",
            points.len()
        )))
    } else {
        Ok(points)
    }
}

// ---------------------------------------------------------------------------
// SHIP_CLASS
// ---------------------------------------------------------------------------

/// 함선 클래스의 비행 모델. 현재 하나뿐이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlightModel {
    /// 비행 보조 6DoF. 두 보조 감쇠를 0으로, `flight_assist` 를 false 로 두면 순수 뉴턴이다.
    #[serde(rename = "assisted_6dof")]
    Assisted6Dof,
}

/// `movement` 블록 — ADR-0010 §2 적분기의 입력 전부.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipClassMovement {
    /// 비행 모델.
    pub model: FlightModel,
    /// 속도 상한.
    #[serde(deserialize_with = "de_max_speed_mps")]
    pub max_speed_mps: f64,
    /// 전방 최대 가속. 대각선 클램프의 상한이기도 하다.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub main_thrust_mps2: f64,
    /// 후진 최대 가속.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub reverse_thrust_mps2: f64,
    /// 측면 최대 가속.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub lateral_thrust_mps2: f64,
    /// 브레이크 감쇠.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub brake_decel_mps2: f64,
    /// 무추력 보조 감쇠(관성이 사는 자리).
    #[serde(deserialize_with = "de_accel_mps2")]
    pub assist_linear_decel_mps2: f64,
    /// 추력 방향에 수직인 성분의 보조 감쇠.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub assist_lateral_decel_mps2: f64,
    /// 자세 슬루 상한.
    #[serde(deserialize_with = "de_rate_deg_s")]
    pub turn_rate_max_deg_s: f64,
    /// 각속도 변화 상한.
    #[serde(deserialize_with = "de_accel_deg_s2")]
    pub turn_accel_deg_s2: f64,
    /// 자세 오차 → 목표 각속도 비례 게인(`sin(θ/2)` 당).
    #[serde(deserialize_with = "de_accel_deg_s2")]
    pub turn_gain_deg_s_per_sin_half: f64,
    /// 이 오차 아래면 목표 각속도가 0이다.
    #[serde(deserialize_with = "de_deadzone_sin")]
    pub turn_deadzone_sin_half: f64,
    /// 수동 롤 상한.
    #[serde(deserialize_with = "de_rate_deg_s")]
    pub roll_rate_max_deg_s: f64,
    /// 롤 각속도 변화 상한.
    #[serde(deserialize_with = "de_accel_deg_s2")]
    pub roll_accel_deg_s2: f64,
    /// 오토레벨 상한.
    #[serde(deserialize_with = "de_f64_0_3600")]
    pub auto_level_rate_deg_s: f64,
    /// 오토레벨 불감대.
    #[serde(deserialize_with = "de_deadzone_sin")]
    pub auto_level_deadzone_sin: f64,
}

/// `derived_for_review` — 코드가 읽지 않는다. QA 대조 전용.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DerivedForReview {
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_to_max_speed_s: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zero_to_max_speed_distance_m: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brake_from_max_speed_s: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brake_from_max_speed_distance_m: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coast_to_rest_from_max_speed_s: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coast_to_rest_distance_m: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flip_180_deg_s: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_delta_per_tick_at_max_speed_m: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_delta_per_tick_at_max_rate_deg: Option<f64>,
}

/// `geometry` 블록.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipClassGeometry {
    /// 충돌은 없다(p1-01) — 스폰 여유·최근접 함선 지표에만 쓴다.
    #[serde(deserialize_with = "de_positive_upto_100000")]
    pub hull_radius_m: f64,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `reference_stats_unused_in_p1_01` — 코드가 읽지 않는다. GDD 원문 스탯 보관용.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ParkedStats {
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crew: Option<i64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_capacity_t: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon_hardpoints: Option<i64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp_range_ly: Option<f64>,
    /// 〃
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shield: Option<f64>,
}

/// `SHIP_CLASS` — 함선 클래스 파일 하나(예: `data/ships/scout-s01.json`)의 모양.
///
/// 대응 스키마: `contracts/data/ship-class.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShipClassTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 id. `SHIP_SPAWNED.ship_class_id` 에 그대로 들어간다(principle 5).
    pub id: DataId,
    /// 표시 이름.
    pub display_name: String,
    /// 선체 분류.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hull_class: Option<String>,
    /// GDD 출처 문구.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gdd_source: Option<String>,
    /// designer 메모.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 적분기 입력.
    pub movement: ShipClassMovement,
    /// QA 대조용(코드가 읽지 않는다).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_for_review: Option<DerivedForReview>,
    /// 스폰 여유·지표용 형상.
    pub geometry: ShipClassGeometry,
    /// GDD 원문 스탯(코드가 읽지 않는다).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_stats_unused_in_p1_01: Option<ParkedStats>,
}

// ---------------------------------------------------------------------------
// STAR_SYSTEM
// ---------------------------------------------------------------------------

/// `coordinate_space.frame`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateFrame {
    /// 유일한 값.
    #[serde(rename = "system_local")]
    SystemLocal,
}

/// `coordinate_space.unit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinateUnit {
    /// 유일한 값.
    #[serde(rename = "meter")]
    Meter,
}

/// `coordinate_space.server_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerScalarType {
    /// 유일한 값.
    #[serde(rename = "f64")]
    F64,
}

/// `coordinate_space` 블록. **선언적일 뿐이다** — 서버는 여기서 손 방향을 읽지 않는다.
/// 손 방향은 ADR-0009 §1 이 고정한다(왼손, Y-up, +Z 전방).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateSpace {
    /// 좌표계 범위.
    pub frame: CoordinateFrame,
    /// 단위.
    pub unit: CoordinateUnit,
    /// 서버 내부 스칼라 타입.
    pub server_type: ServerScalarType,
    /// 원점.
    pub origin: Triplet,
    /// 위 방향.
    pub up_axis: Triplet,
    /// 자유 문구. designer 가 좌표계 손 방향 정정 등을 남긴다.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `play_area` 블록.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayArea {
    /// soft 경계.
    #[serde(deserialize_with = "de_boundary_radius_m")]
    pub soft_boundary_radius_m: f64,
    /// hard 경계. 상한 20 km 는 float32 예산(ADR-0009 §3), 설계 선택이 아니다.
    #[serde(deserialize_with = "de_boundary_radius_m")]
    pub hard_boundary_radius_m: f64,
    /// soft 경계 밖에서 더해지는 가속.
    #[serde(deserialize_with = "de_accel_mps2")]
    pub boundary_pull_mps2: f64,
    /// 문서용 — 코드가 읽지 않는다(스키마도 상한을 두지 않는다).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_float32_ulp_at_hard_boundary_m: Option<f64>,
    /// 〃
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_float32_ulp_visible_jitter_threshold_m: Option<f64>,
    /// 〃
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floating_origin_required_above_radius_m: Option<f64>,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `spawn.plane`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnPlane {
    /// 유일한 값.
    #[serde(rename = "ecliptic")]
    Ecliptic,
}

/// `spawn.initial_facing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitialFacing {
    /// 유일한 값.
    #[serde(rename = "system_origin")]
    SystemOrigin,
}

/// `spawn` 블록 — 결정적 스폰(ADR-0010 §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    /// 문서용 스폰 링 반경.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "de_opt_spawn_radius_m")]
    pub ring_radius_m: Option<f64>,
    /// 스폰 평면.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<SpawnPlane>,
    /// `points_m.len()` 과 일치해야 한다(I-38 — 유도값 검산, S2).
    #[serde(deserialize_with = "de_point_count")]
    pub point_count: i64,
    /// 문서용 Y 교대 간격.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y_alternation_m: Option<f64>,
    /// 스폰 지점 점유 판정 여유.
    #[serde(deserialize_with = "de_spawn_radius_m")]
    pub clearance_m: f64,
    /// 점유 시 다음 지점을 찾는 최대 시도 수.
    #[serde(deserialize_with = "de_max_probe_attempts")]
    pub max_probe_attempts: i64,
    /// 전부 점유일 때 반경 바깥으로 미는 간격.
    #[serde(deserialize_with = "de_spawn_radius_m")]
    pub radial_offset_step_m: f64,
    /// 스폰 초기 속도.
    pub initial_velocity_mps: Triplet,
    /// 초기 자세 규칙.
    pub initial_facing: InitialFacing,
    /// 서버가 구현하는 규칙의 산문 서술. 파싱하지 않는다.
    pub assignment_rule: String,
    /// 스폰 후보 지점. `1 ..= 64` 개, 전부 하드 경계 안이어야 한다(S2 검산).
    #[serde(deserialize_with = "de_points_m")]
    pub points_m: Vec<Triplet>,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `presence` 블록 — 잔류·재개(ADR-0011 §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
    /// 조종사 없는 함선이 세계에 남는 시간.
    #[serde(deserialize_with = "de_presence_seconds_i")]
    pub linger_seconds: i64,
    /// 같은 `actor_id` 가 재접속해 함선을 되찾을 수 있는 창. `≤ linger_seconds` (S2 검산).
    #[serde(deserialize_with = "de_presence_seconds_i")]
    pub reconnect_resume_window_seconds: i64,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `reference_markers[]` 의 `kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReferenceMarkerKind {
    /// 항성.
    Star,
    /// 행성.
    Planet,
    /// 표지.
    Beacon,
    /// 잔해.
    Derelict,
}

/// **클라이언트 전용 정적 시각물.** 서버 상태도 스냅샷도 아니다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceMarker {
    /// 안정적 id.
    pub id: DataId,
    /// 표시 이름.
    pub display_name: String,
    /// 종류.
    pub kind: ReferenceMarkerKind,
    /// 위치.
    pub position_m: Triplet,
    /// 시각 반경.
    #[serde(deserialize_with = "de_boundary_radius_m")]
    pub visual_radius_m: f64,
}

/// `STAR_SYSTEM` — 성계 파일 하나(예: `data/world/systems/cradle.json`)의 모양.
///
/// 대응 스키마: `contracts/data/star-system.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StarSystemTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 content id. 영속화된 `world_id` UUID 와 다르다.
    pub id: DataId,
    /// 표시 이름.
    pub display_name: String,
    /// designer 메모.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 좌표계 선언(문서용).
    pub coordinate_space: CoordinateSpace,
    /// 경계.
    pub play_area: PlayArea,
    /// 스폰 규칙과 후보 지점.
    pub spawn: Spawn,
    /// 잔류·재개 창.
    pub presence: Presence,
    /// 클라이언트 전용 기준물. 비어 있어도 빈 배열로 왕복한다(원본에 키가 있으면
    /// `skip_serializing_if` 로 지우지 않는다 — AC-9(a) 왕복은 값 동일이지 키 생략이 아니다).
    #[serde(default)]
    pub reference_markers: Vec<ReferenceMarker>,
    /// 기준물 메모.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_markers_note: Option<String>,
}

// ---------------------------------------------------------------------------
// SYNC_TUNING
// ---------------------------------------------------------------------------

/// `snapshot` 블록.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncTuningSnapshot {
    /// 초당 전체 스냅샷 수. `tick_hz` 를 나누어떨어뜨려야 한다(S2 검산).
    #[serde(deserialize_with = "de_hz_1_60")]
    pub snapshot_hz: i64,
    /// 스냅샷당 최대 엔티티 수.
    #[serde(deserialize_with = "de_count_1_64")]
    pub max_entities_per_snapshot: i64,
    /// 세션당 스냅샷 스트림 예산.
    #[serde(deserialize_with = "de_positive_upto_100000")]
    pub egress_budget_kib_s_per_session: f64,
    /// 예산 초과 시 폴백 주기.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "de_opt_hz_1_60")]
    pub fallback_snapshot_hz_if_over_budget: Option<i64>,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `input` 블록.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncTuningInput {
    /// 클라이언트가 입력을 보내는 주기. tick 주기와 같다.
    #[serde(deserialize_with = "de_client_send_hz")]
    pub client_send_hz: i64,
    /// 입력 0건일 때 직전 입력을 이월하는 최대 tick 수.
    #[serde(deserialize_with = "de_carry_forward_max_ticks")]
    pub carry_forward_max_ticks: i64,
    /// 지속 초과 시 `RATE_LIMITED` 로 거부하는 기준.
    #[serde(deserialize_with = "de_rate_limit_hz")]
    pub rate_limit_hz: i64,
    /// 프로토콜 위반 예산이 소모되기 시작하는 비율.
    #[serde(deserialize_with = "de_protocol_violation_hz")]
    pub protocol_violation_hz: i64,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `prediction` 블록 — 클라이언트 보간·재조정 임계값.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncTuningPrediction {
    /// 타 함선 보간 지연.
    #[serde(deserialize_with = "de_remote_interp_delay_ms")]
    pub remote_interp_delay_ms: f64,
    /// 외삽 상한.
    #[serde(deserialize_with = "de_remote_extrapolate_max_ms")]
    pub remote_extrapolate_max_ms: f64,
    /// 이 오차 아래는 렌더 오프셋을 건드리지 않는다.
    #[serde(deserialize_with = "de_reconcile_ignore_threshold_m")]
    pub reconcile_ignore_threshold_m: f64,
    /// 이 오차까지는 부드럽게 수렴.
    #[serde(deserialize_with = "de_reconcile_smooth_threshold_m")]
    pub reconcile_smooth_threshold_m: f64,
    /// 수렴 시간.
    #[serde(deserialize_with = "de_reconcile_smooth_duration_ms")]
    pub reconcile_smooth_duration_ms: f64,
    /// 이 오차를 넘으면 하드 스냅.
    #[serde(deserialize_with = "de_positive_upto_100000")]
    pub reconcile_hard_snap_threshold_m: f64,
    /// 자세 무시 밴드.
    #[serde(deserialize_with = "de_orientation_ignore_threshold_deg")]
    pub reconcile_orientation_ignore_threshold_deg: f64,
    /// 자세 부드러운 수렴 임계.
    #[serde(deserialize_with = "de_orientation_deg_0_180")]
    pub reconcile_orientation_smooth_threshold_deg: f64,
    /// 자세 하드 스냅 임계.
    #[serde(deserialize_with = "de_orientation_deg_0_180")]
    pub reconcile_orientation_hard_snap_deg: f64,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `SYNC_TUNING` — `data/movement/sync-tuning.json` 의 모양.
///
/// 대응 스키마: `contracts/data/sync-tuning.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncTuningTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 id.
    pub id: DataId,
    /// designer 메모.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 스냅샷 송신.
    pub snapshot: SyncTuningSnapshot,
    /// 입력 페이싱.
    pub input: SyncTuningInput,
    /// 예측·보정.
    pub prediction: SyncTuningPrediction,
}

// ---------------------------------------------------------------------------
// MINERAL (p1-02)
// ---------------------------------------------------------------------------

/// 희귀도. 클라이언트가 **표시에만** 쓴다 — 역사 판정 규칙의 입력이 아니다(ADR-0014 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MineralRarity {
    /// 흔함.
    Common,
    /// 드묾.
    Uncommon,
    /// 희귀.
    Rare,
}

/// `1 ..= 86400` 초.
fn de_regen_interval_s<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = i64::deserialize(deserializer)?;
    if (1..=86_400).contains(&raw) {
        Ok(raw)
    } else {
        Err(D::Error::custom(format!(
            "regen_interval_s 는 1 ..= 86400 이어야 한다 (받음: {raw})"
        )))
    }
}

/// `extraction` 블록.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralExtraction {
    /// 채굴 1회당 얻는 양. 절대 0이 아니다.
    #[serde(deserialize_with = "de_positive_mass_kg")]
    pub yield_per_extraction_kg: MassKg,
}

/// `regeneration` 블록.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralRegeneration {
    /// 회복 간격마다 늘어나는 양. 절대 0이 아니다.
    #[serde(deserialize_with = "de_positive_mass_kg")]
    pub regen_kg: MassKg,
    /// 회복 간격(초). `tick_hz` 를 곱해 정수 tick 이 되어야 한다(S2 기동 검산).
    #[serde(deserialize_with = "de_regen_interval_s")]
    pub regen_interval_s: i64,
}

/// `MINERAL` — 광물 파일 하나(`data/minerals/{id}.json`)의 모양.
///
/// **수치는 designer 소유다.** GDD §7.1 속성 중 아무 코드도 읽지 않는 것은 의도적으로
/// 빠졌다(설계 문서 §2.4).
///
/// 대응 스키마: `contracts/data/mineral.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MineralTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 id. `MINERAL_MINED.mineral_id` 등에 그대로 들어간다.
    pub id: DataId,
    /// 표시 이름.
    pub display_name: String,
    /// 표시 전용 희귀도.
    pub rarity: MineralRarity,
    /// designer 메모.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 채굴 산출.
    pub extraction: MineralExtraction,
    /// 회복 규칙.
    pub regeneration: MineralRegeneration,
}

// ---------------------------------------------------------------------------
// DEPOSIT_FIELD (p1-02)
// ---------------------------------------------------------------------------

de_f64_range!(
    de_deposit_radius_m,
    min = 0.0,
    min_excl = true,
    max = 1_000.0
);

/// `position_m` — 3성분, 각 성분 `-20000 ..= 20000` m.
fn de_deposit_position_m<'de, D>(deserializer: D) -> Result<Triplet, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Triplet::deserialize(deserializer)?;
    for component in raw {
        if !(-20_000.0..=20_000.0).contains(&component) || !component.is_finite() {
            return Err(D::Error::custom(format!(
                "position_m 성분은 -20000 ..= 20000 이어야 한다 (받음: {component})"
            )));
        }
    }
    Ok(raw)
}

/// 매장지 하나.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deposit {
    /// 안정적 id.
    pub id: DataId,
    /// **지역** 이름일 뿐이다 — 광물을 드러내면 안 된다(스키마 설명).
    pub display_name: String,
    /// **서버 전용 진실**(설계 문서 요구): 클라이언트 사본이 있어도 이 필드는 읽지 않는다
    /// — 광물은 `DEPOSIT_FIELD_STATE` 로 드러난 뒤에만 클라이언트에 닿는다.
    pub mineral_id: DataId,
    /// 성계 로컬 좌표(ADR-0009 §1).
    #[serde(deserialize_with = "de_deposit_position_m")]
    pub position_m: Triplet,
    /// 채굴 판정용 반지름.
    #[serde(deserialize_with = "de_deposit_radius_m")]
    pub radius_m: f64,
    /// **서버 전용 진실**: 원 매장량. `mineral_id` 처럼 드러나기 전에는 클라이언트에
    /// 닿지 않는다.
    #[serde(deserialize_with = "de_positive_mass_kg")]
    pub initial_reserve_kg: MassKg,
}

/// `1 ..= 64` 개.
fn de_deposits<'de, D>(deserializer: D) -> Result<Vec<Deposit>, D::Error>
where
    D: Deserializer<'de>,
{
    let deposits = Vec::<Deposit>::deserialize(deserializer)?;
    if deposits.is_empty() || deposits.len() > 64 {
        Err(D::Error::custom(format!(
            "deposits 는 1 ..= 64 개여야 한다 (받음: {})",
            deposits.len()
        )))
    } else {
        Ok(deposits)
    }
}

/// `DEPOSIT_FIELD` — 한 성계의 매장지 파일(`data/world/deposits/{star_system_id}.json`)의
/// 모양.
///
/// 이 스키마가 표현하지 못하는 기동 시 검산(S2 몫, 스펙 p1-02 I-38 확장): 모든
/// `mineral_id` 가 `data/minerals` 에 존재 / 모든 광물이 매장지 ≥1 / `star_system_id` 가
/// 로딩된 성계와 일치 / `|position| + radius_m + mining_range_from_surface_m ≤
/// soft_boundary_radius_m` / `regen_kg ≤ initial_reserve_kg` / id 고유 / 두 매장지의 채굴
/// 영역이 겹치지 않음.
///
/// 대응 스키마: `contracts/data/deposit-field.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepositFieldTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 이 파일이 속한 성계.
    pub star_system_id: DataId,
    /// designer 메모.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 매장지 목록.
    #[serde(deserialize_with = "de_deposits")]
    pub deposits: Vec<Deposit>,
}

// ---------------------------------------------------------------------------
// MINING_RULES (p1-02)
// ---------------------------------------------------------------------------

de_f64_range!(
    de_mining_range_from_surface_m,
    min = 0.0,
    min_excl = true,
    max = 5_000.0
);
de_f64_range!(
    de_max_ship_speed_mps,
    min = 0.0,
    min_excl = false,
    max = 1_000.0
);

/// `1 ..= 3600` 초.
fn de_cooldown_s<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = i64::deserialize(deserializer)?;
    if (1..=3600).contains(&raw) {
        Ok(raw)
    } else {
        Err(D::Error::custom(format!(
            "cooldown_s 는 1 ..= 3600 이어야 한다 (받음: {raw})"
        )))
    }
}

/// `extraction` 블록 — 채굴 판정 세 수치(스펙 p1-02 §4.2). 사거리: `|ship - deposit|^2 <=
/// (radius_m + mining_range_from_surface_m)^2`. 속도: `|velocity|^2 <=
/// max_ship_speed_mps^2`. 둘 다 명령을 처리하는 tick **시작 시점**의 권위 `f64` 상태로 잰다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MiningExtractionRules {
    /// 매장지 표면에서부터의 채굴 사거리.
    #[serde(deserialize_with = "de_mining_range_from_surface_m")]
    pub mining_range_from_surface_m: f64,
    /// 채굴 허용 최대 함선 속력.
    #[serde(deserialize_with = "de_max_ship_speed_mps")]
    pub max_ship_speed_mps: f64,
    /// 채굴 쿨다운(초). `tick_hz` 를 곱해 정수 tick 이 되어야 한다(기동 검산).
    #[serde(deserialize_with = "de_cooldown_s")]
    pub cooldown_s: i64,
    /// 자유 문구.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `MINING_RULES` — `data/mining/mining-rules.json` 의 모양. 모든 채굴 판정이 참조하는
/// 유일한 수치 출처.
///
/// 대응 스키마: `contracts/data/mining-rules.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MiningRulesTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 id.
    pub id: DataId,
    /// designer 메모.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 판정 세 수치.
    pub extraction: MiningExtractionRules,
}

// ---------------------------------------------------------------------------
// SIGNIFICANCE_RULE (p1-02)
// ---------------------------------------------------------------------------

/// 레지스트리 `TypeName` 패턴(`^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$`)을 손으로 검사한다. 이
/// 파일에서만 쓰여서(`produces_event_type`) 전용 primitive 타입을 만들지 않았다 —
/// `event_type`/`message_type`/`command_type` 예약어와 겹치지 않는 필드 이름이라는 점이
/// 스키마 설명의 요지이지, 재사용 폭이 아니다.
fn de_type_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    let mut chars = raw.bytes();
    let valid = match chars.next() {
        Some(head) if head.is_ascii_uppercase() => {
            chars.all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && !raw.contains("__")
                && !raw.ends_with('_')
        }
        _ => false,
    };
    if valid {
        Ok(raw)
    } else {
        Err(D::Error::custom(format!(
            "produces_event_type 은 SCREAMING_SNAKE_CASE 여야 한다 (받음: {raw})"
        )))
    }
}

/// HSE 증거 종류. p1-02는 `SHIP_LOG` 하나뿐(닫힌 집합이지만 규칙이 늘면 커진다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceType {
    /// 함선 항법 일지 — 자동 생성 증거.
    ShipLog,
}

/// `evidence` 블록 — 판정 통과 시 자동으로 만드는 증거의 종류·가시성.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignificanceRuleEvidence {
    /// 증거 종류.
    pub evidence_type: EvidenceType,
    /// 증거 자체의 가시성(기록 가시성과 다를 수 있다).
    pub visibility: HistoricalVisibility,
}

/// `1 ..= 5`.
fn de_rule_importance_level<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = u8::deserialize(deserializer)?;
    if (1..=5).contains(&raw) {
        Ok(raw)
    } else {
        Err(D::Error::custom(format!(
            "importance_level 은 1 ..= 5 여야 한다 (받음: {raw})"
        )))
    }
}

/// `SIGNIFICANCE_RULE` — 역사 판정 규칙 파일 하나(`data/history/rules/{rule_id}.json`)의
/// 모양. **규칙 하나당 파일 하나** — 규칙을 추가해도 기존 규칙 파일의 해시가 바뀌지 않는다
/// (ADR-0014 §5). 판정 **조건**은 Rust 코드다(HSE §33) — 이 파일은 규칙의 정체성과 산출
/// 상수만 싣는다. `rarity` 같은 다른 데이터 표의 입력을 실어서는 안 된다.
///
/// 대응 스키마: `contracts/data/significance-rule.schema.json`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignificanceRuleTable {
    /// 데이터 파일 스키마 버전.
    pub schema_version: SchemaVersion,
    /// 안정적 규칙 id.
    pub rule_id: DataId,
    /// 규칙 버전. 기동 검산: `rule_id + "@"` 로 시작해야 한다.
    pub rule_version: RuleVersion,
    /// 이 규칙이 만드는 역사 이벤트 타입.
    #[serde(deserialize_with = "de_type_name")]
    pub produces_event_type: String,
    /// HSE §11 중요도.
    #[serde(deserialize_with = "de_rule_importance_level")]
    pub importance_level: u8,
    /// 이 규칙이 만드는 기록의 가시성.
    pub visibility: HistoricalVisibility,
    /// designer 메모.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designer_note: Option<String>,
    /// 자동 생성 증거.
    pub evidence: SignificanceRuleEvidence,
}

// ---------------------------------------------------------------------------
// 계약 테스트 대응표용 왕복 — envelope 이 없는 kind: "data" 타입은
// registry.rs 의 `round_trip_as::<T>` 를 그대로 쓴다(다른 kind 와 동일한 함수).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ship_class_round_trips_example_scout() {
        let raw = include_str!("../../../../contracts/fixtures/SHIP_CLASS/example-scout.json");
        let table: ShipClassTable = serde_json::from_str(raw).expect("파싱 실패");
        assert_eq!(table.id.as_str(), "scout-s01");
        assert!((table.movement.max_speed_mps - 140.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ship_class_rejects_zero_turn_gain() {
        let raw =
            include_str!("../../../../contracts/fixtures/SHIP_CLASS/invalid/turn-gain-zero.json");
        assert!(serde_json::from_str::<ShipClassTable>(raw).is_err());
    }

    #[test]
    fn ship_class_rejects_unknown_movement_field() {
        let raw = include_str!(
            "../../../../contracts/fixtures/SHIP_CLASS/invalid/movement-unknown-field.json"
        );
        assert!(serde_json::from_str::<ShipClassTable>(raw).is_err());
    }

    #[test]
    fn star_system_rejects_hard_radius_above_ceiling() {
        let raw = include_str!(
            "../../../../contracts/fixtures/STAR_SYSTEM/invalid/hard-radius-above-ceiling.json"
        );
        assert!(serde_json::from_str::<StarSystemTable>(raw).is_err());
    }

    #[test]
    fn star_system_rejects_empty_spawn_points() {
        let raw =
            include_str!("../../../../contracts/fixtures/STAR_SYSTEM/invalid/no-spawn-points.json");
        assert!(serde_json::from_str::<StarSystemTable>(raw).is_err());
    }

    #[test]
    fn sync_tuning_rejects_zero_snapshot_hz() {
        let raw = include_str!(
            "../../../../contracts/fixtures/SYNC_TUNING/invalid/snapshot-hz-zero.json"
        );
        assert!(serde_json::from_str::<SyncTuningTable>(raw).is_err());
    }

    #[test]
    fn sync_tuning_rejects_integrator_field_in_prediction() {
        let raw = include_str!(
            "../../../../contracts/fixtures/SYNC_TUNING/invalid/integrator-is-not-a-tunable.json"
        );
        assert!(serde_json::from_str::<SyncTuningTable>(raw).is_err());
    }

    #[test]
    fn star_system_round_trips_real_cradle_data() {
        let raw = include_str!("../../../../data/world/systems/cradle.json");
        let table: StarSystemTable = serde_json::from_str(raw).expect("실제 data/ 파싱 실패");
        assert_eq!(table.spawn.points_m.len(), 12);
    }

    #[test]
    fn sync_tuning_round_trips_real_data() {
        let raw = include_str!("../../../../data/movement/sync-tuning.json");
        let table: SyncTuningTable = serde_json::from_str(raw).expect("실제 data/ 파싱 실패");
        assert_eq!(table.snapshot.snapshot_hz, 10);
    }

    #[test]
    fn ship_class_round_trips_real_scout_data() {
        let raw = include_str!("../../../../data/ships/scout-s01.json");
        let table: ShipClassTable = serde_json::from_str(raw).expect("실제 data/ 파싱 실패");
        assert_eq!(table.id.as_str(), "scout-s01");
    }
}
