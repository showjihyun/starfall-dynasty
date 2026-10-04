//! J. 역사 판정 코어 (AC-10) — `02_sprint_contract.md` §J, SC-41~SC-50 (SC-47 제외 — DB).
//!
//! DB 없이, `starfall-history` 만으로 도는 테스트다. 입력은 계약 fixture
//! (`contracts/fixtures/MINERAL_MINED/*.json`)를 S1 이 만든 계약 타입
//! (`starfall_contracts::events::MineralMinedEvent`)으로 직접 역직렬화한다.
//!
//! **규칙 값의 원천은 파일 하나다**(원칙 9): 모든 테스트가 [`discovery_rule`] 을 거쳐
//! `data/history/rules/mineral-discovery.json` 을 읽는다. 하드코딩된 두 번째 사본을 두지
//! 않는다 — qa 가 지적한 대로, 코드에 값을 복사해 두면 golden 도 다른 테스트도 그 복사본이
//! 파일과 갈라지는 것을 잡지 못한다.
#![cfg(test)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use starfall_contracts::data::SignificanceRuleTable;
use starfall_contracts::events::MineralMinedEvent;
use starfall_contracts::historical::{HistoricalEntityKind, HistoricalRole, HistoricalVisibility};
use starfall_contracts::primitives::{Sequence, Tick, UuidV7};
use starfall_history::{DomainEvent, HistoryCore, MineralDiscoveredRecord};

// ---------------------------------------------------------------------------
// fixture·규칙 파일 로딩 — 테스트 전용 IO (코어 자신은 파일을 읽지 않는다)
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_text(path: PathBuf) -> String {
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 읽기 실패: {e}", path.display()))
}

fn mineral_mined_fixture(name: &str) -> MineralMinedEvent {
    let path = repo_root().join(format!("contracts/fixtures/MINERAL_MINED/{name}.json"));
    let text = read_text(path.clone());
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} 파싱 실패: {e}", path.display()))
}

fn mineral_discovered_fixture(
    name: &str,
) -> starfall_contracts::historical::MineralDiscoveredEvent {
    let path = repo_root().join(format!("contracts/fixtures/MINERAL_DISCOVERED/{name}.json"));
    let text = read_text(path.clone());
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} 파싱 실패: {e}", path.display()))
}

fn discovery_rule_path() -> PathBuf {
    repo_root().join("data/history/rules/mineral-discovery.json")
}

/// 임의 경로에서 규칙 값을 읽는다 — 실제 규칙 파일이든, [`write_temp_rule_copy`] 가 만든
/// 임시 사본이든 같은 경로로 다룬다.
fn discovery_rule_from_path(path: &Path) -> SignificanceRuleTable {
    let text = read_text(path.to_path_buf());
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} 파싱 실패: {e}", path.display()))
}

/// `data/history/rules/mineral-discovery.json` — **유일한 규칙 값 원천**. 모든 테스트가
/// 이 함수를 거친다(하드코딩 사본 없음).
fn discovery_rule() -> SignificanceRuleTable {
    discovery_rule_from_path(&discovery_rule_path())
}

/// 변이 시연·회귀 확인용: 규칙 값을 메모리에서 바꾼 뒤 **추적되지 않는 임시 파일**에 써서
/// [`discovery_rule_from_path`] 로 되읽을 수 있게 한다.
///
/// 팀장 지시(2026-09-27): 공유 추적 파일(`data/`, `contracts/`)을 변이 시연으로 직접
/// 건드리지 않는다 — 그 사이 client 의 SC-72(`ClientDataCopy_MatchesRepositoryOriginal`)나
/// qa 의 `validate_data_files` 가 돌면 거짓 빨간불이 나고, 원복을 잊으면 증거가 오염된다.
/// 반환한 경로는 호출자가 지운다(`fs::remove_file` — 실패해도 무해, OS 임시 디렉터리다).
fn write_temp_rule_copy(rule: &SignificanceRuleTable) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "starfall-history-mutation-{}-{n}.json",
        std::process::id()
    ));
    let text = serde_json::to_string_pretty(rule).expect("규칙 값 직렬화 실패");
    fs::write(&path, text)
        .unwrap_or_else(|e| panic!("임시 규칙 파일 쓰기 실패({}): {e}", path.display()));
    path
}

