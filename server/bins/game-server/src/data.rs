//! `data/` 3종 로딩과 계약 검증 — 기동 시 한 번, 실패하면 기동을 거부한다 (I-38).
//!
//! # 두 계층
//!
//! - **rule=schema**: 필드 하나하나의 범위. `starfall_contracts::data::*` 의 Rust 타입이
//!   역직렬화 시점에 강제한다 — "운영 중 명령을 막는 것은 스키마 검증기가 아니라 serde"
//!   (ADR-0002 §3)를 데이터 테이블에도 그대로 적용한다.
//! - **rule=derived**: 필드 사이의 유도값 검산. 이 모듈이 한다(스펙 I-38):
//!   `tick_hz / snapshot_hz` 가 정수, `point_count == len(points_m)`, `soft ≤ hard`,
//!   스폰 지점 전부가 하드 경계 안, `reconnect_resume_window_seconds ≤ linger_seconds`,
//!   `main_thrust_mps2 ≥ lateral/reverse_thrust_mps2`, 함선 클래스 `id` 중복 없음.
//!   (`max_entities_per_snapshot ≤ 64` 는 이미 스키마 층에서 막힌다 —
//!   `SyncTuningSnapshot::max_entities_per_snapshot` 의 `deserialize_with` 범위가 `1..=64`다.)
//!
//! # p1-02 확장 (S2, 스펙 §4.7) — 광물·광맥·채굴 규칙·역사 규칙
//!
//! 4종을 더 읽는다: `data/minerals/*.json`(광물마다 파일 1개), `data/world/deposits/*.json`
//! (성계마다 파일 1개, 이 슬라이스는 `cradle.json` 하나), `data/mining/mining-rules.json`
//! (유일), `data/history/rules/*.json`(규칙마다 파일 1개). 유도값 검산(§4.7, 12가지):
//! 광맥 `mineral_id` 가 광물 표에 존재 / 모든 광물이 광맥 ≥1 / 광맥 파일의
//! `star_system_id` 가 로딩된 성계와 일치 / `|position| + radius_m +
//! mining_range_from_surface_m ≤ soft_boundary_radius_m` / `regen_kg ≤
//! initial_reserve_kg` / 광물·광맥 `id` 중복 없음 / 두 광맥의 채굴 구역이 겹치지 않음
//! (`거리 > 두 반지름 합 + 2 × 사거리`) / 역사 규칙의 `rule_version` 이 `rule_id + "@"` 로
//! 시작 / `produces_event_type` 이 레지스트리의 `historical_event` 타입.
//!
//! **`cooldown_s × tick_hz`·`regen_interval_s × tick_hz` "정수" 검산은 구조적으로 항상
//! 참이다(judgement call, 실측).** 스키마가 `cooldown_s`·`regen_interval_s` 를 이미
//! `integer`(초)로 강제하고(`contracts/data/{mining-rules,mineral}.schema.json`) `tick_hz`
//! 도 `u32` 이므로, 유효한 JSON 으로는 그 곱을 비정수로 만들 방법이 없다 — 두 정수의 곱은
//! 언제나 정수다. 스프린트 계약의 §4.7 표(경우 ⑤·⑥)가 요구하는 "비정수 거부" 반례를 만들 수
//! 없다는 뜻이라 이 모듈은 **곱을 그대로 유도**만 하고(`cooldown_ticks`·
//! `regen_interval_ticks`), "정수가 아니면 거부"하는 별도 분기를 두지 않았다 — 그런 분기는
//! 도달 불가능한 코드가 된다. team-lead·qa 에게 알린다(`03_server_impl.md` S2 절).
//!
//! # `STARFALL_DATA_DIR` 해석 (architect 결정, `02_server_ack.md` §1① + 중단 후 확정분)
//!
//! - 설정돼 있으면 **그 경로만** 본다(탐색 없음) — QA 가 위반 데이터를 임시 디렉토리로
//!   시연할 때 원본 `data/` 와 섞이지 않게 하는 장치다.
//! - 없으면 프로세스 작업 디렉토리 기준 `data` → `../data` 순으로 **처음 존재하는** 디렉토리.
//! - 어느 경로로 갔든 **해석된 절대 경로**를 로그와 `/debug/stats` 에 남긴다.
//! - 못 찾으면 시도한 절대 경로를 전부 로그에 남기고 기동을 거부한다(종료 코드 1).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use starfall_contracts::data::{
    Deposit, DepositFieldTable, MineralTable, MiningRulesTable, ShipClassTable,
    SignificanceRuleTable, StarSystemTable, SyncTuningTable,
};

/// 데이터 로딩·검산 실패. 전부 기동 거부로 이어진다.
///
/// `Display` 가 그대로 로그 한 줄이 되도록 형식을 맞춘다(스프린트 계약 SC-06):
/// `기동 거부: 데이터 검증 실패 file=<경로> ... rule=<schema|derived>`.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// `STARFALL_DATA_DIR` 를 해석하지 못했다.
    #[error("기동 거부: data/ 디렉토리를 찾지 못했다 — 시도한 경로: {tried}")]
    DataDirNotFound {
        /// 시도한 절대 경로들(쉼표 구분).
        tried: String,
    },
    /// 디렉토리를 열지 못했다.
    #[error("기동 거부: {dir} 을(를) 열지 못했다: {source}")]
    ReadDir {
        /// 디렉토리.
        dir: PathBuf,
        /// IO 오류.
        source: std::io::Error,
    },
    /// 파일을 읽지 못했다.
    #[error("기동 거부: {file} 을(를) 읽지 못했다: {source}")]
    ReadFile {
        /// 파일.
        file: PathBuf,
        /// IO 오류.
        source: std::io::Error,
    },
    /// 스키마 층(필드 범위) 위반 — Rust 타입의 역직렬화가 거부했다.
    #[error("기동 거부: 데이터 검증 실패 file={file} error={error} rule=schema")]
    Schema {
        /// 파일.
        file: PathBuf,
        /// `<JSON 경로>: <serde 오류 메시지>` 형태(`serde_path_to_error` 가 경로를 낸다,
        /// S-R25). 예: `play_area.hard_boundary_radius_m: de_boundary_radius_m 는
        /// [0 .. 20000 범위를 벗어난다 (받음: 20000.1)`. 경로가 공유 `deserialize_with`
        /// 함수 이름과 달리 **필드를 특정한다** — `soft_boundary_radius_m` 과
        /// `hard_boundary_radius_m` 처럼 같은 검증자를 쓰는 필드도 서로 다른 경로로
        /// 구분된다. 메시지 뒷부분(함수 이름 포함)은 여전히 serde 가 만든 문자열 그대로다.
        error: String,
    },
    /// 유도값(필드 간) 검산 위반.
    #[error(
        "기동 거부: 데이터 검증 실패 file={file} field={field} expected={expected} actual={actual} rule=derived"
    )]
    Derived {
        /// 파일.
        file: PathBuf,
        /// 위반한 필드(점 경로).
        field: String,
        /// 기대한 제약.
        expected: String,
        /// 실제 값.
        actual: String,
    },
    /// `ships/` 에 파일이 하나도 없다.
    #[error("기동 거부: {dir} 에 함선 클래스 파일이 없다")]
    NoShipClasses {
        /// 디렉토리.
        dir: PathBuf,
    },
    /// `world/systems/` 에 파일이 하나도 없다.
    #[error("기동 거부: {dir} 에 성계 파일이 없다")]
    NoStarSystems {
        /// 디렉토리.
        dir: PathBuf,
    },
    /// `world/systems/` 에 파일이 여러 개다 — 이 슬라이스는 한 성계만 지원한다(스펙 §1).
    #[error("기동 거부: {dir} 에 성계 파일이 여러 개다(이 슬라이스는 한 성계만 지원한다): {files}")]
    AmbiguousStarSystem {
        /// 디렉토리.
        dir: PathBuf,
        /// 발견한 파일 이름들(쉼표 구분).
        files: String,
    },
    /// `minerals/` 에 파일이 하나도 없다.
    #[error("기동 거부: {dir} 에 광물 파일이 없다")]
    NoMinerals {
        /// 디렉토리.
        dir: PathBuf,
    },
    /// `world/deposits/` 에 파일이 하나도 없다.
    #[error("기동 거부: {dir} 에 광맥 파일이 없다")]
    NoDeposits {
        /// 디렉토리.
        dir: PathBuf,
    },
    /// `history/rules/` 에 파일이 하나도 없다.
    #[error("기동 거부: {dir} 에 역사 규칙 파일이 없다")]
    NoSignificanceRules {
        /// 디렉토리.
        dir: PathBuf,
    },
}

