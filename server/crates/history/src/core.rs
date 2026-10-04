//! `HistoryCore` — `mineral-discovery@1` 규칙의 상태 있는 순수 판정기 (ADR-0014 §1·§5).
//!
//! 판정 상태(월드마다 "이미 발견된 (성계, 광물) 집합")는 history 자신의 앞선 판정
//! 결과에서만 나온다 — 라이브 `WorldState` 를 읽지 않는다. 재구축(기동 시
//! `historical_events` 에서, 또는 새 월드에서 `domain_events` 를 처음부터 재생)은 같은
//! 입력열에 대해 항상 같은 결과를 내야 하고, 그것이 이 타입의 유일한 계약이다.
//!
//! 규칙 값의 원천은 `data/history/rules/mineral-discovery.json` **하나**다(원칙 9). 코어는
//! 그 값을 조립 바이너리(S2)가 파싱해 넘긴 `SignificanceRuleTable` 로 받는다 — 코어 안에
//! 규칙 값의 두 번째 사본(하드코딩 상수 등)을 두지 않는다. 파일을 직접 읽지는 않는다(§5).

use std::collections::{HashMap, HashSet};

use starfall_contracts::data::{EvidenceType, SignificanceRuleTable};
use starfall_contracts::events::MineralMinedEvent;
use starfall_contracts::historical::{
    HistoricalEntityKind, HistoricalLocation, HistoricalParticipant, HistoricalRole,
};
use starfall_contracts::primitives::{DataId, Sequence, Tick, UuidV7};

use crate::id;
use crate::input::{DomainEvent, DomainEventKind};
use crate::output::{EvidenceDraft, MineralDiscoveredDraft, MineralDiscoveredRecord};

/// 코어 오류. 지금은 역행 입력 하나뿐이다.
///
/// ADR-0014 §1: "한 월드에 대해 직전 입력보다 `(tick, sequence)` 가 작거나 같은 이벤트가
/// 오면 코어가 오류를 낸다" — 러너의 `ORDER BY` 누락 같은 버그가 "틀린 발견자" 라는 영구
/// 기록 대신 **정지**로 나타나게 한다(SC-48).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    #[error(
        "월드 {world_id} 에 역행 입력: 이전 좌표 ({prev_tick}, {prev_sequence}) 뒤에 \
         ({tick}, {sequence}) 가 왔다"
    )]
    OutOfOrder {
        world_id: String,
        prev_tick: u64,
        prev_sequence: u64,
        tick: u64,
        sequence: u64,
    },
}

/// 월드 하나의 판정 상태.
#[derive(Debug, Clone, Default)]
struct WorldState {
    /// 이 월드에서 마지막으로 판정한 이벤트의 좌표 — 역행 검사용.
    last_seen: Option<(Tick, Sequence)>,
    /// 이미 "최초 발견"이 난 `(star_system_id, mineral_id)` — 의미 유일성의 코어 쪽 절반
    /// (DB 쪽 절반은 H2 의 `UNIQUE (world_id, event_type, dedupe_key)`).
    discovered: HashSet<(DataId, DataId)>,
}

/// `mineral-discovery@1` 규칙을 쥔 판정 코어. 월드별 상태를 들고 있는 것 말고는 순수하다 —
/// 시계·소켓·DB·난수를 쓰지 않는다(ADR-0014 §1).
#[derive(Debug, Clone)]
pub struct HistoryCore {
    rule: SignificanceRuleTable,
    worlds: HashMap<UuidV7, WorldState>,
}

impl HistoryCore {
    /// 빈 판정 상태(새 월드, 또는 재구축의 시작점)에서 코어를 만든다. `rule` 은 조립
    /// 바이너리가 `data/history/rules/mineral-discovery.json` 을 적재·검증해 넘긴 값이다
    /// (코어는 파일을 읽지 않는다).
    #[must_use]
    pub fn new(rule: SignificanceRuleTable) -> Self {
        Self {
            rule,
            worlds: HashMap::new(),
        }
    }

    /// 기동 재구축(H2) — 이미 커밋된 `historical_events` 에서 판정 상태를 되살려 코어를
    /// 만든다. 이 슬라이스의 러너는 월드 하나만 다루므로 그 월드의 상태만 받는다.
    ///
    /// `last_seen` 은 `history_cursor` 가 별도 표로 이미 추적하므로 꼭 채울 필요는 없다
    /// (러너가 커서 뒤의 이벤트만 넘긴다) — 그래도 방어적으로 받는다: 역행 검사(SC-48)가
    /// 재구축 직후 첫 이벤트에도 그대로 걸리게 하기 위해서다.
    #[must_use]
    pub fn rebuild(
        rule: SignificanceRuleTable,
        world_id: UuidV7,
        discovered: impl IntoIterator<Item = (DataId, DataId)>,
        last_seen: Option<(Tick, Sequence)>,
    ) -> Self {
        let mut worlds = HashMap::new();
        worlds.insert(
            world_id,
            WorldState {
                last_seen,
                discovered: discovered.into_iter().collect(),
            },
        );
        Self { rule, worlds }
    }