fn uuid_v7(raw: &str) -> UuidV7 {
    UuidV7::parse(raw).unwrap_or_else(|| panic!("UuidV7 로 파싱 실패: {raw}"))
}

/// [`write_temp_rule_copy`] 자체를 증명한다 — 실제 규칙을 임시 파일에 써서 되읽은 값이
/// 원본과 같아야 한다(직렬화 왕복 확인). 이 성질이 없으면 아래 "임시 사본 변이" 시연이
/// 무엇을 재는지 알 수 없다.
#[test]
fn temp_rule_copy_round_trips() {
    let original = discovery_rule();
    let path = write_temp_rule_copy(&original);
    let round_tripped = discovery_rule_from_path(&path);
    let _ = fs::remove_file(&path);
    assert_eq!(
        original, round_tripped,
        "임시 사본을 거친 규칙 값이 원본과 달라졌다"
    );
}

/// 팀장 지시(2026-09-27) 이후의 변이 시연 — **추적 파일을 건드리지 않는다.** 메모리에서
/// `evidence.visibility` 를 바꾼 사본을 임시 파일에 쓰고, 그 경로를 주입해 읽은 결과로
/// SC-41 이 실제로 실패하는지 본다(이전 §9 의 시연은 `data/history/rules/
/// mineral-discovery.json` 을 직접 고쳤다가 원복했는데, 그 사이 다른 에이전트의 검증이
/// 돌면 거짓 빨간불을 만들 수 있다는 지적을 받았다).
#[test]
fn sc41_fails_when_evidence_visibility_mutates_in_a_temp_copy() {
    use starfall_contracts::historical::HistoricalVisibility;

    let mut mutated = discovery_rule();
    assert_eq!(
        mutated.evidence.visibility,
        HistoricalVisibility::ParticipantsOnly,
        "원본 규칙의 evidence.visibility 가 이미 CLASSIFIED 라면 이 테스트는 아무것도 안 잰다"
    );
    mutated.evidence.visibility = HistoricalVisibility::Classified;

    let temp_path = write_temp_rule_copy(&mutated);
    let reloaded = discovery_rule_from_path(&temp_path);
    let _ = fs::remove_file(&temp_path);

    let mut core = HistoryCore::new(reloaded);
    let event = DomainEvent::mineral_mined(mineral_mined_fixture("first-extraction"));
    let records = core
        .judge(&event)
        .expect("정렬된 단일 입력은 역행이 아니다");
    assert_eq!(records.len(), 1);

    assert_eq!(
        records[0].evidence.visibility,
        HistoricalVisibility::Classified,
        "임시 사본의 값(CLASSIFIED)이 실제로 판정 결과에 반영돼야 한다 — \
         SC-41 이 `discovery_rule()` 대신 이 임시 규칙으로 돌았다면 이 assert 가 실패해\
         '규칙 값의 원천은 파일 하나' 라는 성질이 실제로 성립함을 보인다"
    );
}

/// 이 규칙이 판정하지 않는 타입(Level 0) — `SESSION_*`·`SHIP_*` 등. payload 는 판정에
/// 안 쓰이므로 좌표만 있으면 된다.
fn unjudged_event(world_id: &str, event_id: &str, tick: u64, sequence: u64) -> DomainEvent {
    DomainEvent::unjudged(
        uuid_v7(event_id),
        uuid_v7(world_id),
        Tick::new(tick).expect("테스트 tick 은 항상 범위 안"),
        Sequence::new(sequence).expect("테스트 sequence 는 항상 범위 안"),
    )
}

// ---------------------------------------------------------------------------
// SC-41 — 첫 채굴 → 기록 1건 + 증거 1건 (AC-10(a), 이 절 전체의 양성 대조)
// ---------------------------------------------------------------------------

