//! 레지스트리 주도 계약 테스트 9종 (ADR-0002 §3).
//!
//! | ADR 테스트 | 이 파일의 테스트 함수 | 스프린트 계약 |
//! |-----------|---------------------|--------------|
//! | 1 메타스키마 유효 + `$ref` 해석 (오프라인) | `schemas_valid_offline` | SC-12 |
//! | 2 유효 fixture 왕복 + 재검증 | `fixtures_roundtrip` | SC-13 |
//! | 3 반례의 스키마 거부 | `invalid_rejected_by_schema` | SC-14 |
//! | 6 반례의 serde 거부 (운영 경로) | `invalid_serde_matrix` | SC-15 |
//! | 5 `server` 태그 타입의 대응표 존재 | `registry_server_types_mapped` | SC-16 |
//! | 4 레지스트리·스키마·fixture 상수 일치 | `registry_consistency` | SC-17 |
//! | 8 레지스트리 자체 검증 + `$id` 정합 | `registry_file_validates_against_schema`, `schema_ids_match_paths` | SC-17 |
//! | 7 `required` 변이 거부 | `required_field_mutations` | SC-18 |
//! | 9 정수 상한 위반 거부 | `integer_bounds_rejected` | (SC-15 보강) |
//!
//! # 빈 순회는 실패다
//!
//! fixture 를 0건 순회하고 통과하는 테스트는 "검증기가 꺼진 상태"와 구분되지 않는다.
//! 그래서 모든 순회 테스트가 개수를 단언하고 `--nocapture` 로 개수를 출력한다.

// 테스트 코드에서는 unwrap/expect/panic 을 쓴다. 실패 지점을 흐리지 않기 위해서다.
// 운영 코드의 금지 규칙은 server/clippy.toml 과 [workspace.lints] 로 유지된다.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use starfall_contracts::registry::{CONTRACT_TYPES, ContractType};

const SCHEMA_ID_PREFIX: &str = "https://schemas.starfall.invalid/contracts/";

/// 기대하는 개수. 계약이 늘면 이 상수도 함께 올린다 — 조용히 줄어드는 것을 막는 가드다.
///
/// p1-01-ship-movement 가 6타입(`SET_SHIP_CONTROL`·`WORLD_SNAPSHOT`·`SHIP_SPAWNED`·
/// `SHIP_DESPAWNED`·`SHIP_CLASS`·`STAR_SYSTEM`·`SYNC_TUNING` — 7종)을 더했다(스펙 §5.3).
const EXPECTED_SCHEMA_COUNT: usize = 18;
// R4 S-6: `SESSION_CLOSED.close_reason` 에 `SUPERSEDED` 가 추가되며 유효 fixture 가
// `superseded.json` 1건 늘었다(architect 계약 변경, 사용자 결정 5). 26 → 27.
const EXPECTED_VALID_FIXTURES: usize = 27;
const EXPECTED_INVALID_FIXTURES: usize = 34;

// ---------------------------------------------------------------------------
// 경로 해석 — 실패하면 명확히 죽는다
// ---------------------------------------------------------------------------

/// `contracts/registry/types.json` 을 마커로 삼아 레포 루트를 찾는다.
///
/// 고정된 `../../..` 대신 마커 기반으로 올라가는 이유: 크레이트가 한 단계라도 옮겨지면
/// 고정 경로는 조용히 엉뚱한 디렉토리를 가리키고, 그러면 fixture 0건 순회로 "통과"한다.
fn repo_root() -> PathBuf {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dir = start;
    loop {
        if dir.join("contracts/registry/types.json").is_file() {
            return dir.to_path_buf();
        }
        dir = dir.parent().unwrap_or_else(|| {
            panic!(
                "contracts/registry/types.json 을 찾을 수 없다. \
                 {} 에서 위로 올라가며 찾았다. 레포 구조가 바뀌었는가?",
                start.display()
            )
        });
    }
}

fn contracts_dir() -> PathBuf {
    repo_root().join("contracts")
}

fn read_json(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} 을(를) 읽지 못했다: {error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("{} 이(가) 올바른 JSON 이 아니다: {error}", path.display()))
}

