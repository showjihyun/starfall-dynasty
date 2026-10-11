//! 거래(판매) 판정 — p1-03-trading.
//!
//! 이 모듈의 `price` 서브모듈(S3b)은 가격 규칙의 **순수 함수**만 담는다 — `Simulation`
//! 의 상태를 모른다. 판정 흐름 자체(8단계, 이벤트·메시지 생성, 인벤토리·지갑·시장
//! 상태 변경)는 `simulation.rs` 가 소유한다(S3, `crate::mining` 모듈과 같은 분업).

use std::collections::BTreeMap;

use starfall_contracts::primitives::{DataId, RuleVersion, UuidV7};

use crate::world::Vec3;

pub mod price;
pub use price::PriceRule;

/// 시장 하나의 월드 상수(`data/market/*.json`, `bins/game-server/src/data.rs` 가
/// §4.8 검산을 끝낸 값).
#[derive(Debug, Clone)]
pub struct MarketConstants {
    /// 성계 로컬 좌표.
    pub position_m: Vec3,
    /// 거래소 구조물 반지름.
    pub radius_m: f64,
    /// 도킹 판정: `|ship - market|^2 <= (radius_m + 이 값)^2`.
    pub dock_range_from_surface_m: f64,
    /// 도킹 허용 최대 함선 속력.
    pub max_ship_speed_mps: f64,
    /// 판매 쿨다운(tick). 채굴 쿨다운과 별개, 메모리 상태.
    pub sell_cooldown_ticks: u64,
    /// 포화 회복 간격(tick).
    pub recovery_interval_ticks: u64,
    /// 이 시장이 매긴 가격 규칙의 버전 — 모든 `MINERAL_SOLD` 에 그대로 싣는다(I-90).
    pub pricing_version: RuleVersion,
    /// 광물 id → 상장 상수(그 시장의 상장).
    pub listings: BTreeMap<DataId, ListingConstants>,
}

/// 상장 하나의 상수 — 가격 규칙(`price.rs`, 회복량을 모른다) + 회복량(회복 간격은
/// 시장 전체가 공유 — [`MarketConstants::recovery_interval_ticks`]).
#[derive(Debug, Clone, Copy)]
pub struct ListingConstants {
    /// 가격 규칙(순수 — [`price::sell`] 의 입력).
    pub price: PriceRule,
    /// `recovery_interval_s` 의 배수 tick마다 빠지는 포화.
    pub recovery_kg: i64,
}

/// 시장의 광물 하나의 런타임 상태 — 그 (market, mineral) 의 첫 판매에 생긴다(ADR-0016
/// §4, `Deposit` 의 "드러남"과 같은 존재 규약: 맵에 항목이 없으면 "한 번도 안 팔림").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketRuntimeState {
    /// `as_of_tick` 시점 기준 포화(회복 미적용 — [`price::effective_saturation`] 이
    /// 적용한다).
    pub saturation_kg: i64,
    /// `saturation_kg` 를 기록한 tick(= 마지막 판매 tick, 둘은 언제나 같다 — ADR-0016
    /// §4).
    pub as_of_tick: u64,
    /// 시장의 공개 게시판(디자인 §4) — 마지막으로 판 사람.
    pub last_seller_actor_id: UuidV7,
    /// 같은 게시판의 판매량.
    pub last_sale_quantity_kg: i64,
}