    /// 이벤트 하나를 판정한다.
    ///
    /// 순서: (1) 역행 검사 — 판정 대상 타입 여부와 무관하게 **모든** 이벤트에 적용된다.
    /// (2) `MineralMined` 가 아니면 즉시 빈 결과(Level 0, SC-49). (3) 이미 발견된
    /// `(star_system_id, mineral_id)` 면 빈 결과(SC-42). (4) 아니면 새 발견 기록 1건.
    ///
    /// # Errors
    ///
    /// 이 월드에 대해 이전 좌표보다 작거나 같은 `(tick, sequence)` 가 오면
    /// [`CoreError::OutOfOrder`] — 판정 상태는 **바뀌지 않는다**(호출자가 이 이벤트를 뺀
    /// 다음 정상 입력으로 계속할 수 있게).
    pub fn judge(
        &mut self,
        event: &DomainEvent,
    ) -> Result<Vec<MineralDiscoveredRecord>, CoreError> {
        let world = self.worlds.entry(event.world_id).or_default();

        if let Some((prev_tick, prev_sequence)) = world.last_seen
            && (event.tick, event.sequence) <= (prev_tick, prev_sequence)
        {
            return Err(CoreError::OutOfOrder {
                world_id: event.world_id.to_string(),
                prev_tick: prev_tick.get(),
                prev_sequence: prev_sequence.get(),
                tick: event.tick.get(),
                sequence: event.sequence.get(),
            });
        }
        world.last_seen = Some((event.tick, event.sequence));

        let DomainEventKind::MineralMined(mined) = &event.kind else {
            return Ok(Vec::new());
        };

        let dedupe = (
            mined.payload.star_system_id.clone(),
            mined.payload.mineral_id.clone(),
        );
        if !world.discovered.insert(dedupe) {
            return Ok(Vec::new());
        }

        Ok(vec![self.build_record(mined)])
    }

    /// `(tick, sequence)` 오름차순으로 이미 정렬된 이벤트열을 차례로 판정한다
    /// (SC-46 배치 경계 불변 — 어디서 잘라 호출하든 합친 결과가 같다).
    ///
    /// # Errors
    ///
    /// [`Self::judge`] 와 같다 — 첫 오류에서 멈추고, 그 오류를 낸 이벤트까지의 결과는
    /// 이미 반영된 판정 상태 위에서 이어갈 수 있다.
    pub fn judge_all(
        &mut self,
        events: &[DomainEvent],
    ) -> Result<Vec<MineralDiscoveredRecord>, CoreError> {
        let mut records = Vec::new();
        for event in events {
            records.extend(self.judge(event)?);
        }
        Ok(records)
    }

    fn build_record(&self, mined: &MineralMinedEvent) -> MineralDiscoveredRecord {
        let dedupe_key = format!(
            "{}/{}",
            mined.payload.star_system_id, mined.payload.mineral_id
        );
        let historical_event_id = id::historical_event_id(
            &mined.world_id.to_string(),
            &self.rule.produces_event_type,
            &dedupe_key,
        );
        // `EvidenceType` 은 SCREAMING_SNAKE_CASE 로 직렬화되지만(`ShipLog` -> `"SHIP_LOG"`),
        // id 파생은 그 와이어 문자열이 필요하다 — 닫힌 집합 하나뿐이라 매치로 옮긴다.
        let evidence_type_wire = match self.rule.evidence.evidence_type {
            EvidenceType::ShipLog => "SHIP_LOG",
        };
        let evidence_id = id::evidence_id(&historical_event_id, evidence_type_wire);

        let event = MineralDiscoveredDraft {
            historical_event_id,
            world_id: mined.world_id,
            dedupe_key: dedupe_key.clone(),
            rule_version: self.rule.rule_version.clone(),
            importance_level: self.rule.importance_level,
            tick: mined.tick,
            occurred_at: mined.occurred_at.clone(),
            visibility: self.rule.visibility,
            source_event_ids: vec![mined.event_id],
            location: HistoricalLocation {
                star_system_id: mined.payload.star_system_id.clone(),
            },
            // 순서 고정: DISCOVERER → VESSEL (jsonb 배열 동등은 순서를 본다 — SC-41·106).
            participants: vec![
                HistoricalParticipant {
                    entity_id: mined.actor_id,
                    entity_kind: HistoricalEntityKind::Player,
                    role: HistoricalRole::Discoverer,
                },
                HistoricalParticipant {
                    entity_id: mined.payload.ship_id,
                    entity_kind: HistoricalEntityKind::Ship,
                    role: HistoricalRole::Vessel,
                },
            ],
            payload: starfall_contracts::historical::MineralDiscoveredPayload {
                mineral_id: mined.payload.mineral_id.clone(),
                deposit_id: mined.payload.deposit_id.clone(),
                quantity_kg: mined.payload.quantity_kg,
            },
        };

        let evidence = EvidenceDraft {
            evidence_id,
            evidence_type: self.rule.evidence.evidence_type,
            visibility: self.rule.evidence.visibility,
            source_entity_id: mined.payload.ship_id,
        };

        MineralDiscoveredRecord { event, evidence }
    }
}