#[test]
fn first_extraction_discovers() {
    let mined = mineral_mined_fixture("first-extraction");
    let event_id = mined.event_id;
    let actor_id = mined.actor_id;
    let ship_id = mined.payload.ship_id;

    let mut core = HistoryCore::new(discovery_rule());
    let records = core
        .judge(&DomainEvent::mineral_mined(mined))
        .expect("정렬된 단일 입력은 역행이 아니다");

    // ⊘ 배제: "판정기가 아무것도 안 냄" — 기록 수를 ≥ 0 이 아니라 정확히 1로 잰다.
    assert_eq!(records.len(), 1, "첫 채굴은 정확히 1건의 발견을 낸다");
    let record = &records[0];

    // 기대 id 는 fixture 에서 읽는다(코드로 계산하지 않는다 — 자기 일치 배제).
    let expected = mineral_discovered_fixture("starfall-glass");
    assert_eq!(
        record.event.historical_event_id,
        expected.historical_event_id
    );

    assert_eq!(record.event.importance_level, 2);
    assert_eq!(record.event.rule_version.as_str(), "mineral-discovery@1");
    assert_eq!(record.event.visibility, HistoricalVisibility::Public);
    assert_eq!(
        record.event.source_event_ids,
        vec![event_id],
        "근거 = 그 이벤트 하나"
    );

    // 참가자 — actor(PLAYER/DISCOVERER) 다음 ship(SHIP/VESSEL), 이 순서로.
    assert_eq!(
        record.event.participants[0].entity_kind,
        HistoricalEntityKind::Player
    );
    assert_eq!(
        record.event.participants[0].role,
        HistoricalRole::Discoverer
    );
    assert_eq!(record.event.participants[0].entity_id, actor_id);
    assert_eq!(
        record.event.participants[1].entity_kind,
        HistoricalEntityKind::Ship
    );
    assert_eq!(record.event.participants[1].role, HistoricalRole::Vessel);
    assert_eq!(record.event.participants[1].entity_id, ship_id);

    // 증거 1건: evidence_id 는 history 가 Python uuid5 로 독립 계산한 값
    // (`tests/data/first-extraction-evidence-id.txt`) — 코드와 같은 함수로 재계산하지 않는다.
    let expected_evidence_id = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/first-extraction-evidence-id.txt"),
    )
    .expect("독립 계산한 evidence_id 데이터 파일이 있어야 한다");
    assert_eq!(
        record.evidence.evidence_id.to_string(),
        expected_evidence_id.trim()
    );
    assert_eq!(
        record.evidence.evidence_type,
        starfall_contracts::data::EvidenceType::ShipLog
    );
    assert_eq!(
        record.evidence.visibility,
        HistoricalVisibility::ParticipantsOnly
    );
}

// ---------------------------------------------------------------------------
// SC-42 — 같은 광물 두 번째 채굴 → 0건 (AC-10(b))
// ---------------------------------------------------------------------------

#[test]
fn second_extraction_is_silent() {
    let mut core = HistoryCore::new(discovery_rule());

    let first = mineral_mined_fixture("first-extraction");
    let first_records = core
        .judge(&DomainEvent::mineral_mined(first.clone()))
        .expect("첫 채굴은 역행이 아니다");
    // ⊘ 배제: "첫 채굴부터 0건" — 먼저 1건임을 단언한다.
    assert_eq!(first_records.len(), 1, "첫 채굴은 1건을 낸다");

    // 같은 (world, star_system, mineral) 의 두 번째 채굴 — 실제 재추출 사본을 만든다.
    let mut second = first;
    second.event_id = uuid_v7("01a0b1c2-a003-7e03-9f13-707384950617");
    second.tick = Tick::new(24001).unwrap();
    second.sequence = Sequence::new(0).unwrap();

    let second_records = core
        .judge(&DomainEvent::mineral_mined(second))
        .expect("두 번째 채굴도 역행이 아니다");
    println!(
        "SC-42: 처리한 MINERAL_MINED 수 = 2 (분모), 두 번째 결과 = {} 건",
        second_records.len()
    );
    assert_eq!(
        second_records.len(),
        0,
        "같은 광물의 재추출은 역사가 아니다"
    );
}

// ---------------------------------------------------------------------------
// SC-43 — 다른 광물 → 1건 추가(총 2) (AC-10(c))
// ---------------------------------------------------------------------------

