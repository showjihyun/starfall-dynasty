//! p1-03 거래 판정 함수의 양성·음성 대조(규칙 6) — 합성 관측으로, 서버 없이.
//! 각 판정이 ⊘(조건 미발생)에서 PASS 가 아니라 미검증을, 조건이 났는데 기대가 틀리면 FAIL 을 내는지 본다.

use starfall_bots::mine_cases::Verdict;
use starfall_bots::mining::{MiningObs, Observed, ObservedResult, SentSell, pair_sell_responses};
use starfall_bots::trade::{self, Exchange, SellCase};
use starfall_bots::wire::*;
use uuid::Uuid;

fn ex() -> Exchange {
    Exchange {
        id: "cradle-exchange".into(),
        position_m: [0.0, 1500.0, 0.0],
        radius_m: 120.0,
        dock_range_m: 200.0,
    }
}

fn mid() -> Uuid {
    Uuid::now_v7()
}

struct B {
    m: MiningObs,
    seq: u64,
}

impl B {
    fn new() -> Self {
        Self {
            m: MiningObs::default(),
            seq: 0,
        }
    }
    fn next(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
    fn sent(&mut self, id: Uuid, q: i64, pos: [f64; 3], speed: f64, text: &str) {
        let fs = self.seq + 1;
        self.m.sell_sent.push(SentSell {
            command_id: id,
            market_id: "cradle-exchange".into(),
            mineral_id: "ferrosite".into(),
            quantity_kg: q,
            text: text.into(),
            at_us: 0,
            frame_seq_at_send: fs,
            last_snapshot_tick: Some(1),
            pos_m: Some(pos),
            speed_mps: Some(speed),
        });
    }
    fn result(&mut self, id: Uuid, tick: u64, status: &str, reason: Option<&str>) {
        let f = self.next();
        self.m.results.push(ObservedResult {
            frame_seq: f,
            tick,
            command_id: id,
            status: status.into(),
            reason_code: reason.map(Into::into),
        });
    }
    fn receipt(&mut self, id: Uuid, tick: u64, q: i64, proceeds: i64, after: i64) {
        let f = self.next();
        self.m.receipts.push(Observed {
            frame_seq: f,
            at_us: 0,
            tick,
            msg: SaleReceiptMessage {
                message_id: mid(),
                message_type: SALE_RECEIPT.into(),
                schema_version: 1,
                tick,
                correlation_id: None,
                payload: SaleReceiptPayload {
                    command_id: id,
                    market_id: "cradle-exchange".into(),
                    mineral_id: "ferrosite".into(),
                    quantity_kg: q,
                    proceeds_cr: proceeds,
                    price_before_cr_per_kg: 10,
                    price_after_cr_per_kg: after,
                },
            },
        });
    }
    fn inv(&mut self, tick: u64, kg: i64) {
        let f = self.next();
        let items = if kg > 0 {
            vec![InventoryItem {
                mineral_id: "ferrosite".into(),
                quantity_kg: kg,
            }]
        } else {
            vec![]
        };
        self.m.inventory.push(Observed {
            frame_seq: f,
            at_us: 0,
            tick,
            msg: InventoryStateMessage {
                message_id: mid(),
                message_type: INVENTORY_STATE.into(),
                schema_version: 1,
                tick,
                correlation_id: None,
                payload: InventoryStatePayload {
                    actor_id: mid(),
                    items,
                },
            },
        });
    }
    fn wallet(&mut self, tick: u64, bal: i64) {
        let f = self.next();
        self.m.wallets.push(Observed {
            frame_seq: f,
            at_us: 0,
            tick,
            msg: WalletStateMessage {
                message_id: mid(),
                message_type: WALLET_STATE.into(),
                schema_version: 1,
                tick,
                correlation_id: None,
                payload: WalletStatePayload {
                    actor_id: mid(),
                    balance_cr: bal,
                },
            },
        });
    }
    fn market(&mut self, tick: u64, sat: Option<i64>, price: i64, last: Option<(Uuid, i64)>) {
        let f = self.next();
        let listings = sat
            .map(|s| {
                vec![MarketListingState {
                    mineral_id: "ferrosite".into(),
                    saturation_kg: s,
                    price_cr_per_kg: price,
                    last_sale: last.map(|(a, q)| LastSale {
                        actor_id: a,
                        quantity_kg: q,
                        tick,
                    }),
                }]
            })
            .unwrap_or_default();
        self.m.markets.push(Observed {
            frame_seq: f,
            at_us: 0,
            tick,
            msg: MarketStateMessage {
                message_id: mid(),
                message_type: MARKET_STATE.into(),
                schema_version: 1,
                tick,
                correlation_id: None,
                payload: MarketStatePayload {
                    market_id: "cradle-exchange".into(),
                    listings,
                },
            },
        });
    }
    /// 수락된 판매 한 벌(결과 → 영수증 → 인벤토리 → 지갑, 같은 tick).
    fn accepted_sale(
        &mut self,
        id: Uuid,
        tick: u64,
        q: i64,
        proceeds: i64,
        hold_after: i64,
        bal_after: i64,
    ) {
        self.result(id, tick, "ACCEPTED", None);
        self.receipt(id, tick, q, proceeds, 9);
        self.inv(tick, hold_after);
        self.wallet(tick, bal_after);
    }
}

const DOCK: [f64; 3] = [0.0, 1500.0, 200.0];

#[test]
fn single_range_needs_far_position_and_right_reason() {
    let (b0, c1) = (mid(), mid());
    let mut b = B::new();
    b.sent(b0, 200, DOCK, 0.5, "{}");
    b.accepted_sale(b0, 100, 200, 2000, 200, 2000);
    b.sent(c1, 100, [0.0, 2400.0, 0.0], 0.5, "{}");
    b.result(c1, 400, "REJECTED", Some("TARGET_OUT_OF_RANGE"));
    assert_eq!(
        trade::judge_single(SellCase::Range, &b.m, &ex()).0,
        Verdict::Pass
    );
    // 음성: 같은 거절인데 보낸 자리가 도킹 구역 안 → 조건 미발생(사거리 경로를 안 탔다)
    b.m.sell_sent[1].pos_m = Some(DOCK);
    assert_eq!(
        trade::judge_single(SellCase::Range, &b.m, &ex()).0,
        Verdict::Undecided
    );
    // 음성: 먼 자리인데 다른 사유 → FAIL
    b.m.sell_sent[1].pos_m = Some([0.0, 2400.0, 0.0]);
    b.m.results[1].reason_code = Some("COOLDOWN_ACTIVE".into());
    assert_eq!(
        trade::judge_single(SellCase::Range, &b.m, &ex()).0,
        Verdict::Fail
    );
}

#[test]
fn single_baseline_not_accepted_is_undecided() {
    let b0 = mid();
    let mut b = B::new();
    b.sent(b0, 200, DOCK, 0.5, "{}");
    b.result(b0, 100, "REJECTED", Some("TARGET_OUT_OF_RANGE"));
    assert_eq!(
        trade::judge_single(SellCase::OverQty, &b.m, &ex()).0,
        Verdict::Undecided
    );
}

#[test]
fn single_dup_counts_receipts_and_balance() {
    let b0 = mid();
    let mut b = B::new();
    b.wallet(1, 0);
    b.sent(b0, 200, DOCK, 0.5, "{}");
    b.accepted_sale(b0, 100, 200, 2000, 200, 2000);
    b.sent(b0, 200, DOCK, 0.5, "{}");
    b.result(b0, 140, "REJECTED", Some("DUPLICATE_COMMAND_ID"));
    assert_eq!(
        trade::judge_single(SellCase::Dup, &b.m, &ex()).0,
        Verdict::Pass
    );
    // 음성: 두 번 반영(영수증 2·잔액 4000) → FAIL
    b.receipt(b0, 140, 200, 2000, 9);
    b.wallet(140, 4000);
    assert_eq!(
        trade::judge_single(SellCase::Dup, &b.m, &ex()).0,
        Verdict::Fail
    );
}

#[test]
fn single_overqty_condition_requires_positive_holding() {
    let (b0, c1) = (mid(), mid());
    let mut b = B::new();
    b.sent(b0, 200, DOCK, 0.5, "{}");
    b.accepted_sale(b0, 100, 200, 2000, 200, 2000);
    b.sent(c1, 201, DOCK, 0.5, "{}");
    b.result(c1, 140, "REJECTED", Some("INSUFFICIENT_QUANTITY"));
    assert_eq!(
        trade::judge_single(SellCase::OverQty, &b.m, &ex()).0,
        Verdict::Pass
    );
    b.m.sell_sent[1].quantity_kg = 500; // 보유 + 1 이 아니다
    assert_eq!(
        trade::judge_single(SellCase::OverQty, &b.m, &ex()).0,
        Verdict::Undecided
    );
}

#[test]
fn race_needs_same_tick_and_zero_start() {
    let (x, y) = (mid(), mid());
    let mut a = B::new();
    a.market(10, Some(0), 10, None);
    a.sent(x, 2000, DOCK, 0.5, "{}");
    a.accepted_sale(x, 500, 2000, 20_000, 0, 20_000);
    let mut b = B::new();
    b.market(10, Some(0), 10, None);
    b.sent(y, 2000, DOCK, 0.5, "{}");
    b.accepted_sale(y, 500, 2000, 18_000, 0, 18_000);
    assert_eq!(trade::judge_race(&a.m, &b.m).0, Verdict::Pass);
    b.m.receipts[0].tick = 501; // 다른 tick → 무효
    assert_eq!(trade::judge_race(&a.m, &b.m).0, Verdict::Undecided);
    b.m.receipts[0].tick = 500;
    b.m.receipts[0].msg.payload.proceeds_cr = 20_000; // 둘 다 20,000 → FAIL
    assert_eq!(trade::judge_race(&a.m, &b.m).0, Verdict::Fail);
}

#[test]
fn trace_b_reads_first_market_only() {
    let seller = mid();
    let mut b = B::new();
    b.market(900, Some(14_000), 3, Some((seller, 14_000)));
    assert_eq!(trade::judge_trace_b(&b.m, seller).0, Verdict::Pass);
    // 음성: 마지막 거래가 다른 actor
    assert_eq!(trade::judge_trace_b(&b.m, mid()).0, Verdict::Fail);
    // 음성: 시세가 이미 기준가
    let mut c = B::new();
    c.market(900, Some(0), 10, Some((seller, 14_000)));
    assert_eq!(trade::judge_trace_b(&c.m, seller).0, Verdict::Fail);
}

#[test]
fn trace_b_window_gate() {
    use serde_json::json;
    let ok = starfall_bots::trade_trace::judge_b(
        Ok(Some(1000)),
        Ok(Some(1700)),
        Ok(0),
        Some(1800),
        Verdict::Pass,
        json!({}),
    );
    assert_eq!(ok.0, Verdict::Pass);
    // 디스폰 전에 B 접속 → 무효
    let early = starfall_bots::trade_trace::judge_b(
        Ok(Some(1000)),
        Ok(Some(1900)),
        Ok(0),
        Some(1800),
        Verdict::Pass,
        json!({}),
    );
    assert_eq!(early.0, Verdict::Undecided);
    // 1,200 tick 넘김 → 무효
    let late = starfall_bots::trade_trace::judge_b(
        Ok(Some(1000)),
        Ok(Some(1700)),
        Ok(0),
        Some(2201),
        Verdict::Pass,
        json!({}),
    );
    assert_eq!(late.0, Verdict::Undecided);
    // B 의 이전 세션 있음 → 무효
    let prior = starfall_bots::trade_trace::judge_b(
        Ok(Some(1000)),
        Ok(Some(1700)),
        Ok(1),
        Some(1800),
        Verdict::Pass,
        json!({}),
    );
    assert_eq!(prior.0, Verdict::Undecided);
}

#[test]
fn sell_pairing_zero_is_not_ok_and_order_matters() {
    let m = MiningObs::default();
    assert!(!pair_sell_responses(&m, &[]).ok(), "수락 0 → ok 아님");
    let x = mid();
    let mut b = B::new();
    b.accepted_sale(x, 10, 100, 1000, 0, 1000);
    assert!(pair_sell_responses(&b.m, &[x]).ok());
    // 음성: 지갑이 인벤토리보다 먼저 → 순서 위반
    let mut c = B::new();
    c.result(x, 10, "ACCEPTED", None);
    c.receipt(x, 10, 100, 1000, 9);
    c.wallet(10, 1000);
    c.inv(10, 0);
    assert!(!pair_sell_responses(&c.m, &[x]).ok());
}

/// 세션 시작 순서(SC-47) — `SESSION_READY` 도 첫 타입 목록에 들어가야 한다(첫 실서버 실행에서 빠져 있어
/// 거짓 FAIL 이 났다 — 2026-10-07). 판정 함수가 다섯 타입을 정확히 요구하는지 양쪽으로 본다.
#[test]
fn session_start_order_requires_session_ready_first() {
    let mut a = MiningObs::default();
    for (i, t) in [
        SESSION_READY,
        INVENTORY_STATE,
        WALLET_STATE,
        DEPOSIT_FIELD_STATE,
        MARKET_STATE,
    ]
    .iter()
    .enumerate()
    {
        a.note_type(i as u64, t);
    }
    let starts: Vec<&str> = a.first_types.iter().map(|(_, t)| *t).collect();
    assert_eq!(starts, trade::SESSION_START_ORDER);
    let mut b = MiningObs::default();
    for (i, t) in [
        INVENTORY_STATE,
        WALLET_STATE,
        DEPOSIT_FIELD_STATE,
        MARKET_STATE,
        WORLD_SNAPSHOT,
    ]
    .iter()
    .enumerate()
    {
        b.note_type(i as u64, t);
    }
    let starts_b: Vec<&str> = b.first_types.iter().map(|(_, t)| *t).collect();
    assert_ne!(
        starts_b,
        trade::SESSION_START_ORDER,
        "SESSION_READY 가 빠진 목록은 순서 일치가 아니다"
    );
}