/// `contracts/` 아래의 모든 `*.schema.json` 을 경로 순으로 모은다.
fn schema_files() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("{} 을(를) 열지 못했다: {error}", dir.display()));
        for entry in entries {
            let path = entry.expect("디렉토리 항목을 읽지 못했다").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.to_string_lossy().ends_with(".schema.json") {
                out.push(path);
            }
        }
    }

    let mut found = Vec::new();
    walk(&contracts_dir(), &mut found);
    found.sort();
    assert!(
        !found.is_empty(),
        "스키마를 한 건도 찾지 못했다 — 경로 해석이 잘못됐다"
    );
    found
}

/// 레포 상대 경로에서 `$id` 를 만든다 (ADR-0002 §1 의 규칙).
fn expected_schema_id(path: &Path) -> String {
    let relative = path
        .strip_prefix(contracts_dir())
        .expect("스키마가 contracts/ 밖에 있다");
    format!(
        "{SCHEMA_ID_PREFIX}{}",
        relative.to_string_lossy().replace('\\', "/")
    )
}

// ---------------------------------------------------------------------------
// 오프라인 검증기
// ---------------------------------------------------------------------------

/// 모든 스키마를 `$id` 로 등록한 오프라인 레지스트리를 만든다.
///
/// 네트워크·파일 리졸버 feature 를 켜지 않았고(`default-features = false`), 여기에 더해
/// `offline()` 으로 원격 fetch 자체를 거부한다. 미등록 `$ref` 는 조용한 네트워크 접근이
/// 아니라 **빌드 실패**로 드러난다.
fn offline_registry() -> jsonschema::Registry<'static> {
    let mut builder = jsonschema::Registry::new();
    for path in schema_files() {
        let id = expected_schema_id(&path);
        builder = builder
            .add(&id, read_json(&path))
            .unwrap_or_else(|error| panic!("{id} 을(를) 레지스트리에 넣지 못했다: {error}"));
    }
    builder
        .prepare()
        .unwrap_or_else(|error| panic!("스키마 레지스트리를 준비하지 못했다: {error}"))
}

/// 등록된 `$id` 를 가리키는 검증기를 만든다.
///
/// `offline()` 은 원격 fetch 자체를 거부하는 리트리버를 끼운다. 즉 "네트워크로 나가지 않는다"가
/// Cargo feature 설정이 아니라 **검증기 자체**로 보장된다.
fn validator_for(
    registry: &jsonschema::Registry<'static>,
    schema_id: &str,
) -> jsonschema::Validator {
    jsonschema::options()
        .with_registry(registry)
        .offline()
        .build(&json!({ "$ref": schema_id }))
        .unwrap_or_else(|error| panic!("{schema_id} 검증기를 만들지 못했다: {error}"))
}

// ---------------------------------------------------------------------------
// 레지스트리 파일
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RegistryEntry {
    name: String,
    kind: String,
    schema: String,
    schema_version: u64,
    producers: Vec<String>,
    consumers: Vec<String>,
    status: String,
}

fn registry_entries() -> Vec<RegistryEntry> {
    let document = read_json(&contracts_dir().join("registry/types.json"));
    let types = document["types"]
        .as_array()
        .expect("registry/types.json 의 types 가 배열이 아니다");
    let entries: Vec<RegistryEntry> = types
        .iter()
        .map(|entry| RegistryEntry {
            name: entry["name"]
                .as_str()
                .expect("name 이 문자열이 아니다")
                .to_owned(),
            kind: entry["kind"]
                .as_str()
                .expect("kind 가 문자열이 아니다")
                .to_owned(),
            schema: entry["schema"]
                .as_str()
                .expect("schema 가 문자열이 아니다")
                .to_owned(),
            schema_version: entry["schema_version"]
                .as_u64()
                .expect("schema_version 이 정수가 아니다"),
            producers: string_list(&entry["producers"]),
            consumers: string_list(&entry["consumers"]),
            status: entry["status"]
                .as_str()
                .expect("status 가 문자열이 아니다")
                .to_owned(),
        })
        .collect();
    assert!(!entries.is_empty(), "레지스트리가 비어 있다");
    entries
}