/// 기동 시 로드된 게임 데이터. 프로세스 수명 동안 불변이다.
#[derive(Debug, Clone)]
pub struct GameData {
    /// 해석된 `data/` 절대 경로. `/debug/stats` 가 그대로 보여준다.
    pub data_dir: PathBuf,
    /// `id` → 행. `data/` 는 파일 이름이 아니라 이 `id` 로 색인한다(ADR-0009 §2).
    pub ship_classes: BTreeMap<String, ShipClassTable>,
    /// 이 슬라이스의 유일한 성계.
    pub star_system: StarSystemTable,
    /// 네트워킹 튜닝 값.
    pub sync_tuning: SyncTuningTable,
    /// `tick_hz / snapshot_hz` — 서버가 기동 시 유도한다(ADR-0011 §2).
    pub snapshot_interval_ticks: u32,
    /// `id` → 행(p1-02, `data/minerals/*.json`).
    pub minerals: BTreeMap<String, MineralTable>,
    /// `id` → 행(p1-02, `data/world/deposits/*.json` — 이 슬라이스가 로딩한 성계의
    /// 것만, `star_system_id` 는 §4.7 검산에서 이미 대조됐다).
    pub deposits: BTreeMap<String, Deposit>,
    /// 채굴 판정 세 수치(p1-02, `data/mining/mining-rules.json`).
    pub mining_rules: MiningRulesTable,
    /// `cooldown_s * tick_hz` 유도(모듈 문서 참고 — 스키마가 이미 정수를 강제해 이 곱은
    /// 언제나 정수다).
    pub mining_cooldown_ticks: u64,
    /// `rule_id` → 행(p1-02, `data/history/rules/*.json`). 현재 시뮬레이션은 이 표를
    /// 소비하지 않는다(history 크레이트가 자신의 판정 코어에서 따로 읽는다, H1) — S2는
    /// 기동 검산(§4.7)과 `/debug/stats` 의 `rule_version` 노출만 책임진다.
    pub significance_rules: BTreeMap<String, SignificanceRuleTable>,
    /// SC-07 — 실제로 읽은 데이터 파일 10개, `data_dir` 기준 상대 경로(posix
    /// 구분자)로, 읽은 순서 그대로. 기동 로그의 `데이터 파일 적재 data_file=...` 줄과
    /// 같은 목록에서 만든다 — 단위 테스트는 이 값을 재지 로그를 캡처하지 않는다(전역
    /// tracing 상태에 기대는 캡처는 병렬 테스트에서 flaky 했다, 팀 리더 방침
    /// 2026-09-29). `main.rs` 의 기동 요약 로그(`loaded_files` 필드)가 이 값을 쓴다.
    pub loaded_files: Vec<String>,
}

impl GameData {
    /// 스폰 후보 지점 수 (`/debug/stats` 의 "스폰 지점 수" — SC-05).
    #[must_use]
    pub fn spawn_point_count(&self) -> usize {
        self.star_system.spawn.points_m.len()
    }

    /// 이 슬라이스의 유일한 역사 규칙의 `rule_version`(`/debug/stats`, SC-04). 규칙이 여럿인
    /// 슬라이스가 오면 이 accessor 도 바뀌어야 한다 — 지금은 정확히 1개임을 `load()`가
    /// 보장하지 않으므로 `None` 도 가능(방어적).
    #[must_use]
    pub fn sole_rule_version(&self) -> Option<&str> {
        if self.significance_rules.len() == 1 {
            self.significance_rules
                .values()
                .next()
                .map(|rule| rule.rule_version.as_str())
        } else {
            None
        }
    }
}

/// `STARFALL_DATA_DIR` 를 해석한다. `override_path` 가 `Some` 이면 그 경로만 본다.
///
/// # Errors
///
/// 해석된 경로가 디렉토리가 아니면 실패한다.
pub fn resolve_data_dir(cwd: &Path, override_path: Option<&Path>) -> Result<PathBuf, DataError> {
    if let Some(path) = override_path {
        let abs = to_abs(cwd, path);
        return if abs.is_dir() {
            Ok(abs)
        } else {
            Err(DataError::DataDirNotFound {
                tried: abs.display().to_string(),
            })
        };
    }

    let mut tried = Vec::new();
    for candidate in ["data", "../data"] {
        let abs = to_abs(cwd, Path::new(candidate));
        if abs.is_dir() {
            return Ok(abs);
        }
        tried.push(abs.display().to_string());
    }
    Err(DataError::DataDirNotFound {
        tried: tried.join(", "),
    })
}

fn to_abs(cwd: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    fs::canonicalize(&joined).unwrap_or(joined)
}

/// 디렉토리 안의 `*.json` 파일을 이름 순으로 모은다.
fn json_files(dir: &Path) -> Result<Vec<PathBuf>, DataError> {
    let entries = fs::read_dir(dir).map_err(|source| DataError::ReadDir {
        dir: dir.to_path_buf(),
        source,
    })?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    Ok(files)
}

/// `serde_json::from_str` 대신 `serde_path_to_error` 로 감싼 역직렬화기를 쓴다(S-R25).
///
/// 이유: `starfall_contracts::data` 의 `deserialize_with` 함수 10개가 필드 여러 개에서
/// 공유된다(`de_boundary_radius_m` 이 `soft_boundary_radius_m`/`hard_boundary_radius_m`/
/// `visual_radius_m` 세 필드 모두에 쓰인다). 그 함수의 에러 메시지는 `stringify!($fn_name)`
/// 만 담아 **어느 필드가 위반했는지 말하지 못한다**. `serde_path_to_error::deserialize` 는
/// 값이 아니라 **역직렬화 트리의 위치**를 추적하므로, 검증자가 공유돼도 JSON 경로
/// (`play_area.hard_boundary_radius_m`)는 항상 1:1 이다.
///
/// 로깅은 여기서 하지 않는다 — SC-07 로그 줄은 `load()` 끝에서, **7종 로더가 전부
/// 성공한 뒤** 한 번에 찍는다(팀 리더 방침, 2026-09-29). 이 함수 안에서 파일마다
/// `tracing::info!` 를 찍던 첫 버전은 단위 테스트가 `tracing::subscriber::with_default`
/// 로 로그를 캡처해 검증했는데, tracing 의 콜사이트 관심 캐시가 **프로세스 전역**이라
/// 다른 테스트가 이 콜사이트를 먼저 건드리면(패키지 테스트를 병렬로 같이 돌릴 때만)
/// 이 구독자에도 이벤트가 안 오는 flaky 가 실측됐다(10회 중 1회). `load()` 가
/// **돌려주는 값**(`GameData::loaded_files`)으로 재는 것은 전역 상태에 기대지 않는다.
fn read_and_parse<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T, DataError> {
    let raw = fs::read_to_string(file).map_err(|source| DataError::ReadFile {
        file: file.to_path_buf(),
        source,
    })?;
    let deserializer = &mut serde_json::Deserializer::from_str(&raw);
    serde_path_to_error::deserialize(deserializer).map_err(|error| {
        let path = error.path().to_string();
        DataError::Schema {
            file: file.to_path_buf(),
            error: format!("{path}: {}", error.into_inner()),
        }
    })
}

fn load_ship_classes(dir: &Path) -> Result<Vec<(PathBuf, ShipClassTable)>, DataError> {
    let files = json_files(dir)?;
    if files.is_empty() {
        return Err(DataError::NoShipClasses {
            dir: dir.to_path_buf(),
        });
    }
    files
        .into_iter()
        .map(|file| {
            let table: ShipClassTable = read_and_parse(&file)?;
            Ok((file, table))
        })
        .collect()
}

