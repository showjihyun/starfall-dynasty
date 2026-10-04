//! STARFALL DYNASTY 역사 판정 코어 (ADR-0014 §1).
//!
//! Domain Event 를 받아 Historical Event 중요도 판정을 내리는 **순수** 계층이다.
//! `sim`·`domain` 과 같은 이유로 IO 가 없다 — 시간(`SystemTime`/`Instant`)·소켓·DB·난수는
//! 이 크레이트에 들어오지 않는다(rust-authoritative-server §4). 판정이 결정적이어야
//! 감사 가능하다(CLAUDE.md 절대 원칙 9).
//!
//! # T0 는 골격만 둔다
//!
//! 이 파일은 T0(경계 인터페이스 합의)의 산출물이다. 실제 판정 규칙(`mineral-discovery@1`
//! 등)과 [`HistoricalEventRecord`] 의 진짜 필드는 H1 이 채운다. 지금 시그니처가 존재하는
//! 이유는 오직 하나: **다른 크레이트(server 쪽 `starfall-persistence`)가 이 크레이트를
//! 두고 먼저·독립적으로 빌드될 수 있게 하는 것**이다(`_workspace/p1-02-mining/02_server_ack.md`
//! §4, history 제안 (7) — "load·run_runner 시그니처는 unimplemented! 없이 빈 Ok 를
//! 돌려 서로 독립 빌드한다").
//!
//! 크레이트 경계·의존(T0 합의): `starfall-history` → `starfall-contracts` + `uuid`(v5) 뿐.
//! `axum`/`sqlx`/`redis`/`rand`/`chrono`/`tokio` 는 이 크레이트의 `[dependencies]` 에
//! 없다(SC-02, SC-03).

#![forbid(unsafe_code)]

pub mod core;
pub mod id;
pub mod input;
pub mod output;
pub mod record;

pub use core::{CoreError, HistoryCore};
pub use input::{DomainEvent, DomainEventKind};
pub use output::{EvidenceDraft, MineralDiscoveredDraft, MineralDiscoveredRecord};
pub use record::HistoricalEventRecord;

// `HistoryCore::new` 는 규칙 값을 `starfall_contracts::data::SignificanceRuleTable` 로
// 받는다 — 규칙 값의 원천은 `data/history/rules/mineral-discovery.json` 하나뿐이고(원칙
// 9), 코어 안에 그 값의 두 번째 사본(하드코딩 상수)을 두지 않는다(qa 지적, 2026-09-27 —
// 이전의 `DiscoveryRuleConfig::v1()` 상수는 golden 에도 파일에도 묶여 있지 않았다).

#[cfg(test)]
mod tests {
    /// SC-01 증거: `starfall-history` 가 워크스페이스 멤버로 링크되어 `cargo test` 가 이
    /// 크레이트의 테스트 바이너리를 최소 1건 통과시킨다. H1 이 실제 판정 테스트로
    /// 대체·확장한다.
    #[test]
    fn crate_links_into_the_workspace() {}
}