#[test]
fn other_mineral_discovers() {
    let mut core = HistoryCore::new(discovery_rule());

    let starfall_glass = mineral_mined_fixture("first-extraction");
    let first_records = core
        .judge(&DomainEvent::mineral_mined(starfall_glass))
        .expect("starfall-glass 첫 채굴은 역행이 아니다");
    assert_eq!(first_records.len(), 1);

    // 같은 성계, 다른 광물(glacine) — 실측 fixture 그대로.
    let glacine = mineral_mined_fixture("partial-last-kg-depletes");
    let second_records = core
        .judge(&DomainEvent::mineral_mined(glacine))
        .expect("glacine 첫 채굴도 역행이 아니다");

    // 변함을 단언한다: 1 -> 2.
    assert_eq!(second_records.len(), 1, "다른 광물의 첫 채굴은 새 발견 1건");
    assert_eq!(
        second_records[0].event.payload.mineral_id.as_str(),
        "glacine",
        "새 발견은 glacine 이어야 한다"
    );
}

// ---------------------------------------------------------------------------
// SC-44 — 같은 tick, sequence 작은 쪽이 단독 발견자 (AC-10(d), H-04)
// ---------------------------------------------------------------------------

#[test]
fn same_tick_lower_sequence_wins() {
    // 승자: 실측 fixture(tick=91234, sequence=3).
    let winner = mineral_mined_fixture("partial-last-kg-depletes");

    // 패자: 같은 tick, 더 큰 sequence(9). event_id 는 승자보다 **사전순으로 앞선다** —
    // event_id 순서를 따랐다면 패자가 이겼을 것이므로, 이 값이 어긋나야 규칙을 구분한다.
    let mut loser = winner.clone();
    loser.event_id = uuid_v7("01a0b1c2-9002-7f03-8a13-717283940516");
    loser.sequence = Sequence::new(9).unwrap();
    loser.actor_id = uuid_v7("01a0b1c2-2c03-7c47-9d69-abcdef012345");
    loser.payload.ship_id = uuid_v7("01a0b1c2-8b03-7b13-9c34-bd45e66f7708");

    // ⊘ 배제 입력 단계 단언: 두 이벤트의 tick 이 같고, event_id 순서 ≠ sequence 순서.
    assert_eq!(
        winner.tick, loser.tick,
        "같은 tick 이어야 이 테스트가 의미 있다"
    );
    assert!(
        loser.event_id.to_string() < winner.event_id.to_string(),
        "event_id 순서가 sequence 순서와 같으면 이 테스트는 어느 규칙인지 구분하지 못한다"
    );
    assert!(
        winner.sequence.get() < loser.sequence.get(),
        "승자가 더 작은 sequence 를 가져야 한다"
    );

    let winner_event_id = winner.event_id;
    let winner_actor_id = winner.actor_id;

    // 호출자(러너)는 (tick, sequence) 오름차순으로 넘긴다 — 승자가 먼저.
    let mut ordered_source = vec![winner, loser];
    ordered_source.sort_by_key(|e| (e.tick, e.sequence));
    assert_eq!(
        ordered_source[0].event_id, winner_event_id,
        "정렬 후 승자가 먼저 와야 한다"
    );
    let ordered: Vec<DomainEvent> = ordered_source
        .into_iter()
        .map(DomainEvent::mineral_mined)
        .collect();

    let mut core = HistoryCore::new(discovery_rule());
    let records = core
        .judge_all(&ordered)
        .expect("정렬된 입력은 역행이 아니다");

    assert_eq!(records.len(), 1, "같은 tick 의 발견은 단 하나만 기록된다");
    let expected = mineral_discovered_fixture("glacine-seq-nonzero");
    assert_eq!(
        records[0].event.historical_event_id,
        expected.historical_event_id
    );
    assert_eq!(
        records[0].event.source_event_ids,
        vec![winner_event_id],
        "근거는 sequence 가 작은 승자의 이벤트여야 한다"
    );
    assert_eq!(records[0].event.participants[0].entity_id, winner_actor_id);
}

// ---------------------------------------------------------------------------
// SC-45 — 멱등: 같은 입력을 k 회 처리 → 1 회와 같은 결과 (AC-10(e), H-01)
// ---------------------------------------------------------------------------

