//! p1-02 채굴 관측 판정 헬퍼의 음성·양성 대조 (계약 §7b 규칙 6).
//!
//! `pair_mine_responses` 는 스펙 §5.1a 의 항등식(수락된 `MINE_RESOURCE` 수 == 결과 뒤 같은 tick
//! 의 `INVENTORY_STATE` 수, 계약 SC-99)을, `leak_hits` 는 누출 검사(SC-40)를 계산한다. 둘 다
//! **걸려야 할 입력에서 걸리는지**를 여기서 먼저 보인다.

use starfall_bots::mining::{MiningObs, Observed, ObservedResult, leak_hits, pair_mine_responses};
use starfall_bots::wire::{InventoryItem, InventoryStateMessage, InventoryStatePayload};
use uuid::Uuid;

fn inv(frame_seq: u64, tick: u64) -> Observed<InventoryStateMessage> {
    Observed {
        frame_seq,
        at_us: frame_seq * 10,
        tick,
        msg: InventoryStateMessage {
            message_id: Uuid::now_v7(),
            message_type: "INVENTORY_STATE".to_owned(),
            schema_version: 1,
            tick,
            correlation_id: None,
            payload: InventoryStatePayload {
                actor_id: Uuid::nil(),
                items: vec![InventoryItem {
                    mineral_id: "ferrosite".to_owned(),
                    quantity_kg: 200,
                }],
            },
        },
    }
}

fn result(frame_seq: u64, tick: u64, id: Uuid, status: &str) -> ObservedResult {
    ObservedResult {
        frame_seq,
        tick,
        command_id: id,
        status: status.to_owned(),
        reason_code: None,
    }
}

#[test]
fn pairing_accepts_result_then_inventory_same_tick() {
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let obs = MiningObs {
        results: vec![result(1, 100, a, "ACCEPTED"), result(3, 160, b, "ACCEPTED")],
        inventory: vec![inv(2, 100), inv(4, 160)],
        ..MiningObs::default()
    };
    let p = pair_mine_responses(&obs, &[a, b]);
    assert_eq!((p.accepted_mine, p.paired), (2, 2));
    assert!(p.ok());
}

#[test]
fn pairing_with_zero_accepted_is_not_ok() {
    // 0 == 0 은 검사가 아니다 — 수락 0 이면 FAIL(스펙 §5.1a).
    let p = pair_mine_responses(&MiningObs::default(), &[]);
    assert_eq!((p.accepted_mine, p.paired), (0, 0));
    assert!(!p.ok(), "수락 0 에서 ok() 가 참이면 자명 통과다");
}

#[test]
fn pairing_rejects_inventory_before_result_or_other_tick() {
    let a = Uuid::now_v7();
    // 인벤토리가 결과 **앞**에 왔다 → 짝이 아니다.
    let before = MiningObs {
        results: vec![result(2, 100, a, "ACCEPTED")],
        inventory: vec![inv(1, 100)],
        ..MiningObs::default()
    };
    assert!(!pair_mine_responses(&before, &[a]).ok());
    // 다른 tick → 짝이 아니다.
    let other_tick = MiningObs {
        results: vec![result(1, 100, a, "ACCEPTED")],
        inventory: vec![inv(2, 101)],
        ..MiningObs::default()
    };
    assert!(!pair_mine_responses(&other_tick, &[a]).ok());
}

#[test]
fn pairing_does_not_reuse_one_inventory_for_two_accepts() {
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let obs = MiningObs {
        results: vec![result(1, 100, a, "ACCEPTED"), result(2, 100, b, "ACCEPTED")],
        inventory: vec![inv(3, 100)],
        ..MiningObs::default()
    };
    let p = pair_mine_responses(&obs, &[a, b]);
    assert_eq!((p.accepted_mine, p.paired), (2, 1));
    assert!(!p.ok());
}

#[test]
fn pairing_ignores_non_mine_and_rejected_results() {
    let mine = Uuid::now_v7();
    let ping = Uuid::now_v7();
    let rejected = Uuid::now_v7();
    let obs = MiningObs {
        results: vec![
            result(1, 100, ping, "ACCEPTED"),
            result(2, 100, rejected, "REJECTED"),
            result(3, 100, mine, "ACCEPTED"),
        ],
        inventory: vec![inv(4, 100)],
        ..MiningObs::default()
    };
    let p = pair_mine_responses(&obs, &[mine, rejected]);
    assert_eq!((p.accepted_mine, p.paired), (1, 1));
}

#[test]
fn leak_scan_finds_and_misses() {
    let frames = vec![
        (
            0u64,
            r#"{"deposit_id":"far-reach","mineral_id":null}"#.to_owned(),
        ),
        (
            1u64,
            r#"{"deposit_id":"far-reach","mineral_id":"starfall-glass"}"#.to_owned(),
        ),
    ];
    let forbidden = vec!["starfall-glass".to_owned()];
    // 양성: 드러난 뒤 프레임에서 적중 ≥ 1 (검출기가 산다)
    assert_eq!(
        leak_hits(&frames, &forbidden),
        vec![(1, "starfall-glass".to_owned())]
    );
    // 음성: 첫 채굴 전 프레임만 보면 0
    assert!(leak_hits(&frames[..1], &forbidden).is_empty());
}

fn fixture(rel: &str) -> String {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let p = dir.join(rel);
        if p.is_file() {
            return std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{rel}: {e}"));
        }
        assert!(dir.pop(), "{rel} 를 찾지 못했다");
    }
}

#[test]
fn deposit_shape_accepts_contract_fixtures() {
    use starfall_bots::mining::deposit_state_shape;
    let frames = vec![
        (
            0u64,
            fixture("contracts/fixtures/DEPOSIT_FIELD_STATE/all-unrevealed.json"),
        ),
        (
            1u64,
            fixture("contracts/fixtures/DEPOSIT_FIELD_STATE/revealed-and-depleted.json"),
        ),
    ];
    let (messages, entries, unrevealed, bad) = deposit_state_shape(&frames);
    assert_eq!(messages, 2);
    assert!(entries >= 16, "광맥 8 × 2 메시지: {entries}");
    assert!(
        unrevealed >= 8,
        "첫 메시지는 전부 미확인이어야 한다: {unrevealed}"
    );
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn deposit_shape_catches_missing_key_and_half_reveal() {
    use starfall_bots::mining::deposit_state_shape;
    let base = fixture("contracts/fixtures/DEPOSIT_FIELD_STATE/all-unrevealed.json");
    let mut v: serde_json::Value = serde_json::from_str(&base).unwrap_or_else(|e| panic!("{e}"));
    // 첫 항목: 키 하나 삭제 / 둘째 항목: mineral_id 만 채움(반쯤 드러남 — 누출의 모양)
    let deps = v["payload"]["deposits"]
        .as_array_mut()
        .unwrap_or_else(|| panic!("deposits"));
    deps[0]
        .as_object_mut()
        .unwrap_or_else(|| panic!("obj"))
        .remove("remaining_kg");
    deps[1]["mineral_id"] = serde_json::json!("starfall-glass");
    let (_, _, _, bad) = deposit_state_shape(&[(7u64, v.to_string())]);
    assert_eq!(bad.len(), 2, "{bad:?}");
    assert!(bad.iter().all(|b| b.frame_seq == 7));
}