/// 함선 클래스 유도값 검산: `id` 중복 없음, `main_thrust_mps2 ≥ lateral/reverse`(대각선
/// 클램프 상수가 항상 최댓값이어야 한다 — ADR-0010 §2, designer 통보 D-12).
fn validate_ship_classes(
    loaded: &[(PathBuf, ShipClassTable)],
) -> Result<BTreeMap<String, ShipClassTable>, DataError> {
    let mut by_id: BTreeMap<String, (PathBuf, ShipClassTable)> = BTreeMap::new();

    for (file, table) in loaded {
        let id = table.id.as_str().to_owned();
        if let Some((prev_file, _)) = by_id.get(&id) {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "id".to_owned(),
                expected: format!(
                    "고유 id (이미 {} 에서 사용됨 — 중복 없음, S2 검산)",
                    prev_file.display()
                ),
                actual: id,
            });
        }

        let movement = &table.movement;
        if movement.main_thrust_mps2 < movement.lateral_thrust_mps2 {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "movement.main_thrust_mps2".to_owned(),
                expected: format!(">= lateral_thrust_mps2 ({})", movement.lateral_thrust_mps2),
                actual: movement.main_thrust_mps2.to_string(),
            });
        }
        if movement.main_thrust_mps2 < movement.reverse_thrust_mps2 {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "movement.main_thrust_mps2".to_owned(),
                expected: format!(">= reverse_thrust_mps2 ({})", movement.reverse_thrust_mps2),
                actual: movement.main_thrust_mps2.to_string(),
            });
        }

        by_id.insert(id, (file.clone(), table.clone()));
    }

    Ok(by_id
        .into_iter()
        .map(|(id, (_, table))| (id, table))
        .collect())
}

fn load_star_system(dir: &Path) -> Result<(PathBuf, StarSystemTable), DataError> {
    let files = json_files(dir)?;
    match files.len() {
        0 => Err(DataError::NoStarSystems {
            dir: dir.to_path_buf(),
        }),
        1 => {
            let file = files.into_iter().next().unwrap_or_default();
            let table: StarSystemTable = read_and_parse(&file)?;
            Ok((file, table))
        }
        _ => Err(DataError::AmbiguousStarSystem {
            dir: dir.to_path_buf(),
            files: files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

/// 성계 유도값 검산: `point_count == len(points_m)`, `soft ≤ hard`, 스폰 지점이 하드 경계
/// 안, `reconnect_resume_window_seconds ≤ linger_seconds`(I-38).
fn validate_star_system(file: &Path, table: &StarSystemTable) -> Result<(), DataError> {
    let expected_len = table.spawn.points_m.len();
    let point_count = usize::try_from(table.spawn.point_count).unwrap_or(usize::MAX);
    if point_count != expected_len {
        return Err(DataError::Derived {
            file: file.to_path_buf(),
            field: "spawn.point_count".to_owned(),
            expected: format!("points_m.len() ({expected_len}) 과 같다"),
            actual: table.spawn.point_count.to_string(),
        });
    }

    if table.play_area.soft_boundary_radius_m > table.play_area.hard_boundary_radius_m {
        return Err(DataError::Derived {
            file: file.to_path_buf(),
            field: "play_area.soft_boundary_radius_m".to_owned(),
            expected: format!(
                "<= hard_boundary_radius_m ({})",
                table.play_area.hard_boundary_radius_m
            ),
            actual: table.play_area.soft_boundary_radius_m.to_string(),
        });
    }

    for (index, point) in table.spawn.points_m.iter().enumerate() {
        let distance = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
        if distance > table.play_area.hard_boundary_radius_m {
            return Err(DataError::Derived {
                file: file.to_path_buf(),
                field: format!("spawn.points_m[{index}]"),
                expected: format!(
                    "원점 거리 <= hard_boundary_radius_m ({})",
                    table.play_area.hard_boundary_radius_m
                ),
                actual: format!("{distance} ({point:?})"),
            });
        }
    }

    if table.presence.reconnect_resume_window_seconds > table.presence.linger_seconds {
        return Err(DataError::Derived {
            file: file.to_path_buf(),
            field: "presence.reconnect_resume_window_seconds".to_owned(),
            expected: format!("<= linger_seconds ({})", table.presence.linger_seconds),
            actual: table.presence.reconnect_resume_window_seconds.to_string(),
        });
    }

    Ok(())
}

fn load_sync_tuning(file: &Path) -> Result<SyncTuningTable, DataError> {
    read_and_parse(file)
}

/// `tick_hz / snapshot_hz` 를 유도한다. 나누어떨어지지 않으면 거부한다(ADR-0011 §2).
fn validate_snapshot_interval(
    file: &Path,
    tick_hz: u32,
    snapshot_hz: i64,
) -> Result<u32, DataError> {
    let snapshot_hz_u32 = u32::try_from(snapshot_hz).unwrap_or(0);
    if snapshot_hz_u32 == 0 || !tick_hz.is_multiple_of(snapshot_hz_u32) {
        return Err(DataError::Derived {
            file: file.to_path_buf(),
            field: "snapshot.snapshot_hz".to_owned(),
            expected: format!("tick_hz({tick_hz}) 을 나누어떨어뜨리는 값"),
            actual: snapshot_hz.to_string(),
        });
    }
    Ok(tick_hz / snapshot_hz_u32)
}

// ---------------------------------------------------------------------------
// p1-02 — data/minerals, data/world/deposits, data/mining, data/history/rules
// ---------------------------------------------------------------------------

fn load_minerals(dir: &Path) -> Result<Vec<(PathBuf, MineralTable)>, DataError> {
    let files = json_files(dir)?;
    if files.is_empty() {
        return Err(DataError::NoMinerals {
            dir: dir.to_path_buf(),
        });
    }
    files
        .into_iter()
        .map(|file| {
            let table: MineralTable = read_and_parse(&file)?;
            Ok((file, table))
        })
        .collect()
}

/// 광물 `id` 중복 없음(§4.7).
fn validate_minerals(
    loaded: &[(PathBuf, MineralTable)],
) -> Result<BTreeMap<String, MineralTable>, DataError> {
    let mut by_id: BTreeMap<String, MineralTable> = BTreeMap::new();
    for (file, table) in loaded {
        let id = table.id.as_str().to_owned();
        if by_id.contains_key(&id) {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "id".to_owned(),
                expected: "고유 id (광물 id 중복 없음, §4.7)".to_owned(),
                actual: id,
            });
        }
        by_id.insert(id, table.clone());
    }
    Ok(by_id)
}

fn load_mining_rules(file: &Path) -> Result<MiningRulesTable, DataError> {
    read_and_parse(file)
}

/// `cooldown_s * tick_hz` 를 유도한다. 모듈 문서 참고 — 스키마가 두 값 모두 정수로
/// 강제해 곱은 언제나 정수다(유효한 JSON 으로는 이 함수가 실패할 수 없다). 그래도
/// `checked_mul` 로 오버플로만은 막는다.
fn derive_cooldown_ticks(file: &Path, cooldown_s: i64, tick_hz: u32) -> Result<u64, DataError> {
    let cooldown_s_u64 = u64::try_from(cooldown_s).unwrap_or(0);
    cooldown_s_u64
        .checked_mul(u64::from(tick_hz))
        .ok_or_else(|| DataError::Derived {
            file: file.to_path_buf(),
            field: "extraction.cooldown_s".to_owned(),
            expected: format!("* tick_hz({tick_hz}) 가 u64 오버플로 없음"),
            actual: cooldown_s.to_string(),
        })
}

fn load_deposit_fields(dir: &Path) -> Result<Vec<(PathBuf, DepositFieldTable)>, DataError> {
    let files = json_files(dir)?;
    if files.is_empty() {
        return Err(DataError::NoDeposits {
            dir: dir.to_path_buf(),
        });
    }
    files
        .into_iter()
        .map(|file| {
            let table: DepositFieldTable = read_and_parse(&file)?;
            Ok((file, table))
        })
        .collect()
}

/// 매장지 유도값 검산 전체(§4.7): 파일의 `star_system_id` 일치 / `mineral_id` 가 광물
/// 표에 존재 / 모든 광물이 매장지 ≥1 / 소프트 경계 안 / `regen_kg ≤ initial_reserve_kg`
/// / 매장지 `id` 중복 없음 / 두 매장지의 채굴 구역이 겹치지 않음.
fn validate_deposit_fields(
    loaded: &[(PathBuf, DepositFieldTable)],
    star_system_id: &str,
    soft_boundary_radius_m: f64,
    mining_range_from_surface_m: f64,
    minerals: &BTreeMap<String, MineralTable>,
) -> Result<BTreeMap<String, Deposit>, DataError> {
    let mut by_id: BTreeMap<String, (PathBuf, Deposit)> = BTreeMap::new();
    let mut minerals_with_deposit: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

    for (file, table) in loaded {
        if table.star_system_id.as_str() != star_system_id {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "star_system_id".to_owned(),
                expected: format!("로딩된 성계와 일치 ({star_system_id})"),
                actual: table.star_system_id.as_str().to_owned(),
            });
        }

        for deposit in &table.deposits {
            let deposit_id = deposit.id.as_str().to_owned();
            if by_id.contains_key(&deposit_id) {
                return Err(DataError::Derived {
                    file: file.clone(),
                    field: "deposits[].id".to_owned(),
                    expected: "고유 id (광맥 id 중복 없음, §4.7)".to_owned(),
                    actual: deposit_id,
                });
            }

            let mineral_id = deposit.mineral_id.as_str();
            let mineral = minerals.get(mineral_id).ok_or_else(|| DataError::Derived {
                file: file.clone(),
                field: format!("deposits[{deposit_id}].mineral_id"),
                expected: "광물 표(data/minerals/)에 존재".to_owned(),
                actual: mineral_id.to_owned(),
            })?;

            let distance_from_origin = (deposit.position_m[0] * deposit.position_m[0]
                + deposit.position_m[1] * deposit.position_m[1]
                + deposit.position_m[2] * deposit.position_m[2])
                .sqrt();
            let reach = distance_from_origin + deposit.radius_m + mining_range_from_surface_m;
            if reach > soft_boundary_radius_m {
                return Err(DataError::Derived {
                    file: file.clone(),
                    field: format!("deposits[{deposit_id}].position_m"),
                    expected: format!(
                        "|position| + radius_m + mining_range_from_surface_m <= soft_boundary_radius_m ({soft_boundary_radius_m})"
                    ),
                    actual: reach.to_string(),
                });
            }

            let regen_kg = mineral.regeneration.regen_kg.get();
            let initial_reserve_kg = deposit.initial_reserve_kg.get();
            if regen_kg > initial_reserve_kg {
                return Err(DataError::Derived {
                    file: file.clone(),
                    field: format!("deposits[{deposit_id}].initial_reserve_kg"),
                    expected: format!(
                        ">= mineral({mineral_id}).regeneration.regen_kg ({regen_kg})"
                    ),
                    actual: initial_reserve_kg.to_string(),
                });
            }

            minerals_with_deposit.insert(mineral_id.to_owned());
            by_id.insert(deposit_id, (file.clone(), deposit.clone()));
        }
    }

    for mineral_id in minerals.keys() {
        if !minerals_with_deposit.contains(mineral_id) {
            return Err(DataError::Derived {
                file: loaded
                    .first()
                    .map(|(file, _)| file.clone())
                    .unwrap_or_default(),
                field: "deposits".to_owned(),
                expected: format!("광물 {mineral_id} 을(를) 내는 매장지 >= 1(§4.7)"),
                actual: "0".to_owned(),
            });
        }
    }

    let deposits: Vec<(String, Deposit)> = by_id
        .iter()
        .map(|(id, (_, deposit))| (id.clone(), deposit.clone()))
        .collect();
    for (i, (id_a, a)) in deposits.iter().enumerate() {
        for (id_b, b) in deposits.iter().skip(i + 1) {
            let dx = a.position_m[0] - b.position_m[0];
            let dy = a.position_m[1] - b.position_m[1];
            let dz = a.position_m[2] - b.position_m[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            let clearance = a.radius_m + b.radius_m + 2.0 * mining_range_from_surface_m;
            if distance <= clearance {
                let file = by_id
                    .get(id_a)
                    .map(|(file, _)| file.clone())
                    .unwrap_or_default();
                return Err(DataError::Derived {
                    file,
                    field: format!("deposits[{id_a}]<->deposits[{id_b}]"),
                    expected: format!(
                        "거리 > 두 반지름 합 + 2 * mining_range_from_surface_m ({clearance})"
                    ),
                    actual: distance.to_string(),
                });
            }
        }
    }

    Ok(by_id
        .into_iter()
        .map(|(id, (_, deposit))| (id, deposit))
        .collect())
}

fn load_significance_rules(dir: &Path) -> Result<Vec<(PathBuf, SignificanceRuleTable)>, DataError> {
    let files = json_files(dir)?;
    if files.is_empty() {
        return Err(DataError::NoSignificanceRules {
            dir: dir.to_path_buf(),
        });
    }
    files
        .into_iter()
        .map(|file| {
            let table: SignificanceRuleTable = read_and_parse(&file)?;
            Ok((file, table))
        })
        .collect()
}

/// `rule_version` 이 `rule_id + "@"` 로 시작 / `produces_event_type` 이 레지스트리의
/// `historical_event` 타입(§4.7).
fn validate_significance_rules(
    loaded: &[(PathBuf, SignificanceRuleTable)],
) -> Result<BTreeMap<String, SignificanceRuleTable>, DataError> {
    let mut by_id = BTreeMap::new();
    for (file, table) in loaded {
        let rule_id = table.rule_id.as_str();
        let expected_prefix = format!("{rule_id}@");
        if !table.rule_version.as_str().starts_with(&expected_prefix) {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "rule_version".to_owned(),
                expected: format!("\"{expected_prefix}\" 로 시작"),
                actual: table.rule_version.as_str().to_owned(),
            });
        }

        let is_registered_historical_event = starfall_contracts::registry::CONTRACT_TYPES
            .iter()
            .any(|entry| {
                entry.kind == "historical_event" && entry.name == table.produces_event_type.as_str()
            });
        if !is_registered_historical_event {
            return Err(DataError::Derived {
                file: file.clone(),
                field: "produces_event_type".to_owned(),
                expected: "레지스트리의 historical_event 타입".to_owned(),
                actual: table.produces_event_type.clone(),
            });
        }

        by_id.insert(rule_id.to_owned(), table.clone());
    }
    Ok(by_id)
}