fn string_list(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item.as_str().expect("태그가 문자열이 아니다").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// 서버가 다뤄야 하는 타입인가 — `producers` **또는** `consumers` 에 `server` 가 있으면 그렇다.
///
/// 한쪽만 보면 검사 범위가 절반이 된다: `PING_SERVER` 는 consumers 에만,
/// `PING_REPLY` 는 producers 에만 `server` 가 있다.
fn is_server_type(entry: &RegistryEntry) -> bool {
    entry.producers.iter().any(|tag| tag == "server")
        || entry.consumers.iter().any(|tag| tag == "server")
}

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

fn fixture_dir(type_name: &str) -> PathBuf {
    contracts_dir().join("fixtures").join(type_name)
}

/// `{TYPE}/*.json` 만 모은다. `invalid/` 하위 디렉토리는 재귀로 빨아들이지 않는다.
fn valid_fixtures(type_name: &str) -> Vec<PathBuf> {
    let dir = fixture_dir(type_name);
    let mut found: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} 을(를) 열지 못했다: {error}", dir.display()))
        .map(|entry| entry.expect("디렉토리 항목을 읽지 못했다").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "{} 에 유효 fixture 가 없다 — active 타입은 fixture 를 가져야 한다 (I-3)",
        dir.display()
    );
    found
}

