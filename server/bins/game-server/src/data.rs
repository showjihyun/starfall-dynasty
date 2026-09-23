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

use starfall_contracts::data::{ShipClassTable, StarSystemTable, SyncTuningTable};

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
        /// serde 오류 메시지(필드·기대값·실제값이 문장에 들어 있다).
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
}

impl GameData {
    /// 스폰 후보 지점 수 (`/debug/stats` 의 "스폰 지점 수" — SC-05).
    #[must_use]
    pub fn spawn_point_count(&self) -> usize {
        self.star_system.spawn.points_m.len()
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

fn read_and_parse<T: serde::de::DeserializeOwned>(file: &Path) -> Result<T, DataError> {
    let raw = fs::read_to_string(file).map_err(|source| DataError::ReadFile {
        file: file.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|error| DataError::Schema {
        file: file.to_path_buf(),
        error: error.to_string(),
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

/// `data_dir` 아래 3종을 전부 로드하고 검산한다. 실패는 전부 기동 거부다(I-38).
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

    Ok(GameData {
        data_dir: data_dir.to_path_buf(),
        ship_classes,
        star_system,
        sync_tuning,
        snapshot_interval_ticks,
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
}
