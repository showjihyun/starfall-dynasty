//! p1-03 계약 거래 케이스 — 행동 조립과 **판정**(계약 SC-45~47 · 106~116 · 121 · 132).
//!
//! 판정 입력은 봇이 받은 메시지뿐이다(`MiningObs`). 판정 함수는 순수 함수이고, 각 함수는 계약 ⊘ 칸의
//! "조건이 실제로 일어났다" 단언을 **먼저** 하고 분모를 찍는다. 조건이 일어나지 않았으면 PASS 가 아니라
//! `미검증(조건 미발생)` 이다(도구 exit 4).
//!
//! 기준값(도킹 320 m, 속도 10 m/s, 판매 쿨다운 20 tick, 대금 표)은 **계약 §0.6·스펙 문구**에서 옮긴 상수다.
//! `data/` 는 길 찾기(거래소·광맥 위치)에만 쓴다 — 기대값이 아니다.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::conn::{
    Behavior, BotSpec, Clock, ConnectionOutcome, FlyMode, MineStep, SellQty, run_connection,
};
use crate::mine_cases::Verdict;
use crate::mine_run::{DepositSite, find_deposit};
use crate::mining::{MiningObs, pair_sell_responses};
use crate::token;

/// 계약 §0.6 / 디자인 §1.3 — 도킹 구역 반지름(120 + 200)과 속도 상한. 판정 근거 출력용.
pub const DOCK_RADIUS_M: f64 = 320.0;
pub const MAX_DOCK_SPEED_MPS: f64 = 10.0;
/// 판매 쿨다운 1 s × 20 Hz.
pub const SELL_COOLDOWN_TICKS: u64 = 20;
/// 회복 배수(60 s × 20 Hz) — 흔적 창(SC-114).
pub const RECOVERY_INTERVAL_TICKS: u64 = 1200;
/// 디자인 §3.6 — Ferrosite 기준가·바닥가, 포화 상한. 대금 표 P2·P9·P10(계약 §0.6).
pub const FERROSITE_BASE: i64 = 10;
pub const FERROSITE_FLOOR: i64 = 3;
pub const P2_14000_PROCEEDS: i64 = 98_000;
pub const P9_FIRST_2000: i64 = 20_000;
pub const P10_SECOND_2000: i64 = 18_000;

pub const FERROSITE: &str = "ferrosite";
pub const GLACINE: &str = "glacine";
pub const STARFALL_GLASS: &str = "starfall-glass";
/// 형식은 유효한(케밥) 없는 id — 스키마 위반으로 `MALFORMED_COMMAND` 가 먼저 나지 않게(SC-111).
pub const UNKNOWN_MARKET_ID: &str = "qa-no-such-market";
pub const UNKNOWN_MINERAL_ID: &str = "qa-no-such-mineral";

/// 채굴 간격 — 채굴 쿨다운 3 s 보다 조금 길게(쿨다운 거절을 만들지 않으려고).
pub const MINE_EVERY: Duration = Duration::from_millis(3150);

/// 거래소 — 길 찾기 입력.
#[derive(Debug, Clone)]
pub struct Exchange {
    pub id: String,
    pub position_m: [f64; 3],
    pub radius_m: f64,
    pub dock_range_m: f64,
}

impl Exchange {
    /// 도킹 구역 안쪽 정지 반경 — 경계에서 충분히 안(브레이크 여유).
    pub fn stop_radius(&self) -> f64 {
        (self.radius_m + self.dock_range_m) - 70.0
    }
}

/// `data/market/*.json` 의 첫 시장(이번 슬라이스는 하나뿐).
pub fn find_exchange(data_dir: &Path) -> Result<Exchange, String> {
    let dir = data_dir.join("market");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let path = files
        .first()
        .ok_or_else(|| format!("{}: 시장 파일 없음", dir.display()))?;
    let v: Value = serde_json::from_str(
        &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let p = v["position_m"].as_array().ok_or("position_m 없음")?;
    let f = |i: usize| p.get(i).and_then(Value::as_f64).unwrap_or(0.0);
    Ok(Exchange {
        id: v["id"].as_str().ok_or("id 없음")?.to_owned(),
        position_m: [f(0), f(1), f(2)],
        radius_m: v["radius_m"].as_f64().ok_or("radius_m 없음")?,
        dock_range_m: v["rules"]["dock_range_from_surface_m"]
            .as_f64()
            .ok_or("dock_range 없음")?,
    })
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

// ── 단계 조립 ────────────────────────────────────────────────────────────────

/// 광맥에서 `kg` 만큼 캐고 거래소로 가서 도킹한다(판매는 호출자가 붙인다).
pub fn mine_and_dock(site: &DepositSite, ex: &Exchange, mineral: &str, kg: i64) -> Vec<MineStep> {
    vec![
        MineStep::MineUntil {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            mineral_id: mineral.to_owned(),
            kg,
            every: MINE_EVERY,
            timeout: Duration::from_secs(600),
        },
        MineStep::GoTo {
            target_m: ex.position_m,
            stop_radius_m: ex.stop_radius(),
            timeout: Duration::from_secs(240),
        },
        MineStep::Wait(Duration::from_millis(600)),
    ]
}

/// 광맥으로 다시 간다.
pub fn back_to(site: &DepositSite) -> MineStep {
    MineStep::GoTo {
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        timeout: Duration::from_secs(240),
    }
}

fn sell(ex: &Exchange, mineral: &str, qty: SellQty) -> MineStep {
    MineStep::Sell {
        market_id: ex.id.clone(),
        mineral_id: mineral.to_owned(),
        qty,
    }
}

fn script(site: &DepositSite, steps: Vec<MineStep>) -> Behavior {
    Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        fly: FlyMode::Stop {
            timeout: Duration::from_secs(300),
        },
        steps,
        grace: Duration::from_secs(2),
    }
}

fn spec(
    secret: &str,
    url: &str,
    label: &str,
    clock: Clock,
    behavior: Behavior,
    raw: bool,
) -> BotSpec {
    let (_s, tok) = token::identity(secret, label);
    BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior,
        clock,
        live_corr: None,
        capture_raw: raw,
    }
}

/// 치트 케이스 공통 앞부분(계약 §1-R 머리): 400 kg 채굴 → 도킹 → 200 kg 정상 판매(수락) → 쿨다운 넘김.
/// 상태를 0 아닌 값으로 만들어 "불변" 이 공짜가 되지 않게 한다.
pub fn baseline(site: &DepositSite, ex: &Exchange) -> Vec<MineStep> {
    let mut s = mine_and_dock(site, ex, FERROSITE, 400);
    s.push(sell(ex, FERROSITE, SellQty::Kg(200)));
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    s
}

/// 판매 주입 반례 7 경우(계약 SC-106) — 계약 반례 파일 이름.
pub const SELL_INJECT_FIXTURES: [(&str, &str); 7] = [
    ("price-field-injected.json", "price_cr_per_kg"),
    ("proceeds-field-injected.json", "proceeds_cr"),
    ("balance-field-injected.json", "balance_cr"),
    ("position-field-injected.json", "position_m"),
    ("actor-field-injected.json", "actor_id"),
    ("quantity-zero.json", "quantity_kg"),
    ("quantity-fractional.json", "quantity_kg"),
];