fn invalid_fixtures(type_name: &str) -> Vec<PathBuf> {
    let dir = fixture_dir(type_name).join("invalid");
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut found: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} 을(를) 열지 못했다: {error}", dir.display()))
        .map(|entry| entry.expect("디렉토리 항목을 읽지 못했다").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    found.sort();
    found
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------------
// 테스트 1 (SC-12) — 스키마 유효성 + $ref 해석, 네트워크 없이
// ---------------------------------------------------------------------------

#[test]
fn schemas_valid_offline() {
    let files = schema_files();
    let registry = offline_registry();

    for path in &files {
        let document = read_json(path);

        // (a) 2020-12 메타스키마로 유효한가.
        if let Err(error) = jsonschema::meta::validate(&document) {
            panic!(
                "{} 이(가) 메타스키마 검증에 실패했다: {error}",
                path.display()
            );
        }

        // (b) 모든 $ref 가 해석되는가. 검증기 빌드가 참조를 전부 풀어 본다.
        //     미등록 참조가 있으면 여기서 실패한다(네트워크로 나가지 않는다).
        let _ = validator_for(&registry, &expected_schema_id(path));
    }

    println!("[SC-12] 오프라인 검증한 스키마: {}건", files.len());
    for path in &files {
        println!("  - {}", file_name(path));
    }
    assert_eq!(
        files.len(),
        EXPECTED_SCHEMA_COUNT,
        "스키마 개수가 기대와 다르다 — 계약이 바뀌었으면 EXPECTED_SCHEMA_COUNT 를 갱신하라"
    );
}

// ---------------------------------------------------------------------------
// 테스트 2 (SC-13) — 유효 fixture 왕복
// ---------------------------------------------------------------------------

#[test]
fn fixtures_roundtrip() {
    let registry = offline_registry();
    let mut checked = Vec::new();

    for contract in CONTRACT_TYPES {
        let schema_id = format!("{SCHEMA_ID_PREFIX}{}", contract.schema_path);
        let validator = validator_for(&registry, &schema_id);

        for path in valid_fixtures(contract.name) {
            let original = read_json(&path);

            // 역직렬화 -> 재직렬화.
            let reserialized = (contract.round_trip)(&original).unwrap_or_else(|error| {
                panic!(
                    "{} 을(를) {} 로 역직렬화하지 못했다: {error}",
                    path.display(),
                    contract.rust_type
                )
            });

            // 원본과 의미적으로 동일한가. 문자열이 아니라 Value 로 비교한다(필드 순서 무관).
            assert_eq!(
                reserialized,
                original,
                "{} 왕복 결과가 원본과 다르다.\n  기대: {}\n  실제: {}",
                path.display(),
                serde_json::to_string_pretty(&original).unwrap(),
                serde_json::to_string_pretty(&reserialized).unwrap()
            );

            // 재직렬화 결과가 스키마 검증도 통과하는가.
            if let Err(error) = validator.validate(&reserialized) {
                panic!(
                    "{} 의 재직렬화 결과가 스키마 검증에 실패했다: {error}",
                    path.display()
                );
            }

            checked.push(file_name(&path));
        }
    }

    println!("[SC-13] 왕복 검증한 유효 fixture: {}건", checked.len());
    for name in &checked {
        println!("  - {name}");
    }
    assert_eq!(
        checked.len(),
        EXPECTED_VALID_FIXTURES,
        "유효 fixture 개수가 기대와 다르다"
    );
}

// ---------------------------------------------------------------------------
// 테스트 3 (SC-14) — 반례의 스키마 거부 [층①]
// ---------------------------------------------------------------------------

#[test]
fn invalid_rejected_by_schema() {
    let registry = offline_registry();
    let mut checked = Vec::new();

    for contract in CONTRACT_TYPES {
        let schema_id = format!("{SCHEMA_ID_PREFIX}{}", contract.schema_path);
        let validator = validator_for(&registry, &schema_id);

        for path in invalid_fixtures(contract.name) {
            let document = read_json(&path);
            assert!(
                validator.validate(&document).is_err(),
                "{} 이(가) 스키마 검증을 통과했다 — 검증기가 죽어 있거나 계약이 느슨하다 (I-4)",
                path.display()
            );
            checked.push(file_name(&path));
        }
    }

    println!("[SC-14] 스키마가 거부한 반례: {}건", checked.len());
    for name in &checked {
        println!("  - {name}");
    }
    assert_eq!(
        checked.len(),
        EXPECTED_INVALID_FIXTURES,
        "반례 개수가 기대와 다르다"
    );
}

// ---------------------------------------------------------------------------
// 테스트 6 (SC-15) — 반례의 serde 거부 [층②, 운영 경로]
// ---------------------------------------------------------------------------

/// 스펙 §5 / 스프린트 계약 §0.4 의 "Rust serde" 열. **이 표가 계약이다.**
///
/// 구현 결과가 표와 다르면 표를 고치지 말고 architect 에게 알린다.
/// `true` = 거부해야 한다.
const SERDE_REJECTION_TABLE: &[(&str, bool)] = &[
    // p0-01 (PING_SERVER / PING_REPLY) — 7건
    ("actor-field-injected.json", true),
    ("command-id-not-v7.json", true),
    ("probe-seq-negative.json", true),
    ("probe-seq-above-u32.json", true),
    ("missing-tick.json", true),
    ("payload-unknown-field.json", true), // COMMAND_RESULT 의 같은 이름 반례도 이 행이 덮는다
    ("tick-above-safe-integer.json", true),
    // p0-02 신규 4타입 — 9건 (파일 이름이 겹치는 actor-id-null.json 은 두 타입이 공유)
    ("unknown-reason-code.json", true),  // 닫힌 열거형
    ("tick-hz-zero.json", true),         // TickHz 범위 newtype
    ("missing-session-id.json", true),   // 필수 필드
    ("actor-id-null.json", true),        // 좁힘: 비-Option
    ("missing-world-id.json", true),     // 필수 필드
    ("unknown-close-reason.json", true), // 닫힌 열거형
    ("correlation-id-null.json", true),  // 이벤트 envelope 의 correlation_id 는 비-Option
    ("causation-id-null.json", true),    // SHIP_SPAWNED·SHIP_DESPAWNED 좁힘 — 비-Option
    // p1-01-ship-movement 신규 — 18건 (스펙 §5.4)
    ("attitude-field-injected.json", true), // deny_unknown_fields — I-26
    ("position-field-injected.json", true), // deny_unknown_fields — I-26
    ("thrust-above-range.json", true),      // ControlAxisMilli 범위 newtype
    ("input-seq-zero.json", true),          // InputSeq 는 1부터
    ("movement-unknown-field.json", true),  // ShipClassMovement deny_unknown_fields
    ("turn-gain-zero.json", true),          // exclusiveMinimum 0 범위 newtype
    ("session-closed-is-not-a-despawn.json", true), // DespawnReason 닫힌 열거형
    ("hard-radius-above-ceiling.json", true), // STAR_SYSTEM·WORLD_SNAPSHOT 공유 — 경계 상한
    ("no-spawn-points.json", true),         // points_m 은 1개 이상
    ("integrator-is-not-a-tunable.json", true), // SyncTuningPrediction deny_unknown_fields
    ("snapshot-hz-zero.json", true),        // snapshot_hz 는 1부터
    ("angular-velocity-roll-missing.json", true), // ShipState 의 필수 필드
    ("ship-missing-orientation-w.json", true), // ShipState 의 필수 필드
    ("unknown-presence.json", true),        // ShipPresence 닫힌 열거형
];

#[test]
fn invalid_serde_matrix() {
    let mut observed = Vec::new();

    for contract in CONTRACT_TYPES {
        for path in invalid_fixtures(contract.name) {
            let document = read_json(&path);
            let name = file_name(&path);

            let expect_rejected = SERDE_REJECTION_TABLE
                .iter()
                .find(|(fixture, _)| *fixture == name)
                .map(|(_, rejected)| *rejected)
                .unwrap_or_else(|| {
                    panic!(
                        "{name} 이(가) serde 기대 결과표에 없다. \
                         반례를 추가했으면 SERDE_REJECTION_TABLE 과 스펙 §5 표를 함께 갱신해야 한다"
                    )
                });

            let result = (contract.round_trip)(&document);
            let rejected = result.is_err();

            println!(
                "  {name:<32} 기대={:<6} 실제={:<6} {}",
                if expect_rejected { "거부" } else { "통과" },
                if rejected { "거부" } else { "통과" },
                result.as_ref().err().map_or(String::new(), |error| {
                    format!("({})", error.lines().next().unwrap_or(""))
                })
            );

            assert_eq!(
                rejected, expect_rejected,
                "{name} 의 serde 거부 결과가 스펙 §5 표와 다르다. \
                 통과시켰다면 운영 경로에 구멍이 있다는 뜻이다 \
                 (#[serde(flatten)] 이나 내부 태그 열거형을 쓰지 않았는지 확인하라)."
            );

            observed.push(name);
        }
    }

    println!("[SC-15] serde 매트릭스 검사한 반례: {}건", observed.len());
    assert_eq!(observed.len(), EXPECTED_INVALID_FIXTURES);
}

// ---------------------------------------------------------------------------
// 테스트 5 (SC-16) — server 태그 타입의 대응표 존재
// ---------------------------------------------------------------------------

#[test]
fn registry_server_types_mapped() {
    let entries = registry_entries();
    let mapped: BTreeSet<&str> = CONTRACT_TYPES.iter().map(|entry| entry.name).collect();

    // (a) server 태그가 있는 타입은 전부 대응표에 있어야 한다.
    //     history 태그는 검사하지 않는다 — starfall-history 크레이트가 아직 없다 (ADR-0002 §3).
    let mut server_types = Vec::new();
    for entry in &entries {
        if is_server_type(entry) {
            assert!(
                mapped.contains(entry.name.as_str()),
                "{} 에 server 태그가 있는데 Rust 대응표에 없다 \
                 (registry.rs 의 CONTRACT_TYPES 에 추가하라)",
                entry.name
            );
            server_types.push(entry.name.clone());
        }
    }

    // (b) fixture 디렉토리인데 레지스트리에 없는 타입이 있으면 실패.
    let registered: BTreeSet<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let fixtures_root = contracts_dir().join("fixtures");
    for entry in fs::read_dir(&fixtures_root).expect("fixtures 디렉토리를 열지 못했다") {
        let path = entry.expect("디렉토리 항목을 읽지 못했다").path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = file_name(&path);
        assert!(
            registered.contains(dir_name.as_str()),
            "fixtures/{dir_name} 디렉토리가 있는데 레지스트리에 그 타입이 없다"
        );
    }

    println!(
        "[SC-16] server 태그 타입 {}건이 대응표에 있다",
        server_types.len()
    );
    for name in &server_types {
        let contract: &ContractType =
            starfall_contracts::registry::find(name).expect("대응표 조회 실패");
        println!("  - {name} -> {}", contract.rust_type);
    }
    assert!(!server_types.is_empty(), "server 태그 타입이 하나도 없다");
}

// ---------------------------------------------------------------------------
// 테스트 4 (SC-17) — 레지스트리·스키마 상수·fixture 일치
// ---------------------------------------------------------------------------

#[test]
fn registry_consistency() {
    let entries = registry_entries();
    let mut checked = 0usize;

    for entry in &entries {
        let schema_path = contracts_dir().join(&entry.schema);
        assert!(
            schema_path.is_file(),
            "{} 의 schema 경로가 가리키는 파일이 없다: {}",
            entry.name,
            schema_path.display()
        );

        let schema = read_json(&schema_path);

        // `data` kind 는 envelope 이 없다 — 타입 태그 필드도, 고정된
        // `schema_version` const 도 없다(파일마다 있는 평범한 정수 필드다). 스펙 §5.1.
        let type_key = match entry.kind.as_str() {
            "command" => Some("command_type"),
            "server_message" => Some("message_type"),
            "domain_event" | "historical_event" => Some("event_type"),
            "data" => None,
            other => panic!("{}: 알 수 없는 kind {other}", entry.name),
        };

        if let Some(type_key) = type_key {
            let schema_type_const = schema["properties"][type_key]["const"]
                .as_str()
                .unwrap_or_else(|| {
                    panic!(
                        "{}: 스키마 최상위 properties.{type_key}.const 가 없다",
                        entry.name
                    )
                });
            assert_eq!(
                schema_type_const, entry.name,
                "{}: 레지스트리 이름과 스키마 타입 상수가 다르다",
                entry.name
            );

            let schema_version_const = schema["properties"]["schema_version"]["const"]
                .as_u64()
                .unwrap_or_else(|| panic!("{}: 스키마에 schema_version const 가 없다", entry.name));
            assert_eq!(
                schema_version_const, entry.schema_version,
                "{}: 레지스트리 schema_version 과 스키마 const 가 다르다",
                entry.name
            );
        }

        // active 타입은 유효 fixture 를 가져야 한다 (I-3), 그리고 fixture 의 값도 일치해야 한다.
        if entry.status == "active" {
            for path in valid_fixtures(&entry.name) {
                let fixture = read_json(&path);
                if let Some(type_key) = type_key {
                    assert_eq!(
                        fixture[type_key].as_str(),
                        Some(entry.name.as_str()),
                        "{}: fixture 의 {type_key} 가 레지스트리 이름과 다르다",
                        path.display()
                    );
                }
                assert_eq!(
                    fixture["schema_version"].as_u64(),
                    Some(entry.schema_version),
                    "{}: fixture 의 schema_version 이 레지스트리와 다르다",
                    path.display()
                );
                checked += 1;
            }
        }

        // 대응표의 메타데이터도 레지스트리와 같아야 한다.
        if let Some(contract) = starfall_contracts::registry::find(&entry.name) {
            assert_eq!(contract.kind, entry.kind, "{}: kind 불일치", entry.name);
            assert_eq!(
                contract.schema_path, entry.schema,
                "{}: schema 경로 불일치",
                entry.name
            );
            assert_eq!(
                u64::from(contract.schema_version),
                entry.schema_version,
                "{}: schema_version 불일치",
                entry.name
            );
        }
    }

    println!(
        "[SC-17] 레지스트리 항목 {}건, 대조한 fixture {checked}건",
        entries.len()
    );
    assert_eq!(checked, EXPECTED_VALID_FIXTURES);
}

// ---------------------------------------------------------------------------
// 테스트 8 (SC-17) — 레지스트리 자체 검증 + $id 정합
// ---------------------------------------------------------------------------

#[test]
fn registry_file_validates_against_schema() {
    // registry/types.json 은 스키마가 아니라 데이터 파일이라, 다른 테스트가 보지 않는다.
    // architect 가 만든 types.schema.json 이 실제로 쓰이는 유일한 지점이다.
    let registry = offline_registry();
    let validator = validator_for(
        &registry,
        &format!("{SCHEMA_ID_PREFIX}registry/types.schema.json"),
    );
    let document = read_json(&contracts_dir().join("registry/types.json"));

    if let Err(error) = validator.validate(&document) {
        panic!("registry/types.json 이 자기 스키마 검증에 실패했다: {error}");
    }
    println!("[SC-17] registry/types.json 이 types.schema.json 검증을 통과했다");
}

#[test]
fn schema_ids_match_paths() {
    // $id 와 파일 경로가 어긋나면 오프라인 등록은 성공하고 $ref 는 조용히
    // 엉뚱한 문서를 가리킨다. 그 사고는 런타임에야 드러난다.
    let files = schema_files();
    for path in &files {
        let document = read_json(path);
        let declared = document["$id"]
            .as_str()
            .unwrap_or_else(|| panic!("{} 에 $id 가 없다", path.display()));
        assert_eq!(
            declared,
            expected_schema_id(path),
            "{} 의 $id 가 경로 규칙과 다르다",
            path.display()
        );
    }
    println!(
        "[SC-17] $id 가 경로 규칙과 일치하는 스키마: {}건",
        files.len()
    );
    assert_eq!(files.len(), EXPECTED_SCHEMA_COUNT);
}

// ---------------------------------------------------------------------------
// 테스트 7 (SC-18) — required 변이
// ---------------------------------------------------------------------------

/// 타입 스키마가 `allOf` 로 합치는 envelope 의 `required` 목록.
fn envelope_required(type_schema: &Value, type_schema_path: &Path) -> Vec<String> {
    let reference = type_schema["allOf"][0]["$ref"]
        .as_str()
        .expect("타입 스키마의 allOf[0].$ref 가 없다");
    let envelope_path = type_schema_path
        .parent()
        .expect("타입 스키마에 부모 디렉토리가 없다")
        .join(reference);
    let envelope = read_json(&envelope_path);
    string_list(&envelope["required"])
}

/// payload 스키마의 `required` 목록.
///
/// **`$defs` 를 "첫 항목"으로 집지 않는다.** `WORLD_SNAPSHOT` 처럼 배열 원소 타입(`ShipState`)
/// 을 위한 두 번째 `$defs` 항목이 생기면, `serde_json::Value` 의 기본 객체 표현(`BTreeMap`,
/// `preserve_order` feature 없음)은 **키를 알파벳순으로 정렬**하므로 `ShipState` 가
/// `WorldSnapshotPayload` 보다 먼저 온다 — "첫 항목"이 조용히 엉뚱한 타입이 된다. 그래서
/// `properties.payload.$ref` 가 가리키는 이름을 따라간다(그것이 실제로 payload 인 것의
/// 유일한 근거다).
fn payload_required(type_schema: &Value) -> Vec<String> {
    let payload_ref = type_schema["properties"]["payload"]["$ref"]
        .as_str()
        .expect("타입 스키마의 properties.payload.$ref 가 없다");
    let def_name = payload_ref
        .rsplit('/')
        .next()
        .expect("payload $ref 형식이 예상과 다르다");
    let defs = type_schema["$defs"]
        .as_object()
        .expect("타입 스키마에 $defs 가 없다");
    let payload = defs
        .get(def_name)
        .unwrap_or_else(|| panic!("$defs 에 {def_name} 이(가) 없다"));
    string_list(&payload["required"])
}

#[test]
fn required_field_mutations() {
    let mut mutations = 0usize;

    for contract in CONTRACT_TYPES {
        // `data` kind 에는 envelope 도 단일 `payload` 블록도 없다 — 스키마 최상위 자체가
        // "payload"다(스펙 §5.1). 그 모양에 맞는 required 변이는 각 타입의 전용 단위
        // 테스트(`data.rs` 의 `*_rejects_*`)가 덮는다. 여기서 일반화하면
        // `allOf`/`$defs` 를 읽다가 패닉한다.
        if contract.kind == "data" {
            continue;
        }

        let schema_path = contracts_dir().join(contract.schema_path);
        let schema = read_json(&schema_path);
        let top_level = envelope_required(&schema, &schema_path);
        let payload_fields = payload_required(&schema);

        for path in valid_fixtures(contract.name) {
            let original = read_json(&path);

            // (a) envelope 필수 필드를 하나씩 뺀다.
            for field in &top_level {
                let mut mutated = original.clone();
                let object: &mut Map<String, Value> =
                    mutated.as_object_mut().expect("fixture 가 객체가 아니다");
                assert!(
                    object.remove(field).is_some(),
                    "{}: 필수 필드 {field} 가 fixture 에 없다 (I-5 위반 가능성)",
                    path.display()
                );
                assert!(
                    (contract.round_trip)(&mutated).is_err(),
                    "{} 에서 필수 필드 {field} 를 빼도 역직렬화가 통과했다. \
                     Rust 쪽이 Option 으로 선언됐거나, Option 필드에 \
                     deserialize_with 가 빠져 serde 가 조용히 None 을 채운 것이다.",
                    path.display()
                );
                mutations += 1;
            }

            // (b) payload 필수 필드를 하나씩 뺀다.
            for field in &payload_fields {
                let mut mutated = original.clone();
                let payload = mutated["payload"]
                    .as_object_mut()
                    .expect("payload 가 객체가 아니다");
                assert!(
                    payload.remove(field).is_some(),
                    "{}: payload 필수 필드 {field} 가 fixture 에 없다",
                    path.display()
                );
                assert!(
                    (contract.round_trip)(&mutated).is_err(),
                    "{} 에서 payload 필수 필드 {field} 를 빼도 역직렬화가 통과했다",
                    path.display()
                );
                mutations += 1;
            }
        }
    }

    println!("[SC-18] required 변이 {mutations}건이 모두 역직렬화에 실패했다");
    assert!(mutations >= 20, "변이 개수가 너무 적다: {mutations}");
}

// ---------------------------------------------------------------------------
// 테스트 9 — 정수 상한 위반
// ---------------------------------------------------------------------------

#[test]
fn integer_bounds_rejected() {
    // 반례 fixture 2건이 이미 이것을 덮지만(SC-15), 경계값을 코드에 직접 박아 두면
    // fixture 가 사라지거나 바뀌어도 이 성질이 지켜진다.
    // MoneyMinor 가 들어오는 순간 같은 실수가 곧바로 화폐 버그가 된다.
    let base_reply = read_json(&contracts_dir().join("fixtures/PING_REPLY/basic.json"));
    let ping_reply = starfall_contracts::registry::find("PING_REPLY").unwrap();

    let mut at_limit = base_reply.clone();
    at_limit["tick"] = json!(9_007_199_254_740_991u64);
    assert!(
        (ping_reply.round_trip)(&at_limit).is_ok(),
        "tick = 2^53-1 은 허용되어야 한다"
    );

    let mut above_limit = base_reply.clone();
    above_limit["tick"] = json!(9_007_199_254_740_992u64);
    assert!(
        (ping_reply.round_trip)(&above_limit).is_err(),
        "tick = 2^53 이 통과했다 — Tick 이 범위 검증 newtype 이 아니다"
    );

    let base_command = read_json(&contracts_dir().join("fixtures/PING_SERVER/basic.json"));
    let ping_server = starfall_contracts::registry::find("PING_SERVER").unwrap();

    for (value, should_pass) in [
        (json!(4_294_967_295u64), true),
        (json!(4_294_967_296u64), false),
        (json!(-1), false),
    ] {
        let mut mutated = base_command.clone();
        mutated["payload"]["probe_seq"] = value.clone();
        assert_eq!(
            (ping_server.round_trip)(&mutated).is_ok(),
            should_pass,
            "probe_seq = {value} 의 처리 결과가 기대와 다르다"
        );
    }

    println!("[테스트 9] 정수 경계 5건 확인 (tick 2건, probe_seq 3건)");

    // 정수/소수 구분 (QA 04_qa_report_r1.md §6.1 — SC-40(f) 는 PASS 지만 권고 1건).
    // 지금까지 `10.0` 이 거부되는 것은 (a) `fixtures_roundtrip` 의 엄격 `Value` 비교와
    // (b) `tests/e2e/interface_matrix.py` 의 정적 타입 대조에만 기대고 있었다 — 둘 다
    // qa 소유 도구라 이 스위트가 스스로 지키지 못했다. 여기서 직접 단언한다: integer
    // 로 선언된 필드에 **같은 값**을 소수 형태로 넣으면 역직렬화가 실패해야 한다.
    let mut fractional_form_checks = 0usize;

    let mut tick_as_float = base_reply.clone();
    tick_as_float["tick"] = json!(18_273_391.0);
    assert!(
        (ping_reply.round_trip)(&tick_as_float).is_err(),
        "tick = 18273391.0 (정수와 같은 값의 소수 형태) 이 통과했다 — 정수/소수 구분이 없다"
    );
    fractional_form_checks += 1;

    let mut probe_seq_as_float = base_command.clone();
    probe_seq_as_float["payload"]["probe_seq"] = json!(42.0);
    assert!(
        (ping_server.round_trip)(&probe_seq_as_float).is_err(),
        "probe_seq = 42.0 (정수와 같은 값의 소수 형태) 이 통과했다 — 정수/소수 구분이 없다"
    );
    fractional_form_checks += 1;

    println!(
        "[테스트 9] 정수 선언 필드의 소수 형태(10 -> 10.0) 거부 {fractional_form_checks}건 확인"
    );
    // architect 조건 — §5.3/AC-9와 같은 가드: 검사 건수가 0이면 그 자체로 실패시킨다.
    // (지금은 하드코딩된 두 건이라 "훑기가 조용히 깨진다"는 이 형태로는 못 벌어지지만,
    // 가드가 있으면 이후 필드를 스캔 방식으로 늘려도 같은 실수를 잡는다.)
    assert!(
        fractional_form_checks > 0,
        "정수 소수 형태 검사가 0건이다 — 아무것도 검사하지 않고 조용히 통과했을 수 있다"
    );
}