/// `data_dir` 아래 7종(p1-01 3 + p1-02 4)을 전부 로드하고 검산한다. 실패는 전부 기동
/// 거부다(I-38).
///
/// # Errors
///
/// 파일을 읽지 못했거나, 스키마 층(필드 범위) 또는 유도값 층(필드 간 관계)을 어기면 실패한다.
pub fn load(data_dir: &Path, tick_hz: u32) -> Result<GameData, DataError> {
    let ship_class_files = load_ship_classes(&data_dir.join("ships"))?;
    let ship_classes = validate_ship_classes(&ship_class_files)?;

    let (star_system_file, star_system) = load_star_system(&data_dir.join("world/systems"))?;
    validate_star_system(&star_system_file, &star_system)?;

    let sync_tuning_file = data_dir.join("movement/sync-tuning.json");
    let sync_tuning = load_sync_tuning(&sync_tuning_file)?;
    let snapshot_interval_ticks =
        validate_snapshot_interval(&sync_tuning_file, tick_hz, sync_tuning.snapshot.snapshot_hz)?;

    let mineral_files = load_minerals(&data_dir.join("minerals"))?;
    let minerals = validate_minerals(&mineral_files)?;

    let mining_rules_file = data_dir.join("mining/mining-rules.json");
    let mining_rules = load_mining_rules(&mining_rules_file)?;
    let mining_cooldown_ticks = derive_cooldown_ticks(
        &mining_rules_file,
        mining_rules.extraction.cooldown_s,
        tick_hz,
    )?;

    let deposit_files = load_deposit_fields(&data_dir.join("world/deposits"))?;
    let deposits = validate_deposit_fields(
        &deposit_files,
        star_system.id.as_str(),
        star_system.play_area.soft_boundary_radius_m,
        mining_rules.extraction.mining_range_from_surface_m,
        &minerals,
    )?;

    let significance_rule_files = load_significance_rules(&data_dir.join("history/rules"))?;
    let significance_rules = validate_significance_rules(&significance_rule_files)?;

    // SC-07 — 여기가 유일한 지점이다: 7종 로더가 **전부 성공한 뒤에만** 도달하므로,
    // 실패한 파일이 성공 줄로 찍히는 일이 없다(qa 요구). 로그(실서버 증거, 실행
    // 시점의 부작용)와 반환값(단위 테스트가 재는 값, 전역 상태 없음) 둘 다 같은
    // 목록에서 만든다 — 팀 리더 방침(2026-09-29): 캡처가 아니라 반환값으로 잰다. 순서는
    // §2 표(SC-07 판정 대상)와 같은 7종 순서.
    let loaded_files: Vec<PathBuf> = ship_class_files
        .iter()
        .map(|(file, _)| file.clone())
        .chain(std::iter::once(star_system_file))
        .chain(std::iter::once(sync_tuning_file))
        .chain(mineral_files.iter().map(|(file, _)| file.clone()))
        .chain(std::iter::once(mining_rules_file))
        .chain(deposit_files.iter().map(|(file, _)| file.clone()))
        .chain(significance_rule_files.iter().map(|(file, _)| file.clone()))
        .collect();
    let loaded_files: Vec<String> = loaded_files
        .iter()
        .map(|file| {
            let relative = file.strip_prefix(data_dir).unwrap_or(file);
            relative.display().to_string().replace('\\', "/")
        })
        .collect();
    for data_file in &loaded_files {
        tracing::info!(data_file = %data_file, "데이터 파일 적재");
    }

    Ok(GameData {
        data_dir: data_dir.to_path_buf(),
        ship_classes,
        star_system,
        sync_tuning,
        snapshot_interval_ticks,
        minerals,
        deposits,
        mining_rules,
        mining_cooldown_ticks,
        significance_rules,
        loaded_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 레포 루트의 실제 `data/`. `contracts/registry/types.json` 을 마커로 찾는다
    /// (계약 테스트와 같은 패턴 — 고정 `../../..` 는 크레이트가 옮겨지면 조용히 틀린다).
    fn repo_root() -> PathBuf {
        let start = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut dir = start;
        loop {
            if dir.join("contracts/registry/types.json").is_file() {
                return dir.to_path_buf();
            }
            dir = dir.parent().expect("레포 루트를 찾지 못했다");
        }
    }

    /// SC-05 — 정상 `data/` 3파일로 로드하면 함선 클래스 수·스폰 지점 수·
    /// 유도된 `snapshot_interval_ticks` 가 파일과 일치한다.
    #[test]
    fn loads_the_real_data_directory() {
        let data_dir = repo_root().join("data");
        let loaded = load(&data_dir, 20).expect("실제 data/ 로딩 실패");
        assert_eq!(loaded.ship_classes.len(), 1, "클래스 1종(scout-s01)");
        assert!(loaded.ship_classes.contains_key("scout-s01"));
        assert_eq!(loaded.spawn_point_count(), 12);
        assert_eq!(loaded.snapshot_interval_ticks, 2, "20 / 10 = 2");
    }

    /// 임시 디렉토리에 위반 데이터 하나를 심고 8종 중 하나씩 거부되는지 본다(SC-06).
    struct TempDataDir {
        root: PathBuf,
    }

    impl TempDataDir {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("starfall-data-test-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("ships")).unwrap();
            fs::create_dir_all(root.join("world/systems")).unwrap();
            fs::create_dir_all(root.join("movement")).unwrap();
            fs::create_dir_all(root.join("minerals")).unwrap();
            fs::create_dir_all(root.join("world/deposits")).unwrap();
            fs::create_dir_all(root.join("mining")).unwrap();
            fs::create_dir_all(root.join("history/rules")).unwrap();
            Self { root }
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.root.join(relative);
            let mut file = fs::File::create(&path).unwrap();
            file.write_all(contents.as_bytes()).unwrap();
        }
    }

    impl Drop for TempDataDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn valid_ship_class() -> &'static str {
        include_str!("../../../../contracts/fixtures/SHIP_CLASS/example-scout.json")
    }

    fn valid_sync_tuning() -> &'static str {
        include_str!("../../../../contracts/fixtures/SYNC_TUNING/example-p1-01-default.json")
    }

    fn seed_valid(dir: &TempDataDir) {
        dir.write("ships/scout-s01.json", valid_ship_class());
        dir.write("movement/sync-tuning.json", valid_sync_tuning());
    }

    fn valid_star_system_json() -> String {
        // 실제 cradle.json 을 그대로 쓰면 유도값이 전부 통과한다는 것을 함께 보증한다.
        fs::read_to_string(repo_root().join("data/world/systems/cradle.json")).unwrap()
    }

    /// p1-02 4종을 실제 `data/` 사본으로 심는다(§4.7 검산이 전부 통과함을 함께 보증한다 —
    /// `accepts_unmodified_copy`와 같은 정신). 개별 reject_c* 테스트는 이 결과를 문자열
    /// 치환으로 살짝 어긋내 쓴다.
    fn seed_valid_p102(dir: &TempDataDir) {
        for mineral in ["ferrosite", "glacine", "cobaltine", "starfall-glass"] {
            dir.write(
                &format!("minerals/{mineral}.json"),
                &fs::read_to_string(repo_root().join(format!("data/minerals/{mineral}.json")))
                    .unwrap(),
            );
        }
        dir.write(
            "world/deposits/cradle.json",
            &fs::read_to_string(repo_root().join("data/world/deposits/cradle.json")).unwrap(),
        );
        dir.write(
            "mining/mining-rules.json",
            &fs::read_to_string(repo_root().join("data/mining/mining-rules.json")).unwrap(),
        );
        dir.write(
            "history/rules/mineral-discovery.json",
            &fs::read_to_string(repo_root().join("data/history/rules/mineral-discovery.json"))
                .unwrap(),
        );
    }

    /// 실제 `data/` 전체(7종)를 심은 상태에서 `load()`를 부른다.
    fn seed_all_valid(dir: &TempDataDir) {
        seed_valid(dir);
        dir.write("world/systems/cradle.json", &valid_star_system_json());
        seed_valid_p102(dir);
    }

    #[test]
    fn rejects_hard_radius_above_ceiling() {
        let dir = TempDataDir::new("hard-radius");
        seed_valid(&dir);
        let mut system = valid_star_system_json();
        system = system.replace(
            "\"hard_boundary_radius_m\": 12000.0",
            "\"hard_boundary_radius_m\": 20000.1",
        );
        dir.write("world/systems/cradle.json", &system);

        let error = load(&dir.root, 20).unwrap_err();
        assert!(matches!(error, DataError::Schema { .. }), "{error}");
        assert!(
            error
                .to_string()
                .contains("play_area.hard_boundary_radius_m"),
            "S-R25 — 셋이 공유하는 de_boundary_radius_m 이 아니라 JSON 경로로 필드가 특정돼야 한다: {error}"
        );
    }

    /// S-R25 음성 대조 — `soft_boundary_radius_m` 도 `de_boundary_radius_m` 을 공유하는
    /// 같은 파일의 옆 줄(`cradle.json:16` vs `:17`)이다. 위 [`rejects_hard_radius_above_ceiling`]
    /// 과 이 테스트의 오류 메시지가 **서로 다른 필드**를 지칭해야 구제가 실제로 된 것이다 —
    /// 둘 다 `de_boundary_radius_m` 만 찍으면 공유 검증자 결함이 그대로 남은 것이다.
    #[test]
    fn rejects_soft_radius_above_ceiling_and_names_a_different_field_than_hard() {
        let dir = TempDataDir::new("soft-radius");
        seed_valid(&dir);
        let mut system = valid_star_system_json();
        system = system.replace(
            "\"soft_boundary_radius_m\": 10000.0",
            "\"soft_boundary_radius_m\": 20000.1",
        );
        dir.write("world/systems/cradle.json", &system);

        let error = load(&dir.root, 20).unwrap_err();
        assert!(matches!(error, DataError::Schema { .. }), "{error}");
        let message = error.to_string();
        assert!(
            message.contains("play_area.soft_boundary_radius_m"),
            "S-R25 — JSON 경로로 soft 필드가 특정돼야 한다: {message}"
        );
        assert!(
            !message.contains("play_area.hard_boundary_radius_m"),
            "S-R25 음성 대조 실패 — soft 위반 로그가 hard 필드를 지칭한다: {message}"
        );
    }

    #[test]
    fn rejects_empty_spawn_points() {
        let dir = TempDataDir::new("empty-spawn");
        seed_valid(&dir);
        dir.write(
            "world/systems/cradle.json",
            r#"{"schema_version":1,"id":"cradle","display_name":"Cradle",
            "coordinate_space":{"frame":"system_local","unit":"meter","server_type":"f64","origin":[0,0,0],"up_axis":[0,1,0]},
            "play_area":{"soft_boundary_radius_m":10000.0,"hard_boundary_radius_m":12000.0,"boundary_pull_mps2":25.0},
            "spawn":{"point_count":1,"clearance_m":150.0,"max_probe_attempts":12,"radial_offset_step_m":150.0,
            "initial_velocity_mps":[0,0,0],"initial_facing":"system_origin","assignment_rule":"x","points_m":[]},
            "presence":{"linger_seconds":30,"reconnect_resume_window_seconds":30}}"#,
        );
        let error = load(&dir.root, 20).unwrap_err();
        assert!(matches!(error, DataError::Schema { .. }), "{error}");
    }

    #[test]
    fn rejects_point_count_mismatch() {
        let dir = TempDataDir::new("point-count-mismatch");
        seed_valid(&dir);
        let mut system = valid_star_system_json();
        system = system.replace("\"point_count\": 12,", "\"point_count\": 11,");
        dir.write("world/systems/cradle.json", &system);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "spawn.point_count"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn rejects_non_integer_tick_ratio() {
        // sync-tuning 의 snapshot_hz 는 10이다. tick_hz=20 은 나누어떨어지지만
        // tick_hz=7 은 나누어떨어지지 않는다 — 그 값으로 검산이 걸리는지 본다.
        let dir = TempDataDir::new("tick-ratio");
        seed_valid(&dir);
        dir.write("world/systems/cradle.json", &valid_star_system_json());
        let error = load(&dir.root, 7).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "snapshot.snapshot_hz"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn rejects_resume_window_longer_than_linger() {
        let dir = TempDataDir::new("resume-window");
        seed_valid(&dir);
        let mut system = valid_star_system_json();
        system = system.replace(
            "\"reconnect_resume_window_seconds\": 30",
            "\"reconnect_resume_window_seconds\": 31",
        );
        dir.write("world/systems/cradle.json", &system);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => {
                assert_eq!(field, "presence.reconnect_resume_window_seconds");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn rejects_spawn_point_outside_hard_boundary() {
        let dir = TempDataDir::new("spawn-outside");
        seed_valid(&dir);
        dir.write(
            "world/systems/cradle.json",
            r#"{"schema_version":1,"id":"cradle","display_name":"Cradle",
            "coordinate_space":{"frame":"system_local","unit":"meter","server_type":"f64","origin":[0,0,0],"up_axis":[0,1,0]},
            "play_area":{"soft_boundary_radius_m":10000.0,"hard_boundary_radius_m":12000.0,"boundary_pull_mps2":25.0},
            "spawn":{"point_count":1,"clearance_m":150.0,"max_probe_attempts":12,"radial_offset_step_m":150.0,
            "initial_velocity_mps":[0,0,0],"initial_facing":"system_origin","assignment_rule":"x",
            "points_m":[[13000.0,0.0,0.0]]},
            "presence":{"linger_seconds":30,"reconnect_resume_window_seconds":30}}"#,
        );
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert!(field.starts_with("spawn.points_m[")),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// S-R25 음성 대조 ②(§7a) — `de_accel_mps2` 는 `ShipClassMovement` 필드 6개
    /// (`main_thrust_mps2`/`reverse_thrust_mps2`/`lateral_thrust_mps2`/`brake_decel_mps2`/
    /// `assist_linear_decel_mps2`/`assist_lateral_decel_mps2`)와 다른 구조체 필드 1개까지
    /// 도합 7곳이 공유하는, 이 파일에서 가장 많이 공유되는 검증자다. 그중 둘을 각각
    /// 위반시켜 로그가 서로 다른 필드를 지칭하는지 본다 — 하나만 보면 "우연히 맞았다"를
    /// 배제할 수 없다.
    #[test]
    fn rejects_two_different_de_accel_mps2_fields_and_names_each_one() {
        let dir = TempDataDir::new("accel-mps2-main");
        dir.write("world/systems/cradle.json", &valid_star_system_json());
        dir.write("movement/sync-tuning.json", valid_sync_tuning());
        let bad_main = valid_ship_class().replace(
            "\"main_thrust_mps2\": 35.0",
            "\"main_thrust_mps2\": 1000000.1",
        );
        dir.write("ships/scout-s01.json", &bad_main);
        let error_main = load(&dir.root, 20).unwrap_err();
        let message_main = error_main.to_string();
        assert!(
            message_main.contains("movement.main_thrust_mps2"),
            "{message_main}"
        );

        let dir2 = TempDataDir::new("accel-mps2-brake");
        dir2.write("world/systems/cradle.json", &valid_star_system_json());
        dir2.write("movement/sync-tuning.json", valid_sync_tuning());
        let bad_brake = valid_ship_class().replace(
            "\"brake_decel_mps2\": 50.0",
            "\"brake_decel_mps2\": 1000000.1",
        );
        dir2.write("ships/scout-s01.json", &bad_brake);
        let error_brake = load(&dir2.root, 20).unwrap_err();
        let message_brake = error_brake.to_string();
        assert!(
            message_brake.contains("movement.brake_decel_mps2"),
            "{message_brake}"
        );

        assert_ne!(
            message_main, message_brake,
            "S-R25 음성 대조 실패 — 서로 다른 필드를 위반했는데 로그가 같다"
        );
        assert!(
            !message_main.contains("brake_decel_mps2")
                && !message_brake.contains("main_thrust_mps2"),
            "S-R25 음성 대조 실패 — 로그가 위반하지 않은 필드를 지칭한다: main={message_main} brake={message_brake}"
        );
    }

    #[test]
    fn rejects_main_thrust_below_lateral() {
        let dir = TempDataDir::new("thrust-clamp");
        dir.write("world/systems/cradle.json", &valid_star_system_json());
        dir.write("movement/sync-tuning.json", valid_sync_tuning());
        let bad_ship =
            valid_ship_class().replace("\"main_thrust_mps2\": 35.0", "\"main_thrust_mps2\": 10.0");
        dir.write("ships/scout-s01.json", &bad_ship);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "movement.main_thrust_mps2"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn rejects_duplicate_ship_class_id() {
        let dir = TempDataDir::new("dup-ship-id");
        dir.write("world/systems/cradle.json", &valid_star_system_json());
        dir.write("movement/sync-tuning.json", valid_sync_tuning());
        dir.write("ships/scout-s01.json", valid_ship_class());
        dir.write("ships/scout-s01-copy.json", valid_ship_class());
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "id"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn resolve_data_dir_uses_override_only_no_search() {
        let dir = TempDataDir::new("override-only");
        let resolved = resolve_data_dir(&std::env::temp_dir(), Some(&dir.root)).unwrap();
        assert_eq!(fs::canonicalize(&dir.root).unwrap(), resolved);
    }

    #[test]
    fn resolve_data_dir_reports_all_tried_paths_when_missing() {
        let cwd = std::env::temp_dir().join(format!("starfall-no-data-{}", std::process::id()));
        let _ = fs::remove_dir_all(&cwd);
        fs::create_dir_all(&cwd).unwrap();
        let error = resolve_data_dir(&cwd, None).unwrap_err();
        match &error {
            DataError::DataDirNotFound { tried } => {
                assert!(tried.contains("data"), "{tried}");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
        let _ = fs::remove_dir_all(&cwd);
    }

    // -----------------------------------------------------------------------
    // p1-02(S2) — SC-04·SC-05·SC-06. 스프린트 계약 §4.7 의 16 경우 중 ⑤·⑥(비정수 검산)은
    // 모듈 문서가 실측으로 설명하듯 구조적으로 도달 불가능하다(스키마가 이미 정수를
    // 강제) — 그 두 경우는 "거부" 테스트가 아니라 [`cooldown_ticks_is_exactly_derived`]
    // 로 대신한다.
    // -----------------------------------------------------------------------

    /// SC-04/SC-06 — 실제 `data/` 로 로드하면 광물 4·광맥 8·`rule_version` 이 파일과
    /// 일치하고, 판정 상수(사거리·속도·쿨다운 tick)도 파일 값에서 정확히 유도된다.
    #[test]
    fn accepts_unmodified_copy() {
        let dir = TempDataDir::new("p102-valid-copy");
        seed_all_valid(&dir);
        let loaded = load(&dir.root, 20).expect("실제 data/ 사본 로딩 실패");
        assert_eq!(loaded.minerals.len(), 4, "광물 4종");
        assert_eq!(loaded.deposits.len(), 8, "광맥 8개(cradle.json)");
        assert_eq!(loaded.significance_rules.len(), 1);
        assert_eq!(
            loaded.sole_rule_version(),
            Some("mineral-discovery@1"),
            "data/history/rules/mineral-discovery.json 의 rule_version"
        );
        // cooldown_s: 3, tick_hz: 20 → 60 tick(mining-rules.json).
        assert_eq!(loaded.mining_cooldown_ticks, 60);
    }

    /// 실제 `data/` 로 실서버가 하는 그대로 로드했을 때 이 슬라이스에서 마주치는
    /// 데이터 값으로도 [`derive_cooldown_ticks`]가 실제로 곱셈을 한다는 것(자명하게
    /// 통과하는 0×0 이 아님)을 단언한다 — 모듈 문서가 말하는 "⑤·⑥ 도달 불가능"의
    /// 양성 짝.
    #[test]
    fn cooldown_ticks_is_exactly_derived() {
        let dir = TempDataDir::new("p102-cooldown-derive");
        seed_all_valid(&dir);
        let loaded = load(&dir.root, 20).unwrap();
        assert_eq!(
            loaded.mining_rules.extraction.cooldown_s, 3,
            "전제 확인: mining-rules.json 의 cooldown_s"
        );
        assert_eq!(loaded.mining_cooldown_ticks, 3 * 20);
    }

    /// ① 광맥의 `mineral_id` 가 광물 표에 없음.
    #[test]
    fn reject_c01_deposit_unknown_mineral() {
        let dir = TempDataDir::new("p102-c01-unknown-mineral");
        seed_all_valid(&dir);
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace(
                "\"mineral_id\": \"ferrosite\"",
                "\"mineral_id\": \"unobtainium\"",
            );
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, actual, .. } => {
                assert!(field.contains("mineral_id"), "{error}");
                assert_eq!(actual, "unobtainium");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ② 광맥이 하나도 없는 광물(전부 다른 광물로 바꿔치기).
    #[test]
    fn reject_c02_mineral_without_deposit() {
        let dir = TempDataDir::new("p102-c02-orphan-mineral");
        seed_all_valid(&dir);
        // starfall-glass 를 내는 광맥이 하나뿐이다(far-reach) — 그것도 ferrosite 로 바꿔
        // starfall-glass 를 광맥 없는 광물로 만든다.
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace(
                "\"mineral_id\": \"starfall-glass\"",
                "\"mineral_id\": \"ferrosite\"",
            );
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, actual, .. } => {
                assert_eq!(field, "deposits");
                assert_eq!(actual, "0");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ③ 광맥 파일의 `star_system_id` 가 로딩된 성계와 다름.
    #[test]
    fn reject_c03_deposit_wrong_star_system() {
        let dir = TempDataDir::new("p102-c03-wrong-system");
        seed_all_valid(&dir);
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace(
                "\"star_system_id\": \"cradle\"",
                "\"star_system_id\": \"elsewhere\"",
            );
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, actual, .. } => {
                assert_eq!(field, "star_system_id");
                assert_eq!(actual, "elsewhere");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ④ `|position| + radius_m + mining_range_from_surface_m > soft_boundary_radius_m`.
    #[test]
    fn reject_c04_deposit_outside_soft_boundary() {
        let dir = TempDataDir::new("p102-c04-soft-boundary");
        seed_all_valid(&dir);
        // 스키마의 position_m 성분 상한(±20000, mineral.schema.json)은 넘지 않되 소프트
        // 경계(10000)는 넘는 값 — 유도값 층에서만 걸려야 한다(schema 층이 먼저 걸리면
        // ④ 를 재현하지 못한다).
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace("-3200.0, 150.0, -2400.0", "9900.0, 150.0, -2400.0");
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert!(field.contains("position_m"), "{error}"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑤ `cooldown_s` 소수(스프린트 계약 r2 보강 — 원문의 "곱이 비정수"는 스키마가 정수를
    /// 강제해 도달 불가라, 같은 의도의 도달 가능한 반례로 바꾼다: `mining-rules.json` 의
    /// `cooldown_s` 를 소수로). `de_cooldown_s`(`i64::deserialize`)가 부동소수점을
    /// 받으면 `invalid type` 로 거부한다 — rule=schema, 오류 메시지에 필드 이름이 있다.
    #[test]
    fn reject_c05_cooldown_fractional() {
        let dir = TempDataDir::new("p102-c05-cooldown-fractional");
        seed_all_valid(&dir);
        let rules = fs::read_to_string(dir.root.join("mining/mining-rules.json"))
            .unwrap()
            .replace("\"cooldown_s\": 3", "\"cooldown_s\": 3.5");
        dir.write("mining/mining-rules.json", &rules);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(
                    error.contains("cooldown_s"),
                    "오류 메시지에 필드 이름이 있어야 한다: {error}"
                );
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑥ `regen_interval_s` 소수 — ⑤ 와 같은 사정, `de_regen_interval_s` 가 거부한다.
    #[test]
    fn reject_c06_regen_interval_fractional() {
        let dir = TempDataDir::new("p102-c06-regen-interval-fractional");
        seed_all_valid(&dir);
        let mineral = fs::read_to_string(dir.root.join("minerals/starfall-glass.json"))
            .unwrap()
            .replace("\"regen_interval_s\": 180", "\"regen_interval_s\": 180.5");
        dir.write("minerals/starfall-glass.json", &mineral);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(
                    error.contains("regen_interval_s"),
                    "오류 메시지에 필드 이름이 있어야 한다: {error}"
                );
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑦ `regen_kg > initial_reserve_kg`.
    #[test]
    fn reject_c07_regen_exceeds_initial_reserve() {
        let dir = TempDataDir::new("p102-c07-regen-exceeds");
        seed_all_valid(&dir);
        // far-reach 의 initial_reserve_kg 는 500, starfall-glass 의 regen_kg 는 25 다.
        // regen_kg 를 501 로 올려 초과시킨다.
        let mineral = fs::read_to_string(dir.root.join("minerals/starfall-glass.json"))
            .unwrap()
            .replace("\"regen_kg\": 25", "\"regen_kg\": 501");
        dir.write("minerals/starfall-glass.json", &mineral);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, actual, .. } => {
                assert!(field.contains("initial_reserve_kg"), "{error}");
                assert_eq!(actual, "500");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑧ 광물 `id` 중복.
    #[test]
    fn reject_c08_duplicate_mineral_id() {
        let dir = TempDataDir::new("p102-c08-dup-mineral");
        seed_all_valid(&dir);
        dir.write(
            "minerals/ferrosite-copy.json",
            &fs::read_to_string(dir.root.join("minerals/ferrosite.json")).unwrap(),
        );
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "id"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑨ 광맥 `id` 중복(다른 파일 — 이 슬라이스는 성계 파일이 하나뿐이므로 두 번째
    /// 성계 파일에 같은 `id` 를 넣어 재현한다).
    #[test]
    fn reject_c09_duplicate_deposit_id() {
        let dir = TempDataDir::new("p102-c09-dup-deposit");
        seed_all_valid(&dir);
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json")).unwrap();
        // 광맥 목록에 첫 항목("inner-belt-1")을 그대로 복제해 넣는다.
        let duplicated =
            deposits.replacen("\"id\": \"inner-belt-2\"", "\"id\": \"inner-belt-1\"", 1);
        dir.write("world/deposits/cradle.json", &duplicated);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert!(field.contains("deposits[].id"), "{error}"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑩ 두 광맥의 채굴 구역이 겹침(`거리 <= 두 반지름 합 + 2 × 사거리`).
    #[test]
    fn reject_c10_overlapping_mining_zones() {
        let dir = TempDataDir::new("p102-c10-overlap");
        seed_all_valid(&dir);
        // inner-belt-2 를 inner-belt-1 과 같은 좌표로 옮긴다 — 거리 0, 확실히 겹친다.
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace("3600.0, 400.0, 1900.0", "-3200.0, 150.0, -2400.0");
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert!(field.contains("<->"), "{error}"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑪ `rule_version` 이 `rule_id + "@"` 로 시작하지 않음.
    #[test]
    fn reject_c11_rule_version_wrong_prefix() {
        let dir = TempDataDir::new("p102-c11-rule-version");
        seed_all_valid(&dir);
        let rule = fs::read_to_string(dir.root.join("history/rules/mineral-discovery.json"))
            .unwrap()
            .replace(
                "\"rule_version\": \"mineral-discovery@1\"",
                "\"rule_version\": \"something-else@1\"",
            );
        dir.write("history/rules/mineral-discovery.json", &rule);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, .. } => assert_eq!(field, "rule_version"),
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑫ `produces_event_type` 이 레지스트리의 `historical_event` 타입이 아님.
    #[test]
    fn reject_c12_produces_event_type_not_registered() {
        let dir = TempDataDir::new("p102-c12-produces-event-type");
        seed_all_valid(&dir);
        let rule = fs::read_to_string(dir.root.join("history/rules/mineral-discovery.json"))
            .unwrap()
            .replace(
                "\"produces_event_type\": \"MINERAL_DISCOVERED\"",
                "\"produces_event_type\": \"NOT_A_REAL_EVENT\"",
            );
        dir.write("history/rules/mineral-discovery.json", &rule);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Derived { field, actual, .. } => {
                assert_eq!(field, "produces_event_type");
                assert_eq!(actual, "NOT_A_REAL_EVENT");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// ⑬~⑯ 신규 스키마 4종 각각의 필드 범위 위반 — rule=schema, 이 모듈이 유도값을
    /// 따로 검산하기 전에 serde 가 먼저 거부한다(ADR-0002 §3, p1-01 과 같은 층).
    /// r2 보강(qa census): 오류 **종류**뿐 아니라 **주입한 필드 이름**도 단언한다 —
    /// `DataError::Schema.error` 가 필드 이름 없이도 매칭될 수 있어서다(p1-01 규칙 9 근거
    /// 사례와 같은 함정).
    #[test]
    fn reject_c13_mineral_schema_violation() {
        let dir = TempDataDir::new("p102-c13-mineral-schema");
        seed_all_valid(&dir);
        let mineral = fs::read_to_string(dir.root.join("minerals/ferrosite.json"))
            .unwrap()
            .replace("\"rarity\": \"common\"", "\"rarity\": \"legendary\"");
        dir.write("minerals/ferrosite.json", &mineral);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(error.contains("rarity"), "필드 이름이 있어야 한다: {error}");
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn reject_c14_deposit_field_schema_violation() {
        let dir = TempDataDir::new("p102-c14-deposit-schema");
        seed_all_valid(&dir);
        let deposits = fs::read_to_string(dir.root.join("world/deposits/cradle.json"))
            .unwrap()
            .replace("\"radius_m\": 60.0", "\"radius_m\": -1.0");
        dir.write("world/deposits/cradle.json", &deposits);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(
                    error.contains("radius_m"),
                    "필드 이름이 있어야 한다: {error}"
                );
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn reject_c15_mining_rules_schema_violation() {
        let dir = TempDataDir::new("p102-c15-mining-rules-schema");
        seed_all_valid(&dir);
        let rules = fs::read_to_string(dir.root.join("mining/mining-rules.json"))
            .unwrap()
            .replace("\"cooldown_s\": 3", "\"cooldown_s\": 0");
        dir.write("mining/mining-rules.json", &rules);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(
                    error.contains("cooldown_s"),
                    "필드 이름이 있어야 한다: {error}"
                );
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    #[test]
    fn reject_c16_significance_rule_schema_violation() {
        let dir = TempDataDir::new("p102-c16-significance-rule-schema");
        seed_all_valid(&dir);
        let rule = fs::read_to_string(dir.root.join("history/rules/mineral-discovery.json"))
            .unwrap()
            .replace("\"importance_level\": 2", "\"importance_level\": 6");
        dir.write("history/rules/mineral-discovery.json", &rule);
        let error = load(&dir.root, 20).unwrap_err();
        match &error {
            DataError::Schema { error, .. } => {
                assert!(
                    error.contains("importance_level"),
                    "필드 이름이 있어야 한다: {error}"
                );
            }
            other => panic!("기대와 다른 오류: {other}"),
        }
    }

    /// SC-07 — 성공 로딩 경로에서 파일마다 `데이터 파일 적재 data_file=<상대 경로>` 한
    /// 줄을 찍는다(qa `data_file_pairs.py` 가 이 줄로 대조한다). 실제 로그 줄 형식은
    /// qa 가 실바이너리 기동 로그로 판정한다 — 이 단위 테스트는 **로그를 캡처하지
    /// 않는다**. `tracing::subscriber::with_default` 로 캡처하던 첫 버전은 tracing 의
    /// 콜사이트 관심 캐시가 **프로세스 전역** 이라, 패키지 테스트를 병렬로 같이 돌리면
    /// 다른 테스트가 이 콜사이트를 먼저 건드려 이 구독자에 이벤트가 안 오는 flaky 가
    /// 실측됐다(10회 중 1회, 팀 리더 발견 2026-09-29). 로그는 `load()` 가 돌려주는
    /// [`GameData::loaded_files`] 와 **같은 목록**에서 만들어지므로(코드로 확인
    /// 가능), 이 값을 재는 것으로 전역 상태 없이 같은 사실을 증명한다.
    #[test]
    fn returns_ten_loaded_files_matching_the_boot_log_source() {
        let dir = TempDataDir::new("p102-sc07-loaded-files");
        seed_all_valid(&dir);
        let loaded = load(&dir.root, 20).expect("실제 data/ 사본 로딩 실패");

        assert_eq!(
            loaded.loaded_files.len(),
            10,
            "data/**/*.json 10개 전부가 목록에 있어야 한다(SC-06/SC-07): {:?}",
            loaded.loaded_files
        );
        // 중복 없음 — 같은 파일이 두 번 읽혔다면 짝 검사가 거짓으로 맞아떨어질 수 있다.
        let unique: std::collections::BTreeSet<&String> = loaded.loaded_files.iter().collect();
        assert_eq!(
            unique.len(),
            10,
            "중복 항목이 있다: {:?}",
            loaded.loaded_files
        );

        for expected in [
            "ships/",
            "world/systems/cradle.json",
            "movement/sync-tuning.json",
            "minerals/ferrosite.json",
            "minerals/glacine.json",
            "minerals/cobaltine.json",
            "minerals/starfall-glass.json",
            "world/deposits/cradle.json",
            "mining/mining-rules.json",
            "history/rules/mineral-discovery.json",
        ] {
            assert!(
                loaded_files_contains(&loaded.loaded_files, expected),
                "{expected} 이 목록에 없다: {:?}",
                loaded.loaded_files
            );
        }
        // 경로 구분자는 '/' 여야 한다(qa 도구가 그대로 대조한다) — Windows 에서도.
        for file in &loaded.loaded_files {
            assert!(!file.contains('\\'), "역슬래시가 섞이면 안 된다: {file}");
        }
    }

    fn loaded_files_contains(loaded: &[String], expected: &str) -> bool {
        loaded.iter().any(|file| file.contains(expected))
    }
}
