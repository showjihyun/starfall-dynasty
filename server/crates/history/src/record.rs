//! 역사 기록 채널(러너 → 게이트웨이)이 나르는 항목의 자리표시자 타입.
//!
//! T0 합의(`02_server_ack.md` §4, 조정 1): 채널 항목은 `HISTORICAL_EVENT_NOTICE` 메시지
//! **그 자체가 아니다** — NOTICE 의 `tick`(보낼 때의 게이트웨이 현재 tick)·`message_id`
//! (송신마다 새로 생성)는 러너가 알 수 없는 값이기 때문이다. 대신 채널 항목은 계약의
//! 레코드 타입(`MINERAL_DISCOVERED` 등)이고, 게이트웨이의 tick 드라이버가 `delivery`·
//! `tick`·`message_id` 를 붙여 NOTICE 를 조립한다(S5 의 몫).
//!
//! `MINERAL_DISCOVERED` 계약 타입은 아직 없다(S1 이 만든다 — `01_architect_tasks.md`
//! 순서: T0 ∥ S1). 그래서 지금은 재현성 검증(H1 의 "id 재현" 테스트, AC-10)에 필요한
//! 최소 필드만 있는 자리표시자다. **S1 이 계약 타입을 만들면 이 구조체를 그 타입으로
//! 교체한다** — H1 이 그 교체 지점이다.
use starfall_contracts::primitives::UuidV7;

/// 역사 기록 채널(초기 BACKFILL 목록 + 이후 LIVE) 항목의 자리표시자.
///
/// 실제 필드(광물·광맥·발견자 등)는 H1 이 계약의 `MINERAL_DISCOVERED` 레코드로 채운다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalEventRecord {
    /// UUIDv5(이름 기반) 로 재현 가능한 역사 이벤트 id (H1, AC-10).
    pub historical_event_id: UuidV7,
}