/// 계약 반례 원문 템플릿 — `command_id` 만 `{COMMAND_ID}` 로. 주입 필드는 그대로(봇 타입을 거치면
/// 어휘 밖 필드를 시험할 수 없다).
pub fn sell_inject_templates(contracts_dir: &Path) -> Result<Vec<String>, String> {
    let dir = contracts_dir
        .join("fixtures")
        .join("SELL_MINERAL")
        .join("invalid");
    SELL_INJECT_FIXTURES
        .iter()
        .map(|(file, _)| {
            let path = dir.join(file);
            let text =
                std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut v: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            v["command_id"] = json!("{COMMAND_ID}");
            serde_json::to_string(&v).map_err(|e| e.to_string())
        })
        .collect()
}

// ── 공통 사실 ─────────────────────────────────────────────────────────────────

/// 판매 1건의 결과 요약.
fn sell_facts(m: &MiningObs, ex: &Exchange) -> Vec<Value> {
    m.sell_sent
        .iter()
        .map(|s| {
            let r = m.results_of(s.command_id);
            json!({
                "command_id": s.command_id, "mineral_id": s.mineral_id, "quantity_kg": s.quantity_kg,
                "results": r.iter().map(|x| json!({"status": x.status, "reason": x.reason_code, "tick": x.tick})).collect::<Vec<_>>(),
                "snapshot_tick": s.last_snapshot_tick,
                "dist_to_exchange_m": s.pos_m.map(|p| dist(p, ex.position_m)),
                "speed_mps": s.speed_mps,
            })
        })
        .collect()
}

fn accepted_sells(m: &MiningObs) -> Vec<Uuid> {
    m.sell_sent
        .iter()
        .filter(|s| {
            m.results_of(s.command_id)
                .iter()
                .any(|r| r.status == "ACCEPTED")
        })
        .map(|s| s.command_id)
        .collect()
}

fn reason_of(m: &MiningObs, id: Uuid) -> Option<String> {
    m.results_of(id).last().and_then(|r| r.reason_code.clone())
}

/// 기준 판매 뒤로 상태가 바뀌지 않았는가 — 마지막 수락 판매 결과 뒤에 온 `WALLET_STATE`·`SALE_RECEIPT` 수
/// (응답 묶음 1 벌은 허용), 그리고 보유량.
fn state_frozen_after(m: &MiningObs, base_id: Uuid) -> (bool, Value) {
    let Some(base) = m.result_of(base_id) else {
        return (false, json!({"why": "기준 판매 결과 없음"}));
    };
    let wallets_after = m
        .wallets
        .iter()
        .filter(|w| w.frame_seq > base.frame_seq)
        .count();
    let receipts_after = m
        .receipts
        .iter()
        .filter(|r| r.frame_seq > base.frame_seq)
        .count();
    let frozen = wallets_after <= 1 && receipts_after <= 1;
    (
        frozen,
        json!({"wallet_messages_after_baseline": wallets_after, "receipts_after_baseline": receipts_after,
               "allowed": "기준 판매의 응답 1 벌", "final_balance": m.balance(), "final_ferrosite": m.holding(FERROSITE)}),
    )
}

// ── 치트·중복 케이스 (단일 봇) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellCase {
    Inject,
    Range,
    Fast,
    OverQty,
    Cooldown,
    Unknown,
    Dup,
    CrossKind,
    Once,
}

pub fn behavior(case: SellCase, site: &DepositSite, ex: &Exchange, inject: &[String]) -> Behavior {
    let mut s = baseline(site, ex);
    match case {
        SellCase::Inject => {
            for t in inject {
                s.push(MineStep::SellRaw(t.clone()));
                s.push(MineStep::Wait(Duration::from_millis(400)));
            }
        }
        SellCase::Range => {
            // 도킹 구역 밖 500 m — 거래소 중심에서 820 m 넘게(위쪽으로 900 m).
            let p = ex.position_m;
            s.push(MineStep::GoTo {
                target_m: [p[0], p[1] + 900.0, p[2]],
                stop_radius_m: 40.0,
                timeout: Duration::from_secs(120),
            });
            s.push(sell(ex, FERROSITE, SellQty::Kg(100)));
        }
        SellCase::Fast => {
            let p = ex.position_m;
            s.push(MineStep::GoTo {
                target_m: [p[0], p[1] + 2000.0, p[2]],
                stop_radius_m: 60.0,
                timeout: Duration::from_secs(150),
            });
            s.push(MineStep::PassThroughTo {
                target_m: p,
                radius_m: ex.radius_m + 100.0,
                speed_mps: 30.0,
                timeout: Duration::from_secs(150),
            });
            s.push(sell(ex, FERROSITE, SellQty::Kg(100)));
        }
        SellCase::OverQty => s.push(sell(ex, FERROSITE, SellQty::HoldingPlus(1))),
        SellCase::Cooldown => {
            s.push(sell(ex, FERROSITE, SellQty::Kg(50)));
            s.push(MineStep::Wait(Duration::from_millis(500)));
            s.push(sell(ex, FERROSITE, SellQty::Kg(50)));
            s.push(MineStep::Wait(Duration::from_millis(1500)));
            s.push(sell(ex, FERROSITE, SellQty::Kg(50)));
        }
        SellCase::Unknown => {
            s.push(MineStep::Sell {
                market_id: UNKNOWN_MARKET_ID.to_owned(),
                mineral_id: FERROSITE.to_owned(),
                qty: SellQty::Kg(50),
            });
            s.push(MineStep::Wait(Duration::from_millis(1200)));
            s.push(MineStep::Sell {
                market_id: ex.id.clone(),
                mineral_id: UNKNOWN_MINERAL_ID.to_owned(),
                qty: SellQty::Kg(50),
            });
        }
        SellCase::Dup => s.push(MineStep::SellResend(0)),
        SellCase::CrossKind => {
            // 채굴로 수락된 키(첫 MineUntil 의 첫 채굴 id 는 스크립트 밖에서 모른다) 대신, 정해 둔 키 둘을 쓴다.
            // 판매 키 → 채굴: 기준 판매(sell_sent[0])의 id 를 채굴로. 채굴 키 → 판매: run_cross_kind 가 따로 조립한다.
        }
        SellCase::Once => {}
    }
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    script(site, s)
}

/// 단일 봇 판매 케이스 실행 + 판정.
pub async fn run_single(
    case: SellCase,
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
    contracts_dir: &Path,
) -> (Verdict, Value) {
    let inject = if case == SellCase::Inject {
        match sell_inject_templates(contracts_dir) {
            Ok(t) => t,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        }
    } else {
        Vec::new()
    };
    if case == SellCase::CrossKind {
        return run_cross_kind(site, ex, url, secret, label).await;
    }
    let out = run_connection(spec(
        secret,
        url,
        label,
        Clock::start(),
        behavior(case, site, ex, &inject),
        false,
    ))
    .await;
    let (v, mut d) = judge_single(case, &out.mining, ex);
    d["label"] = json!(label);
    d["sells"] = json!(sell_facts(&out.mining, ex));
    d["goto_failures"] = json!(out.mining.goto_failures);
    d["mine_until_shortfalls"] = json!(out.mining.mine_until_shortfalls);
    d["actor_id"] = json!(out.ledger.session.as_ref().map(|s| s.actor_id));
    // 조건 미발생의 원인을 같은 실행에서 볼 수 있게 — 접속 실패·닫힘 사유·봇 오류.
    let fin = out.ledger.finish();
    d["connect_error"] = json!(out.connect_error);
    d["close"] = json!(out.ledger.session.as_ref().map(|s| json!({
        "initiator": s.close_initiator, "code": s.close_code, "reason": s.close_reason_text})));
    d["bot_errors"] = json!(fin.errors);
    d["wire_errors"] = json!(fin.wire_errors);
    (v, d)
}

