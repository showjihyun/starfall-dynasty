//! 결정적 id 파생 (ADR-0014 §1) — `historical_event_id` · `evidence_id`.
//!
//! 무작위 id 였다면 재구축이 id 를 바꾸고, 와이어의 중복 제거 키(스펙 I-65)와 p2 Claim 의
//! `about_event_id` 참조를 고아로 만든다. 그래서 이름 기반 UUIDv5 로 **의미에서** 파생한다 —
//! 같은 월드·같은 타입·같은 dedupe 키는 언제나 같은 id 를 낸다. SHA-1 은 id 유도용이지
//! 보안용이 아니다.

use starfall_contracts::primitives::UuidV5;
use uuid::Uuid;

/// 역사 이벤트 id 이름공간. **바꾸면 모든 역사 id 가 바뀐다 — 바꾸지 않는다**(ADR-0014 §1).
pub const NS_HISTORY: Uuid = Uuid::from_u128(0x29c4_82cc_234d_4876_a837_347b_852f_802d);

/// 증거 id 이름공간. 위와 같은 이유로 고정이다.
pub const NS_EVIDENCE: Uuid = Uuid::from_u128(0x503d_3cb5_4cfe_4b52_9d4e_8acd_439a_d5c1);

/// `historical_event_id = UUIDv5(NS_HISTORY, "{world_id}|{event_type}|{dedupe_key}")`.
///
/// `dedupe_key` 에 `rule_version` 을 넣지 않는다 — 규칙이 `@2` 로 올라도 같은 광물을
/// "다시 발견" 하지 않게 하기 위해서다(ADR-0014 §4).
#[must_use]
pub fn historical_event_id(world_id: &str, event_type: &str, dedupe_key: &str) -> UuidV5 {
    let name = format!("{world_id}|{event_type}|{dedupe_key}");
    UuidV5::new_v5(&NS_HISTORY, name.as_bytes())
}

/// `evidence_id = UUIDv5(NS_EVIDENCE, "{historical_event_id}|{evidence_type}")`.
///
/// 역사 이벤트 1건당 자동 증거 1건이므로 `historical_event_id` 만으로 유일하다 —
/// `ship_id` 등을 더 섞을 필요가 없다.
#[must_use]
pub fn evidence_id(historical_event_id: &UuidV5, evidence_type: &str) -> UuidV5 {
    let name = format!("{historical_event_id}|{evidence_type}");
    UuidV5::new_v5(&NS_EVIDENCE, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// history 검토(`01_history_review.md` §9.5)가 Python `uuid.uuid5` 로 독립 계산해
    /// fixture `MINERAL_DISCOVERED/starfall-glass.json` 과 일치를 확인한 값이다. 이 테스트는
    /// Rust 파생 함수가 **같은 독립 계산과 같은 값**을 내는지를 건다(자기 일치가 아니다 —
    /// 기대값은 이 함수를 호출해 만든 것이 아니라 옆에서 따로 계산됐다).
    #[test]
    fn historical_event_id_matches_independently_computed_fixture_value() {
        let id = historical_event_id(
            "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
            "MINERAL_DISCOVERED",
            "cradle/starfall-glass",
        );
        assert_eq!(id.to_string(), "2775a80a-2a8a-5615-a86f-859ea777901d");
    }

    #[test]
    fn evidence_id_matches_independently_computed_fixture_value() {
        let hist_id = historical_event_id(
            "01a0b1c2-3d4e-7f01-8a2b-9c0d1e2f3a4b",
            "MINERAL_DISCOVERED",
            "cradle/starfall-glass",
        );
        let evidence = evidence_id(&hist_id, "SHIP_LOG");
        assert_eq!(evidence.to_string(), "e9b611d1-637e-5287-862e-74eabffb0252");
    }
}