#[test]
fn reprocessing_is_idempotent() {
    let mut baseline: Option<Vec<MineralDiscoveredRecord>> = None;
    for k in [1usize, 2, 3, 5] {
        // 매 실행마다 빈 판정 상태에서 새로 시작한다 — "재처리" 는 이 사실의 전체 재생을
        // 여러 번 되풀이해도 결과가 같다는 뜻이다(재구축·재배달의 결정성).
        let mut core = HistoryCore::new(discovery_rule());
        let event = DomainEvent::mineral_mined(mineral_mined_fixture("first-extraction"));
        let records = core
            .judge(&event)
            .expect("정렬된 단일 입력은 역행이 아니다");

        if k == 1 {
            // ⊘ 배제: "첫 처리부터 0건" — 먼저 1건임을 단언한다.
            assert_eq!(records.len(), 1);
            baseline = Some(records);
        } else {
            assert_eq!(
                Some(&records),
                baseline.as_ref(),
                "{k} 회째 재처리 결과가 1회와 달라졌다"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SC-46 — 배치 경계 불변: 정렬된 입력을 자른 지점에 관계없이 같은 결과 (AC-10(f))
// ---------------------------------------------------------------------------

#[test]
fn batch_boundary_invariance() {
    let mut events = vec![
        DomainEvent::mineral_mined(mineral_mined_fixture("first-extraction")), // 발견 #1 (idx 0)
        unjudged_event(
            "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
            "01a0b1c2-b001-7a01-8b11-505162738496",
            24000,
            1,
        ),
        DomainEvent::mineral_mined(mineral_mined_fixture("partial-last-kg-depletes")), // 발견 #2 (idx 2)
        unjudged_event(
            "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
            "01a0b1c2-b002-7a02-8b12-505162738497",
            91235,
            0,
        ),
    ];
    events.sort_by_key(|e| (e.tick, e.sequence));

    let full = {
        let mut core = HistoryCore::new(discovery_rule());
        core.judge_all(&events).expect("전체 배치는 역행이 아니다")
    };
    assert_eq!(full.len(), 2, "발견 이벤트가 2건 섞여 있다");

    // 전수 열거(공간이 5개 뿐이라 무작위 표본보다 강한 성질) + 발견 이벤트 바로 앞·뒤 절단을
    // 명시적으로 표시한다. "시드" 대신 열거한 절단점 전부를 증거로 찍는다.
    let discovery_indices: HashSet<usize> = [0usize, 2].into_iter().collect();
    let mut boundary_cuts_seen = 0usize;
    for cut in 0..=events.len() {
        if discovery_indices.contains(&cut) || discovery_indices.contains(&cut.wrapping_sub(1)) {
            boundary_cuts_seen += 1;
        }

        let mut core = HistoryCore::new(discovery_rule());
        let mut got = core
            .judge_all(&events[..cut])
            .unwrap_or_else(|e| panic!("앞 배치(0..{cut}) 판정 실패: {e:?}"));
        got.extend(
            core.judge_all(&events[cut..])
                .unwrap_or_else(|e| panic!("뒤 배치({cut}..) 판정 실패: {e:?}")),
        );

        assert_eq!(
            got, full,
            "절단점 {cut} 에서 결과가 전체 배치 처리와 달라졌다"
        );
    }
    println!("SC-46: 절단점 0..=4 전수 열거, 발견 인접 절단 {boundary_cuts_seen}건 포함");
    assert!(
        boundary_cuts_seen >= 2,
        "발견 이벤트 바로 앞·뒤 절단이 각각 최소 1건씩 있어야 한다"
    );
}

// ---------------------------------------------------------------------------
// SC-48 — 역행 입력은 거부된다 (AC-10(g))
// ---------------------------------------------------------------------------

#[test]
fn out_of_order_input_rejected() {
    let mut core = HistoryCore::new(discovery_rule());
    let first_mined = mineral_mined_fixture("first-extraction");
    core.judge(&DomainEvent::mineral_mined(first_mined.clone()))
        .expect("첫 이벤트는 역행이 아니다");

    let state_before = format!("{core:?}");

    // 같은 (tick, sequence) 재입력 — 작거나 "같은" 경우도 거부 대상이다.
    let repeat = DomainEvent::mineral_mined(first_mined.clone());
    let result = core.judge(&repeat);
    assert!(result.is_err(), "같은 좌표의 재입력은 Err 이어야 한다");

    // 판정 상태가 바뀌지 않았음을 단언한다 — 다음 정상 입력이 이 이벤트를 안 본 것과 같다.
    assert_eq!(
        format!("{core:?}"),
        state_before,
        "역행 입력 뒤 판정 상태가 바뀌면 안 된다"
    );

    let mut earlier = first_mined;
    earlier.tick = Tick::new(1).unwrap();
    earlier.sequence = Sequence::new(0).unwrap();
    assert!(
        core.judge(&DomainEvent::mineral_mined(earlier)).is_err(),
        "더 이른 좌표도 역행으로 거부돼야 한다"
    );
}

// ---------------------------------------------------------------------------
// SC-49 — Level 0 타입만의 입력은 0건, 섞인 입력은 K건 (AC-10(h), H-08)
// ---------------------------------------------------------------------------

#[test]
fn level0_filter() {
    let world = "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b";

    // Level 0 타입만 — 0건.
    let mut only_level0 = HistoryCore::new(discovery_rule());
    let level0_events = vec![
        unjudged_event(world, "01a0b1c2-c001-7a01-8b11-505162738498", 100, 0),
        unjudged_event(world, "01a0b1c2-c002-7a02-8b12-505162738499", 101, 0),
    ];
    println!(
        "SC-49: SESSION_*/SHIP_* 류 입력 {} 건 (모두 Level 0)",
        level0_events.len()
    );
    let records = only_level0
        .judge_all(&level0_events)
        .expect("Level 0 입력은 역행이 아니다");
    assert_eq!(records.len(), 0);

    // 섞인 입력: 채굴 N건, 서로 다른 (성계, 광물) K = 2 개, N = 3K = 6.
    let mut mixed = HistoryCore::new(discovery_rule());
    let mut events = vec![unjudged_event(
        world,
        "01a0b1c2-c003-7a03-8b13-50516273849a",
        200,
        0,
    )];
    events.push(DomainEvent::mineral_mined(mineral_mined_fixture(
        "first-extraction",
    )));
    // starfall-glass 재추출 두 번 더 (중복, 발견 없음).
    for (i, (tick, seq)) in [(24001u64, 0u64), (24002, 0)].into_iter().enumerate() {
        let mut e = mineral_mined_fixture("first-extraction");
        e.event_id = uuid_v7(&format!("01a0b1c2-d00{i}-7a0{i}-8b1{i}-50516273849b"));
        e.tick = Tick::new(tick).unwrap();
        e.sequence = Sequence::new(seq).unwrap();
        events.push(DomainEvent::mineral_mined(e));
    }
    events.push(DomainEvent::mineral_mined(mineral_mined_fixture(
        "partial-last-kg-depletes",
    )));
    // glacine 재추출 두 번 더.
    for (i, (tick, seq)) in [(91235u64, 0u64), (91236, 0)].into_iter().enumerate() {
        let mut e = mineral_mined_fixture("partial-last-kg-depletes");
        e.event_id = uuid_v7(&format!("01a0b1c2-e00{i}-7a0{i}-8b1{i}-50516273849c"));
        e.tick = Tick::new(tick).unwrap();
        e.sequence = Sequence::new(seq).unwrap();
        events.push(DomainEvent::mineral_mined(e));
    }
    events.sort_by_key(|e| (e.tick, e.sequence));

    let session_or_ship_count = 1usize; // 위 unjudged_event 1건.
    let mineral_mined_count = events.len() - session_or_ship_count;
    println!(
        "SC-49: 입력 타입별 = SESSION_*/SHIP_* {session_or_ship_count}건, MINERAL_MINED {mineral_mined_count}건, K=2 성계x광물"
    );
    assert!(session_or_ship_count >= 1);
    assert!(mineral_mined_count >= 3 * 2);

    let records = mixed.judge_all(&events).expect("섞인 입력은 역행이 아니다");
    assert_eq!(
        records.len(),
        2,
        "서로 다른 (성계, 광물) 이 둘이므로 기록도 둘"
    );
}

// ---------------------------------------------------------------------------
// SC-50 — golden: rule_version 이 규칙 파일 정규화 해시 + 고정 입력 출력에 묶인다 (AC-10(i))
// ---------------------------------------------------------------------------
//
// 모든 SC-41~49 가 이미 [`discovery_rule`] 하나로 돈다 — 코드 안에 값의 두 번째 사본이
// 없으므로, "값이 여기서만 바뀌었다" 는 종류의 드리프트(qa 지적, 2026-09-27)는 애초에
// 생기지 않는다. golden 이 잡아야 하는 것은 그보다 좁다: **파일 값이 바뀌었는데
// `rule_version` 을 안 올린 경우**.

mod golden {
    use std::fs;
    use std::path::{Path, PathBuf};

    use sha2::{Digest, Sha256};
    use starfall_contracts::data::SignificanceRuleTable;
    use starfall_history::{DomainEvent, HistoryCore};

    use super::{discovery_rule, discovery_rule_path, mineral_mined_fixture, read_text};

    fn golden_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/golden")
    }

    /// 정규화: 규칙 파일의 **원본 JSON**(파싱한 Rust 타입이 아니다)을 키 정렬 정규형으로
    /// 다시 직렬화해 해시한다. `designer_note` 는 값이 아니라 설명문이라 뺀다.
    ///
    /// (개정, qa·팀장 지적 2026-09-27) — 이전 버전은 `SignificanceRuleTable` 의 열거형
    /// 필드를 `{:?}`(Debug) 로 찍어 해시했다. Debug 출력은 안정된 직렬화 계약이 아니다 —
    /// variant 이름을 바꾸거나 파생을 수동 구현으로 바꾸면 **규칙 값이 그대로여도** 해시가
    /// 흔들리고, 반대로 와이어 문자열(`PUBLIC` 등)과 Debug 이름이 갈라져도 이 함수는 모른다.
    /// 이제 원본 JSON 텍스트를 파싱해 `serde_json::Value` 로 만들고(`serde_json` 기본
    /// 빌드는 `preserve_order` 를 안 켜서 객체가 `BTreeMap` 이므로 재직렬화가 **키 정렬**로
    /// 나온다), 그 바이트를 해시한다 — 해시가 Rust 타입의 모양이 아니라 **파일의 값**에만
    /// 반응한다.
    fn normalise_and_hash(raw_json: &str) -> String {
        let mut value: serde_json::Value =
            serde_json::from_str(raw_json).expect("규칙 파일이 유효한 JSON 이어야 한다");
        if let Some(obj) = value.as_object_mut() {
            obj.remove("designer_note");
        }
        let canonical =
            serde_json::to_string(&value).expect("정규화된 값의 재직렬화는 항상 성공한다");
        let digest = Sha256::digest(canonical.as_bytes());
        format!("{digest:x}")
    }

    fn fixed_input_output(rule: &SignificanceRuleTable) -> Vec<String> {
        let mut core = HistoryCore::new(rule.clone());
        let event = DomainEvent::mineral_mined(mineral_mined_fixture("first-extraction"));
        let records = core.judge(&event).expect("고정 입력은 역행이 아니다");
        records
            .iter()
            .map(|r| {
                format!(
                    "{}|{}|{}|{}",
                    r.event.historical_event_id,
                    r.event.rule_version,
                    r.event.importance_level,
                    r.evidence.evidence_id
                )
            })
            .collect()
    }

    #[test]
    fn rule_golden() {
        let raw_json = read_text(discovery_rule_path());
        let rule = discovery_rule();
        assert!(
            rule.rule_version
                .as_str()
                .starts_with(&format!("{}@", rule.rule_id.as_str())),
            "rule_version 은 rule_id + '@' 로 시작해야 한다(ADR-0014 §5 기동 검사)"
        );

        let content_hash = normalise_and_hash(&raw_json);
        let output = fixed_input_output(&rule);

        let hash_path = golden_dir().join(format!("{}.hash.txt", rule.rule_version.as_str()));
        let output_path = golden_dir().join(format!("{}.output.txt", rule.rule_version.as_str()));

        if std::env::var("STARFALL_REPLAY_BLESS").as_deref() == Ok("1") {
            fs::create_dir_all(golden_dir()).expect("golden 디렉터리 생성 실패");
            fs::write(&hash_path, &content_hash).expect("golden 해시 쓰기 실패");
            fs::write(&output_path, output.join("\n")).expect("golden 출력 쓰기 실패");
        }

        let expected_hash = fs::read_to_string(&hash_path).unwrap_or_else(|e| {
            panic!(
                "golden 해시 파일이 없다({}): {e} — STARFALL_REPLAY_BLESS=1 로 먼저 만든다",
                hash_path.display()
            )
        });
        let expected_output = fs::read_to_string(&output_path)
            .unwrap_or_else(|e| panic!("golden 출력 파일이 없다({}): {e}", output_path.display()));

        assert_eq!(
            content_hash,
            expected_hash.trim(),
            "규칙 파일 내용이 golden 해시와 달라졌다 — rule_version 을 올렸는가?"
        );
        assert_eq!(
            output.join("\n"),
            expected_output.trim_end(),
            "고정 입력에 대한 코어 출력이 golden 과 달라졌다"
        );
    }

    /// 음성 대조: `rule_version` 을 올리지 않고 값만 바꾼 규칙 파일은 golden 해시 비교에서
    /// **실제로** 실패해야 한다(golden 이 죽은 assert 가 아님을 이 테스트가 관측한다).
    ///
    /// 메모리 위의 `serde_json::Value` 만 바꾼다 — 실제 규칙 파일은 안 읽지도 쓰지도 않는다
    /// (팀장 지시 2026-09-27: 공유 추적 파일을 변이 시연에 직접 쓰지 않는다).
    #[test]
    fn rule_golden_detects_value_change_without_version_bump() {
        let raw_json = read_text(discovery_rule_path());
        let before_hash = normalise_and_hash(&raw_json);

        let mut value: serde_json::Value =
            serde_json::from_str(&raw_json).expect("규칙 파일이 유효한 JSON 이어야 한다");
        let rule_version = value["rule_version"]
            .as_str()
            .expect("rule_version 은 문자열이어야 한다")
            .to_string();
        // 값만 바꾼다(rule_version 은 그대로).
        let current = value["importance_level"]
            .as_u64()
            .expect("importance_level 은 정수여야 한다");
        value["importance_level"] = serde_json::json!(if current == 5 { 1 } else { current + 1 });
        let mutated_json = serde_json::to_string(&value).expect("직렬화는 항상 성공한다");

        let after_hash = normalise_and_hash(&mutated_json);

        assert_ne!(
            before_hash, after_hash,
            "값을 바꿨는데 정규화 해시가 그대로면 golden 이 그 변경을 못 잡는다"
        );

        // 이 "달라진" 해시로 golden 파일과 비교하면 실패해야 한다(Err 를 관측한다 —
        // should_panic 이 아니라 결과를 직접 비교).
        let expected_hash_path = golden_dir().join(format!("{rule_version}.hash.txt"));
        let golden_hash = fs::read_to_string(&expected_hash_path)
            .expect("golden 해시 파일이 있어야 한다(rule_golden 이 먼저 만든다)");
        assert_ne!(
            after_hash,
            golden_hash.trim(),
            "값이 바뀐 규칙 파일의 해시가 golden 과 여전히 같다 — golden 이 드리프트를 못 잡는다"
        );
    }

    /// qa·팀장 지적(2026-09-27)의 핵심 성질을 직접 건다: **Debug 포맷이 아니라 값**에만
    /// 해시가 반응한다. `SignificanceRuleTable` 의 Rust 쪽 모양(필드 순서, enum variant
    /// 이름 등)이 바뀌어도, 같은 JSON 값이면 해시가 같아야 한다. JSON 텍스트의 키 순서를
    /// 뒤집고 공백을 달리해도(값은 그대로) 같은 해시가 나오는 것으로 이를 확인한다.
    #[test]
    fn hash_is_insensitive_to_json_key_order_and_whitespace() {
        let raw_json = read_text(discovery_rule_path());
        let canonical_hash = normalise_and_hash(&raw_json);

        let value: serde_json::Value =
            serde_json::from_str(&raw_json).expect("규칙 파일이 유효한 JSON 이어야 한다");
        // 키 순서를 일부러 뒤집어 다시 쓴 텍스트(값은 완전히 같다).
        let reversed_keys: serde_json::Map<String, serde_json::Value> = value
            .as_object()
            .expect("규칙 파일은 JSON 객체다")
            .iter()
            .rev()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let reordered_text =
            serde_json::to_string_pretty(&serde_json::Value::Object(reversed_keys))
                .expect("직렬화는 항상 성공한다");

        assert_ne!(
            raw_json.trim(),
            reordered_text.trim(),
            "이 입력 재배열이 실제로 원문과 달라야(글자 그대로) 이 테스트가 무언가를 잰다"
        );
        assert_eq!(
            normalise_and_hash(&reordered_text),
            canonical_hash,
            "키 순서·공백만 다른 같은 값인데 해시가 달라졌다 — 정규화가 값이 아니라 텍스트 모양에 반응하고 있다"
        );
    }
}