/// 단일 봇 판정 — 순수 함수(테스트가 합성 관측으로 부른다).
pub fn judge_single(case: SellCase, m: &MiningObs, ex: &Exchange) -> (Verdict, Value) {
    let Some(base) = m.sell_sent.first() else {
        return (
            Verdict::Undecided,
            json!({"why": "판매를 하나도 보내지 못했다(도킹 실패?)"}),
        );
    };
    let base_ok = m
        .results_of(base.command_id)
        .iter()
        .any(|r| r.status == "ACCEPTED");
    if !base_ok {
        return (
            Verdict::Undecided,
            json!({"why": "⊘ 기준 판매가 수락되지 않았다 — 상태가 0 이라 '불변' 이 공짜", "base_reason": reason_of(m, base.command_id)}),
        );
    }
    let rest: Vec<_> = m.sell_sent.iter().skip(1).collect();
    let (frozen, fz) = state_frozen_after(m, base.command_id);
    let reasons: Vec<Option<String>> = rest.iter().map(|s| reason_of(m, s.command_id)).collect();
    let want = |r: &str| reasons.iter().filter(|x| x.as_deref() == Some(r)).count();
    let mut d = json!({"case": format!("{case:?}"), "baseline_accepted": true, "cheat_reasons": reasons, "state": fz});
    let (cond, pass) = match case {
        SellCase::Inject => {
            // ⊘: 보낸 원문에 주입 필드가 실제로 있다.
            let carried = rest
                .iter()
                .zip(SELL_INJECT_FIXTURES.iter())
                .filter(|(s, (_, key))| s.text.contains(&format!("\"{key}\"")))
                .count();
            d["attempts"] = json!(rest.len());
            d["frames_carrying_injected_key"] = json!(carried);
            let n = want("MALFORMED_COMMAND");
            d["blocked_malformed"] = json!(n);
            (rest.len() == 7 && carried == 7, n == 7 && frozen)
        }
        SellCase::Range => {
            let s = rest.first();
            let far = s.and_then(|s| s.pos_m).map(|p| dist(p, ex.position_m));
            d["dist_at_send_m"] = json!(far);
            d["speed_at_send_mps"] = json!(s.and_then(|s| s.speed_mps));
            let cond = far.is_some_and(|f| f > DOCK_RADIUS_M + 100.0)
                && s.and_then(|s| s.speed_mps)
                    .is_some_and(|v| v <= MAX_DOCK_SPEED_MPS);
            (cond, want("TARGET_OUT_OF_RANGE") == 1 && frozen)
        }
        SellCase::Fast => {
            let s = rest.first();
            let near = s.and_then(|s| s.pos_m).map(|p| dist(p, ex.position_m));
            let v = s.and_then(|s| s.speed_mps);
            d["dist_at_send_m"] = json!(near);
            d["speed_at_send_mps"] = json!(v);
            let cond = near.is_some_and(|f| f <= DOCK_RADIUS_M)
                && v.is_some_and(|v| v > MAX_DOCK_SPEED_MPS * 1.5);
            (cond, want("SHIP_TOO_FAST") == 1 && frozen)
        }
        SellCase::OverQty => {
            let s = rest.first();
            let held = m
                .inventory
                .iter()
                .rev()
                .find(|o| s.is_some_and(|s| o.frame_seq < s.frame_seq_at_send))
                .and_then(|o| {
                    o.msg
                        .payload
                        .items
                        .iter()
                        .find(|i| i.mineral_id == FERROSITE)
                })
                .map_or(0, |i| i.quantity_kg);
            d["holding_before_kg"] = json!(held);
            d["requested_kg"] = json!(s.map(|s| s.quantity_kg));
            (
                held > 0 && s.is_some_and(|s| s.quantity_kg == held + 1),
                want("INSUFFICIENT_QUANTITY") == 1 && frozen,
            )
        }
        SellCase::Cooldown => {
            // rest = [50 수락, 0.5 s 뒤 50 거부, 1.5 s 뒤 50 수락]
            let ok_first = rest.first().is_some_and(|s| {
                m.results_of(s.command_id)
                    .iter()
                    .any(|r| r.status == "ACCEPTED")
            });
            let r2 = rest.get(1).and_then(|s| m.result_of(s.command_id));
            let r1 = rest.first().and_then(|s| m.result_of(s.command_id));
            let gap = match (r1, r2) {
                (Some(a), Some(b)) => Some(b.tick.saturating_sub(a.tick)),
                _ => None,
            };
            let third_ok = rest.get(2).is_some_and(|s| {
                m.results_of(s.command_id)
                    .iter()
                    .any(|r| r.status == "ACCEPTED")
            });
            d["tick_gap_first_to_retry"] = json!(gap);
            d["accepted_after_cooldown"] = json!(third_ok);
            d["open_at_end"] = json!(m.open_at_end);
            let cond = ok_first && gap.is_some_and(|g| g < SELL_COOLDOWN_TICKS);
            (
                cond,
                want("COOLDOWN_ACTIVE") == 1 && third_ok && m.open_at_end == Some(true),
            )
        }
        SellCase::Unknown => {
            let two = rest.len() == 2;
            d["attempts"] = json!(rest.len());
            (two, want("TARGET_UNKNOWN") == 2 && frozen)
        }
        SellCase::Dup => {
            let resent = rest
                .first()
                .is_some_and(|s| s.command_id == base.command_id);
            let receipts = m.receipts.len();
            let proceeds: i64 = m.receipts.iter().map(|r| r.msg.payload.proceeds_cr).sum();
            d["resent_same_id"] = json!(resent);
            d["receipts"] = json!(receipts);
            d["final_balance"] = json!(m.balance());
            d["sum_proceeds"] = json!(proceeds);
            (
                resent,
                want("DUPLICATE_COMMAND_ID") == 1 && receipts == 1 && m.balance() == Some(proceeds),
            )
        }
        SellCase::CrossKind | SellCase::Once => (true, true),
    };
    d["condition_occurred"] = json!(cond);
    if !cond {
        return (Verdict::Undecided, d);
    }
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

/// SC-112 명령 종류 교차: 채굴 키 X → 판매로, 판매 키 Y → 채굴로. 둘 다 `DUPLICATE_COMMAND_ID`.
pub async fn run_cross_kind(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let (x, y) = (Uuid::now_v7(), Uuid::now_v7());
    let mut s = vec![
        MineStep::MineId(x),
        MineStep::Wait(Duration::from_millis(3300)),
    ];
    s.extend(mine_and_dock(site, ex, FERROSITE, 400));
    s.push(MineStep::SellId {
        id: x,
        market_id: ex.id.clone(),
        mineral_id: FERROSITE.to_owned(),
        qty: SellQty::Kg(100),
    });
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    s.push(MineStep::SellId {
        id: y,
        market_id: ex.id.clone(),
        mineral_id: FERROSITE.to_owned(),
        qty: SellQty::Kg(100),
    });
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    s.push(back_to(site));
    s.push(MineStep::Wait(Duration::from_millis(3300)));
    s.push(MineStep::MineId(y));
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    let out = run_connection(spec(
        secret,
        url,
        label,
        Clock::start(),
        script(site, s),
        false,
    ))
    .await;
    let (v, mut d) = judge_cross_kind(&out.mining, x, y);
    d["sells"] = json!(sell_facts(&out.mining, ex));
    (v, d)
}

pub fn judge_cross_kind(m: &MiningObs, x: Uuid, y: Uuid) -> (Verdict, Value) {
    let statuses = |id: Uuid| -> Vec<(String, Option<String>)> {
        m.results_of(id)
            .iter()
            .map(|r| (r.status.clone(), r.reason_code.clone()))
            .collect()
    };
    let (sx, sy) = (statuses(x), statuses(y));
    let d = json!({"mine_key_x": x, "results_x": sx, "sell_key_y": y, "results_y": sy});
    // ⊘: 첫 사용이 수락(채굴 X 수락 · 판매 Y 수락).
    let first_ok = |v: &[(String, Option<String>)]| v.first().is_some_and(|r| r.0 == "ACCEPTED");
    if !(first_ok(&sx) && first_ok(&sy)) || sx.len() < 2 || sy.len() < 2 {
        return (Verdict::Undecided, d);
    }
    let dup = |v: &[(String, Option<String>)]| {
        v.get(1)
            .is_some_and(|r| r.1.as_deref() == Some("DUPLICATE_COMMAND_ID"))
    };
    (
        if dup(&sx) && dup(&sy) {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        d,
    )
}

/// SC-112 (b) 잔류 창 안 재접속: 연결 1 에서 판매 수락 → 닫음 → 같은 신원 연결 2 에서 같은 원문 재전송.
pub async fn run_reconnect(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let o1 = run_connection(spec(
        secret,
        url,
        label,
        clock,
        script(site, baseline(site, ex)),
        false,
    ))
    .await;
    let Some(first) = o1.mining.sell_sent.first().cloned() else {
        return (
            Verdict::Undecided,
            json!({"why": "연결 1 에서 판매를 보내지 못했다"}),
        );
    };
    let steps = vec![
        MineStep::Wait(Duration::from_millis(500)),
        MineStep::SellId {
            id: first.command_id,
            market_id: first.market_id.clone(),
            mineral_id: first.mineral_id.clone(),
            qty: SellQty::Kg(first.quantity_kg),
        },
        MineStep::Wait(Duration::from_millis(1500)),
    ];
    let beh = Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: 1.0,
        fly: FlyMode::Stay,
        steps,
        grace: Duration::from_secs(1),
    };
    let o2 = run_connection(spec(secret, url, label, clock, beh, false)).await;
    judge_reconnect(&o1, &o2, first.command_id)
}

pub fn judge_reconnect(
    o1: &ConnectionOutcome,
    o2: &ConnectionOutcome,
    id: Uuid,
) -> (Verdict, Value) {
    let acc1 = o1
        .mining
        .results_of(id)
        .iter()
        .any(|r| r.status == "ACCEPTED");
    let r2 = o2
        .mining
        .results_of(id)
        .first()
        .map(|r| r.reason_code.clone());
    let same_ship =
        o1.mining.own_ship_id.is_some() && o1.mining.own_ship_id == o2.mining.own_ship_id;
    let start_balance2 = o2.mining.wallets.first().map(|w| w.msg.payload.balance_cr);
    let end_balance1 = o1.mining.balance();
    let d = json!({"accepted_first": acc1, "second_reason": r2, "same_ship_resumed": same_ship,
                   "balance_end_conn1": end_balance1, "balance_start_conn2": start_balance2,
                   "balance_end_conn2": o2.mining.balance(), "receipts_conn2": o2.mining.receipts.len()});
    if !acc1 || !same_ship {
        return (Verdict::Undecided, d);
    }
    let pass = r2 == Some(Some("DUPLICATE_COMMAND_ID".to_owned()))
        && o2.mining.receipts.is_empty()
        && start_balance2 == end_balance1
        && o2.mining.balance() == end_balance1;
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ── 흔적 (AC-19) ──────────────────────────────────────────────────────────────

/// A: Ferrosite 14,000 kg 을 모아 **한 번에** 판다 → 접속 종료. 광맥 둘(IB1 → IB3).
pub async fn run_trace_a(
    ib1: &DepositSite,
    ib3: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let mut s = vec![MineStep::MineUntil {
        deposit_id: ib1.id.clone(),
        target_m: ib1.position_m,
        mineral_id: FERROSITE.to_owned(),
        kg: 12_000,
        every: MINE_EVERY,
        timeout: Duration::from_secs(420),
    }];
    s.push(back_to(ib3));
    s.extend(mine_and_dock(ib3, ex, FERROSITE, 14_000));
    s.push(sell(ex, FERROSITE, SellQty::Kg(14_000)));
    s.push(MineStep::Wait(Duration::from_millis(1500)));
    let out = run_connection(spec(
        secret,
        url,
        label,
        Clock::start(),
        script(ib1, s),
        false,
    ))
    .await;
    let (v, mut d) = judge_trace_a(&out.mining);
    d["actor_id"] = json!(out.ledger.session.as_ref().map(|s| s.actor_id));
    d["sells"] = json!(sell_facts(&out.mining, ex));
    (v, d)
}

pub fn judge_trace_a(m: &MiningObs) -> (Verdict, Value) {
    let Some(s) = m.sell_sent.iter().find(|s| s.quantity_kg == 14_000) else {
        return (
            Verdict::Undecided,
            json!({"why": "14,000 kg 판매를 보내지 못했다(채굴·도킹 실패)"}),
        );
    };
    let pre = m
        .markets
        .iter()
        .rev()
        .find(|o| o.frame_seq < s.frame_seq_at_send)
        .and_then(|o| o.msg.listing(FERROSITE).map(|l| l.saturation_kg));
    let rc = m
        .receipts
        .iter()
        .find(|r| r.msg.payload.command_id == s.command_id);
    let d = json!({"saturation_before_send": pre, "receipt": rc.map(|r| json!({
        "proceeds_cr": r.msg.payload.proceeds_cr, "price_before": r.msg.payload.price_before_cr_per_kg,
        "price_after": r.msg.payload.price_after_cr_per_kg, "tick": r.tick}))});
    // ⊘: 판매 직전 포화 0(새 월드).
    if pre != Some(0) {
        return (Verdict::Undecided, d);
    }
    let pass = rc.is_some_and(|r| {
        r.msg.payload.price_after_cr_per_kg == FERROSITE_FLOOR
            && r.msg.payload.proceeds_cr == P2_14000_PROCEEDS
    });
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

/// B: 별도 프로세스. 접속해서 **첫** `MARKET_STATE` 만 본다. 판정 입력은 B 가 받은 메시지뿐.
pub async fn run_trace_b(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label: &str,
) -> ConnectionOutcome {
    let beh = Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: 1.0,
        fly: FlyMode::Stay,
        steps: vec![MineStep::Wait(Duration::from_secs(2))],
        grace: Duration::from_millis(500),
    };
    run_connection(spec(secret, url, label, Clock::start(), beh, false)).await
}

/// SC-115: B 의 첫 시장 상태 — 시세 < 기준가, `last_sale.actor_id = A`, 14,000 kg.
pub fn judge_trace_b(m: &MiningObs, seller: Uuid) -> (Verdict, Value) {
    let Some(first) = m.markets.first() else {
        return (
            Verdict::Undecided,
            json!({"why": "MARKET_STATE 를 받지 못했다"}),
        );
    };
    let l = first.msg.listing(FERROSITE);
    let d = json!({"first_market_tick": first.tick, "ferrosite": l.map(|l| json!({
        "price": l.price_cr_per_kg, "saturation": l.saturation_kg,
        "last_sale": l.last_sale.as_ref().map(|s| json!({"actor_id": s.actor_id, "quantity_kg": s.quantity_kg, "tick": s.tick}))}))});
    let Some(l) = l else {
        return (Verdict::Fail, d);
    };
    let pass = l.price_cr_per_kg < FERROSITE_BASE
        && l.last_sale
            .as_ref()
            .is_some_and(|s| s.actor_id == seller && s.quantity_kg == 14_000);
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ── 같은 tick 경쟁 (AC-20(a)) ──────────────────────────────────────────────────

/// 두 봇이 각자 2,000 kg 을 들고 도킹한 뒤 코디네이터 신호로 **동시에** 판다.
pub async fn run_race(
    a_site: &DepositSite,
    b_site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label_a: &str,
    label_b: &str,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let (arr_a, arr_b) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let (go_a, go_b) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
    let beh = |site: &DepositSite, arr: Arc<Notify>, go: Arc<Notify>| {
        let mut s = mine_and_dock(site, ex, FERROSITE, 2000);
        s.push(MineStep::Signal(arr));
        s.push(MineStep::WaitSignal {
            notify: go,
            timeout: Duration::from_secs(600),
        });
        s.push(sell(ex, FERROSITE, SellQty::Kg(2000)));
        s.push(MineStep::Wait(Duration::from_secs(2)));
        script(site, s)
    };
    let coordinator = {
        let (arr_a, arr_b, go_a, go_b) = (arr_a.clone(), arr_b.clone(), go_a.clone(), go_b.clone());
        async move {
            let both = async {
                arr_a.notified().await;
                arr_b.notified().await;
            };
            let _ = tokio::time::timeout(Duration::from_secs(700), both).await;
            // 두 신호를 연달아 — 두 봇의 전송 사이 간격은 스케줄링 지연뿐이다(p1-02 race 와 같다).
            go_a.notify_one();
            go_b.notify_one();
        }
    };
    let (oa, ob, ()) = tokio::join!(
        run_connection(spec(
            secret,
            url,
            label_a,
            clock,
            beh(a_site, arr_a.clone(), go_a.clone()),
            false
        )),
        run_connection(spec(
            secret,
            url,
            label_b,
            clock,
            beh(b_site, arr_b.clone(), go_b.clone()),
            false
        )),
        coordinator
    );
    let (v, mut d) = judge_race(&oa.mining, &ob.mining);
    d["sells_a"] = json!(sell_facts(&oa.mining, ex));
    d["sells_b"] = json!(sell_facts(&ob.mining, ex));
    (v, d)
}

/// SC-116: 같은 tick 이면 sequence 작은 쪽 20,000 · 큰 쪽 18,000. 봇은 sequence 를 모른다 —
/// 같은 tick 두 대금이 {20,000, 18,000} 이고 지갑 합 = 38,000 이면 PASS. sequence 대조는 SQL(판정 보조)로.
pub fn judge_race(a: &MiningObs, b: &MiningObs) -> (Verdict, Value) {
    let ra = a
        .receipts
        .iter()
        .find(|r| r.msg.payload.quantity_kg == 2000);
    let rb = b
        .receipts
        .iter()
        .find(|r| r.msg.payload.quantity_kg == 2000);
    let pre_zero = |m: &MiningObs| {
        m.sell_sent.first().and_then(|s| {
            m.markets
                .iter()
                .rev()
                .find(|o| o.frame_seq < s.frame_seq_at_send)
                .and_then(|o| o.msg.listing(FERROSITE).map(|l| l.saturation_kg))
        })
    };
    let d = json!({
        "tick_a": ra.map(|r| r.tick), "tick_b": rb.map(|r| r.tick),
        "proceeds_a": ra.map(|r| r.msg.payload.proceeds_cr), "proceeds_b": rb.map(|r| r.msg.payload.proceeds_cr),
        "saturation_seen_before_a": pre_zero(a), "saturation_seen_before_b": pre_zero(b),
        "wallet_a": a.balance(), "wallet_b": b.balance()});
    let (Some(ra), Some(rb)) = (ra, rb) else {
        return (Verdict::Undecided, d);
    };
    // ⊘: 같은 tick + 시작 포화 0.
    if ra.tick != rb.tick || pre_zero(a) != Some(0) || pre_zero(b) != Some(0) {
        return (Verdict::Undecided, d);
    }
    let mut p = [ra.msg.payload.proceeds_cr, rb.msg.payload.proceeds_cr];
    p.sort_unstable();
    let pass = p == [P10_SECOND_2000, P9_FIRST_2000]
        && a.balance()
            .zip(b.balance())
            .is_some_and(|(x, y)| x + y == P9_FIRST_2000 + P10_SECOND_2000);
    (if pass { Verdict::Pass } else { Verdict::Fail }, d)
}

// ── 누출·상장·세션 시작 (AC-10) ─────────────────────────────────────────────────

/// A·B 접속 → (a) 첫 채굴 전 상장 0 → A 가 Ferrosite 첫 채굴 → (b) 두 세션 모두 그 tick 에 ferrosite 상장,
/// 다른 광물 없음 → A 가 Far Reach 에서 Starfall Glass 첫 채굴 → 두 세션 모두 glass 상장.
pub async fn run_leak_market(
    ib1: &DepositSite,
    far: &DepositSite,
    url: &str,
    secret: &str,
    label_a: &str,
    label_b: &str,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let done = Arc::new(Notify::new());
    let a_steps = vec![
        MineStep::Wait(Duration::from_secs(2)),
        MineStep::Mine,
        MineStep::Wait(Duration::from_secs(3)),
        back_to(far),
        MineStep::MineUntil {
            deposit_id: far.id.clone(),
            target_m: far.position_m,
            mineral_id: STARFALL_GLASS.to_owned(),
            kg: 25,
            every: MINE_EVERY,
            timeout: Duration::from_secs(60),
        },
        MineStep::Wait(Duration::from_secs(2)),
        MineStep::Signal(done.clone()),
    ];
    let b_beh = Behavior::MineScript {
        deposit_id: ib1.id.clone(),
        target_m: ib1.position_m,
        stop_radius_m: 1.0,
        fly: FlyMode::Stay,
        steps: vec![MineStep::WaitSignal {
            notify: done.clone(),
            timeout: Duration::from_secs(900),
        }],
        grace: Duration::from_secs(1),
    };
    let (oa, ob) = tokio::join!(
        run_connection(spec(
            secret,
            url,
            label_a,
            clock,
            script(ib1, a_steps),
            true
        )),
        run_connection(spec(secret, url, label_b, clock, b_beh, true)),
    );
    judge_leak_market(&oa.mining, &ob.mining)
}

/// 세션 시작 순서(스펙 §5.1a).
pub const SESSION_START_ORDER: [&str; 5] = [
    "SESSION_READY",
    "INVENTORY_STATE",
    "WALLET_STATE",
    "DEPOSIT_FIELD_STATE",
    "MARKET_STATE",
];

/// 첫 채굴(수락) 결과의 tick.
fn first_accept_tick(m: &MiningObs, mineral: &str) -> Option<u64> {
    let ids: Vec<Uuid> = m.mine_sent.iter().map(|s| s.command_id).collect();
    m.results
        .iter()
        .filter(|r| r.status == "ACCEPTED" && ids.contains(&r.command_id))
        .find_map(|r| {
            m.inventory
                .iter()
                .find(|i| {
                    i.tick == r.tick && i.msg.payload.items.iter().any(|x| x.mineral_id == mineral)
                })
                .map(|_| r.tick)
        })
}

fn listed_at(m: &MiningObs, tick: u64) -> Option<Vec<String>> {
    m.markets.iter().find(|o| o.tick == tick).map(|o| {
        o.msg
            .payload
            .listings
            .iter()
            .map(|l| l.mineral_id.clone())
            .collect()
    })
}

pub fn judge_leak_market(a: &MiningObs, b: &MiningObs) -> (Verdict, Value) {
    let starts = |m: &MiningObs| {
        m.first_types
            .iter()
            .map(|(_, t)| *t)
            .take(5)
            .collect::<Vec<_>>()
    };
    let order_ok = |m: &MiningObs| starts(m) == SESSION_START_ORDER;
    let t_fe = first_accept_tick(a, FERROSITE);
    let t_gl = first_accept_tick(a, STARFALL_GLASS);
    // (a) 첫 채굴 전 받은 모든 MARKET_STATE 의 상장이 비어 있다.
    let before = |m: &MiningObs| -> (usize, usize) {
        let pre: Vec<_> = m
            .markets
            .iter()
            .filter(|o| t_fe.is_none_or(|t| o.tick < t))
            .collect();
        (
            pre.len(),
            pre.iter()
                .filter(|o| !o.msg.payload.listings.is_empty())
                .count(),
        )
    };
    let (pa, pa_bad) = before(a);
    let (pb, pb_bad) = before(b);
    let fe_a = t_fe.and_then(|t| listed_at(a, t));
    let fe_b = t_fe.and_then(|t| listed_at(b, t));
    let gl_a = t_gl.and_then(|t| listed_at(a, t));
    let gl_b = t_gl.and_then(|t| listed_at(b, t));
    let d = json!({
        "session_start_a": starts(a), "session_start_b": starts(b),
        "first_ferrosite_tick": t_fe, "first_glass_tick": t_gl,
        "markets_before_first_mine": [pa, pb], "nonempty_before_first_mine": [pa_bad, pb_bad],
        "listed_at_ferrosite_tick": [fe_a, fe_b], "listed_at_glass_tick": [gl_a, gl_b]});
    let sc45_cond = pa >= 1 && pb >= 1;
    let sc45 = sc45_cond && pa_bad == 0 && pb_bad == 0;
    let only = |v: &Option<Vec<String>>, want: &[&str]| {
        v.as_ref()
            .is_some_and(|l| l.len() == want.len() && want.iter().all(|w| l.iter().any(|x| x == w)))
    };
    let sc46_cond = t_fe.is_some() && t_gl.is_some();
    let sc46 = only(&fe_a, &[FERROSITE])
        && only(&fe_b, &[FERROSITE])
        && only(&gl_a, &[FERROSITE, STARFALL_GLASS])
        && only(&gl_b, &[FERROSITE, STARFALL_GLASS]);
    let sc47 = order_ok(a) && order_ok(b);
    let mut d = d;
    d["sub_verdicts"] = json!({"sc45": if !sc45_cond {"미검증(조건 미발생)"} else if sc45 {"PASS"} else {"FAIL"},
                               "sc46": if !sc46_cond {"미검증(조건 미발생)"} else if sc46 {"PASS"} else {"FAIL"},
                               "sc47": if sc47 {"PASS"} else {"FAIL"}});
    let v = if !sc45_cond || !sc46_cond {
        Verdict::Undecided
    } else if sc45 && sc46 && sc47 {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    (v, d)
}

// ── 원장 월드 + H-34 (trade-cycle) ────────────────────────────────────────────

/// A·B 는 Ferrosite 를 `cycles` 번 캐고 팔고, C 는 A 의 첫 판매 신호 뒤 Glacine 을 처음 캐서(발견)
/// 팔고 한 번 더. 같은 월드가 SC-30~33(원장 도구)의 입력이다.
#[allow(clippy::too_many_arguments)]
pub async fn run_trade_cycle(
    a_site: &DepositSite,
    b_site: &DepositSite,
    c_site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    labels: [&str; 3],
    cycles: usize,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let first_sale = Arc::new(Notify::new());
    let loop_steps = |site: &DepositSite, mineral: &str, n: usize, signal: Option<Arc<Notify>>| {
        let mut s = Vec::new();
        for i in 0..n {
            if i > 0 {
                s.push(back_to(site));
            }
            s.extend(mine_and_dock(site, ex, mineral, 2000));
            s.push(sell(ex, mineral, SellQty::All));
            s.push(MineStep::Wait(Duration::from_millis(1500)));
            if i == 0
                && let Some(n) = &signal
            {
                s.push(MineStep::Signal(n.clone()));
            }
        }
        s
    };
    let a = script(
        a_site,
        loop_steps(a_site, FERROSITE, cycles, Some(first_sale.clone())),
    );
    let b = script(b_site, loop_steps(b_site, FERROSITE, cycles, None));
    let mut cs = vec![
        MineStep::WaitSignal {
            notify: first_sale.clone(),
            timeout: Duration::from_secs(600),
        },
        back_to(c_site),
    ];
    cs.extend(loop_steps(c_site, GLACINE, 2, None));
    let c = Behavior::MineScript {
        deposit_id: c_site.id.clone(),
        target_m: c_site.position_m,
        stop_radius_m: 1.0,
        fly: FlyMode::Stay,
        steps: cs,
        grace: Duration::from_secs(2),
    };
    let (oa, ob, oc) = tokio::join!(
        run_connection(spec(secret, url, labels[0], clock, a, false)),
        run_connection(spec(secret, url, labels[1], clock, b, false)),
        run_connection(spec(secret, url, labels[2], clock, c, false)),
    );
    judge_trade_cycle(&[&oa.mining, &ob.mining, &oc.mining])
}

/// SC-121(H-34) + SC-132. 발견 지연 = C 의 첫 Glacine 채굴 수락 결과 수신(µs) → 각 봇의 LIVE 수신(µs),
/// 같은 프로세스·같은 시계. ⊘: 그 발견 tick 이전에 수락된 판매 ≥ 1.
pub fn judge_trade_cycle(ms: &[&MiningObs]) -> (Verdict, Value) {
    let c = ms[2];
    let disc_tick = first_accept_tick(c, GLACINE);
    let disc_at = disc_tick
        .and_then(|t| {
            c.results
                .iter()
                .find(|r| r.tick == t && r.status == "ACCEPTED")
                .map(|r| r.frame_seq)
        })
        .and_then(|fs| {
            c.inventory
                .iter()
                .find(|i| i.frame_seq > fs)
                .map(|i| i.at_us)
        });
    let sales_before: usize = ms
        .iter()
        .map(|m| {
            m.receipts
                .iter()
                .filter(|r| disc_tick.is_some_and(|t| r.tick < t))
                .count()
        })
        .sum();
    let mut lat = Vec::new();
    for m in ms {
        if let Some(n) = m.notices.iter().find(|n| {
            n.msg.payload.delivery == "LIVE"
                && n.msg.payload.historical_event.payload.mineral_id == GLACINE
        }) && let Some(t0) = disc_at
        {
            lat.push(n.at_us.saturating_sub(t0) as f64 / 1e6);
        }
    }
    lat.sort_by(f64::total_cmp);
    let max = lat.last().copied();
    let p95 = if lat.is_empty() {
        None
    } else {
        Some(lat[((lat.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)])
    };
    let pairs: Vec<_> = ms
        .iter()
        .map(|m| pair_sell_responses(m, &accepted_sells(m)))
        .collect();
    let accepted: usize = pairs.iter().map(|p| p.accepted_sell).sum();
    let complete: usize = pairs.iter().map(|p| p.complete).sum();
    let order_bad: usize = pairs.iter().map(|p| p.order_violations.len()).sum();
    let d = json!({
        "glacine_discovery_tick": disc_tick, "sales_before_discovery": sales_before,
        "live_receivers": lat.len(), "latency_s": lat, "latency_max_s": max, "latency_p95_s": p95,
        "sc132": {"accepted_sell": accepted, "complete_response_sets": complete, "order_violations": order_bad},
        "receipts_per_bot": ms.iter().map(|m| m.receipts.len()).collect::<Vec<_>>(),
        "goto_failures": ms.iter().map(|m| m.goto_failures).collect::<Vec<_>>()});
    let sc121_cond = sales_before >= 1 && lat.len() >= 2;
    let sc121 = max.is_some_and(|x| x <= 5.0);
    let sc132 = accepted > 0 && complete == accepted && order_bad == 0;
    let mut d = d;
    d["sub_verdicts"] = json!({"sc121": if !sc121_cond {"미검증(조건 미발생)"} else if sc121 {"PASS"} else {"FAIL"},
                               "sc132": if accepted == 0 {"FAIL(수락 0 — 0 == 0 은 검사가 아니다)"} else if sc132 {"PASS"} else {"FAIL"}});
    let v = if accepted == 0 || (sc121_cond && !sc121) || !sc132 {
        Verdict::Fail
    } else if !sc121_cond {
        Verdict::Undecided
    } else {
        Verdict::Pass
    };
    (v, d)
}

/// 데이터 디렉터리에서 광맥 하나 — 못 찾으면 오류 문자열.
pub fn site(data_dir: &Path, id: &str) -> Result<DepositSite, String> {
    find_deposit(data_dir, id)
}

// ── 부하 (AC-20(b), 디자인 T-11) ─────────────────────────────────────────────

/// 부하 봇이 흩어질 광맥(차례로). Starfall Glass 는 500 kg 뿐이라 넣지 않는다 — 한 봇이 영원히 못 채운다.
pub const LOAD_DEPOSITS: [&str; 7] = [
    "inner-belt-1",
    "inner-belt-2",
    "inner-belt-3",
    "vela-orbit-1",
    "vela-orbit-2",
    "outer-field-1",
    "outer-field-2",
];

fn mineral_of(deposit: &str) -> &'static str {
    if deposit.starts_with("inner-belt") {
        FERROSITE
    } else if deposit.starts_with("vela-orbit") {
        GLACINE
    } else {
        "cobaltine"
    }
}

#[derive(Debug, Clone)]
pub struct SellLoadFacts {
    pub bots: usize,
    pub min_ws: u64,
    pub ticks: Option<u64>,
    pub overruns: Option<u64>,
    pub sells_accepted: usize,
    pub backlog_rejects: u64,
    pub max_recording_lag: u64,
    pub pair_accepted: usize,
    pub pair_complete: usize,
    pub pair_order_bad: usize,
}

/// 계약 SC-117(tick 초과 ≤ 0.5 %, ⊘ 31 연결 유지·판매 ≥ 1) · SC-119(RECORDING_BACKLOG 0) · SC-132(응답 항등식).
/// 항등식·바닥·회복(SC-118·120)은 같은 월드를 `trade_ledger.py` 가 판정한다.
pub fn judge_sell_load(f: &SellLoadFacts) -> (Verdict, Value) {
    let ratio = match (f.ticks, f.overruns) {
        (Some(t), Some(o)) if t > 0 => Some(o as f64 / t as f64),
        _ => None,
    };
    let cond117 =
        f.min_ws >= f.bots as u64 && f.bots >= 31 && f.sells_accepted >= 1 && ratio.is_some();
    let ok117 = ratio.is_some_and(|r| r <= 0.005);
    let ok119 = f.backlog_rejects == 0;
    let ok132 = f.pair_accepted > 0 && f.pair_complete == f.pair_accepted && f.pair_order_bad == 0;
    let s117 = if !cond117 {
        "미검증(조건 미발생)"
    } else if ok117 {
        "PASS"
    } else {
        "FAIL"
    };
    let s132 = if f.pair_accepted == 0 {
        "FAIL(수락 0)"
    } else if ok132 {
        "PASS"
    } else {
        "FAIL"
    };
    let d = json!({"bots": f.bots, "min_ws_connections": f.min_ws, "ticks": f.ticks, "overruns": f.overruns,
        "overrun_ratio": ratio, "sells_accepted": f.sells_accepted, "recording_backlog_rejects": f.backlog_rejects,
        "max_recording_lag": f.max_recording_lag,
        "sub_verdicts": {"sc117": s117, "sc119": if ok119 {"PASS(검출기 생존은 sc38·39 짝)"} else {"FAIL"}, "sc132": s132}});
    let v = if !ok119 || !ok132 || (cond117 && !ok117) {
        Verdict::Fail
    } else if !cond117 {
        Verdict::Undecided
    } else {
        Verdict::Pass
    };
    (v, d)
}

pub async fn run_sell_load(
    data_dir: &Path,
    url: &str,
    secret: &str,
    bots: usize,
    label_base: u32,
    duration: Duration,
) -> (Verdict, Value) {
    use crate::restart_cases::fetch_stats;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    let ex = match find_exchange(data_dir) {
        Ok(e) => e,
        Err(e) => return (Verdict::Undecided, json!({"error": e})),
    };
    let clock = Clock::start();
    // 한 사이클 ≈ 채굴 ≤ 120 s + 왕복 70~150 s. 넉넉히 반복하고, 측정 창은 `duration` 으로 자른다.
    // 사이클 수를 창 길이에 맞춘다(2026-10-07 첫 실행: 넉넉히 잡은 사이클 때문에 창이 끝난 뒤 20 분 넘게 돌아
    // 바깥 timeout 에 걸려 JSON 을 못 썼다). 남은 연결은 창 + 5 분 뒤 끊고 `connections_unfinished` 로 센다.
    // r1(2026-10-10) 실측: 세션 지속 min 1,066 / 중앙 1,192 / max 1,510 s, 창 1,200 s →
    // 평균 사이클 ≈ 199 s 로 **분모 200 과 거의 같았다**. 그래서 봇이 창 경계에 몰려 끝나고
    // 약 절반이 먼저 닫아 `min_ws` 가 13 까지 떨어졌다(선단언 `min_ws >= bots` 가 깨진 원인).
    // 150 이면 8 사이클이고 **가장 빠른 봇(178 s/사이클)도 1,424 s > 1,210 s 로 창을 덮는다.**
    // 위 2026-10-07 결함(창 뒤 20 분 초과 → 바깥 timeout)은 재발하지 않는다 — 그 뒤 추가된
    // 유예 절단(창 + 300 s)이 `case_timeout` 안에서 끝낸다. 대가는 `connections_unfinished`
    // 가 느는 것이고, 그 카운터가 바로 그 용도이며 판정식에는 들어가지 않는다(아래 :1238).
    let cycles = ((duration.as_secs() / 150) as usize).max(2);
    let mut conns = Vec::new();
    for i in 0..bots {
        let dep = LOAD_DEPOSITS[i % LOAD_DEPOSITS.len()];
        let site = match find_deposit(data_dir, dep) {
            Ok(s) => s,
            Err(e) => return (Verdict::Undecided, json!({"error": e})),
        };
        let mineral = mineral_of(dep);
        let mut s = vec![MineStep::Wait(Duration::from_millis(
            (i as u64 * 97) % 3100,
        ))];
        for c in 0..cycles {
            if c > 0 {
                s.push(back_to(&site));
            }
            s.push(MineStep::MineUntil {
                deposit_id: site.id.clone(),
                target_m: site.position_m,
                mineral_id: mineral.to_owned(),
                kg: 2000,
                every: MINE_EVERY,
                timeout: Duration::from_secs(120),
            });
            s.push(MineStep::GoTo {
                target_m: ex.position_m,
                stop_radius_m: ex.stop_radius(),
                timeout: Duration::from_secs(240),
            });
            s.push(MineStep::Wait(Duration::from_millis(600)));
            s.push(sell(&ex, mineral, SellQty::All));
            s.push(MineStep::Wait(Duration::from_millis(1200)));
        }
        let label = format!("bot-{:03}", label_base + i as u32);
        conns.push(tokio::spawn(run_connection(spec(
            secret,
            url,
            &label,
            clock,
            script(&site, s),
            false,
        ))));
    }
    // 연결이 다 붙을 시간을 준 뒤 측정 창을 연다.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let stats0 = fetch_stats(url).await.ok();
    let stop = Arc::new(AtomicBool::new(false));
    let min_ws = Arc::new(AtomicU64::new(u64::MAX));
    let max_lag = Arc::new(AtomicU64::new(0));
    let poller = {
        let (stop, min_ws, max_lag, url) = (
            stop.clone(),
            min_ws.clone(),
            max_lag.clone(),
            url.to_owned(),
        );
        tokio::spawn(async move {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(s) = fetch_stats(&url).await {
                    min_ws.fetch_min(s["ws_connections"].as_u64().unwrap_or(0), Ordering::Relaxed);
                    max_lag.fetch_max(s["recording_lag"].as_u64().unwrap_or(0), Ordering::Relaxed);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
    };
    tokio::time::sleep(duration).await;
    stop.store(true, Ordering::Relaxed);
    let _ = poller.await;
    let stats1 = fetch_stats(url).await.ok();
    // 봇은 자기 스크립트를 끝까지 돈다 — 측정 창 밖이므로 tick 비율에는 안 들어간다.
    let mut outs = Vec::new();
    let mut unfinished = 0usize;
    let grace_end = tokio::time::Instant::now() + Duration::from_secs(300);
    for c in conns {
        let left = grace_end.saturating_duration_since(tokio::time::Instant::now());
        let abort = c.abort_handle();
        match tokio::time::timeout(left, c).await {
            Ok(Ok(o)) => outs.push(o),
            _ => {
                abort.abort();
                unfinished += 1;
            }
        }
    }
    let mut by: std::collections::BTreeMap<String, u64> = Default::default();
    let (mut acc, mut comp, mut bad) = (0usize, 0usize, 0usize);
    let mut by_mineral: std::collections::BTreeMap<String, i64> = Default::default();
    for o in &outs {
        let m = &o.mining;
        let ids = m
            .sell_sent
            .iter()
            .map(|s| s.command_id)
            .chain(m.mine_sent.iter().map(|s| s.command_id));
        for id in ids {
            let k = match m.result_of(id) {
                Some(r) if r.status == "ACCEPTED" => "ACCEPTED".to_owned(),
                Some(r) => r.reason_code.clone().unwrap_or_else(|| "<null>".into()),
                None => "<no result>".into(),
            };
            *by.entry(k).or_default() += 1;
        }
        let p = pair_sell_responses(m, &accepted_sells(m));
        acc += p.accepted_sell;
        comp += p.complete;
        bad += p.order_violations.len();
        for r in &m.receipts {
            *by_mineral
                .entry(r.msg.payload.mineral_id.clone())
                .or_default() += r.msg.payload.quantity_kg;
        }
    }
    let d_tick = |k: &str| -> Option<u64> {
        Some(
            stats1.as_ref()?[k]
                .as_u64()?
                .saturating_sub(stats0.as_ref()?[k].as_u64()?),
        )
    };
    let facts = SellLoadFacts {
        bots,
        min_ws: min_ws.load(Ordering::Relaxed),
        ticks: d_tick("tick_total"),
        overruns: d_tick("tick_overrun_total"),
        sells_accepted: acc,
        backlog_rejects: by.get("RECORDING_BACKLOG").copied().unwrap_or(0),
        max_recording_lag: max_lag.load(Ordering::Relaxed),
        pair_accepted: acc,
        pair_complete: comp,
        pair_order_bad: bad,
    };
    let (v, mut d) = judge_sell_load(&facts);
    d["results_by_reason"] = json!(by);
    d["sold_kg_by_mineral"] = json!(by_mineral);
    d["duration_s"] = json!(duration.as_secs());
    d["connections_finished"] = json!(outs.len());
    d["connections_unfinished"] = json!(unfinished);
    (v, d)
}
