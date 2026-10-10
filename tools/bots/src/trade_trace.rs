//! p1-03 계약 SC-114 · SC-115 — "B 가 흔적을 본다"(디자인 T-3)의 B 쪽. **별도 프로세스**로 돈다.
//!
//! 순서: A(`trace-sell-a`)가 판매 뒤 접속을 끊는다 → 여기서 A 의 함선 디스폰 행(`LINGER_EXPIRED`)이
//! DB 에 생길 때까지 기다린다 → B 가 처음 접속해 첫 `MARKET_STATE` 를 받는다.
//! 판정 입력은 B 가 받은 메시지뿐이다(SC-115). SQL 은 ⊘ 전제(디스폰·첫 접속·tick 차)만 잰다.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::sql;
use crate::token;
use crate::trade::{RECOVERY_INTERVAL_TICKS, judge_trace_b, run_trace_b};

fn actor_of(label: &str) -> Option<Uuid> {
    Uuid::parse_str(&token::subject_for(label)).ok()
}

/// A 의 14,000 kg 판매 tick(없으면 `None`).
fn sale_tick(world: &str, a: Uuid) -> Result<Option<i64>, String> {
    sql::scalar(&format!(
        "select max(tick) from domain_events where world_id = '{world}' and event_type = 'MINERAL_SOLD' \
         and actor_id = '{a}' and (payload->>'quantity_kg')::bigint = 14000"
    ))
    .map(|v| v.and_then(|s| s.parse().ok()))
}

/// A 의 잔류 만료 디스폰 tick. 계약 필드 이름 `despawn_reason`(p1-02 trace 의 함정 — 키가 없으면 드러낸다).
fn despawn_tick(world: &str, a: Uuid) -> Result<Option<i64>, String> {
    let rows = sql::rows(&format!(
        "select tick, coalesce(payload->>'despawn_reason', '<missing>') from domain_events \
         where world_id = '{world}' and event_type = 'SHIP_DESPAWNED' and actor_id = '{a}' order by tick"
    ))?;
    if rows
        .iter()
        .any(|r| r.get(1).is_some_and(|v| v == "<missing>"))
    {
        return Err("SHIP_DESPAWNED payload 에 despawn_reason 키가 없다".to_owned());
    }
    Ok(rows
        .iter()
        .filter(|r| r.get(1).is_some_and(|v| v == "LINGER_EXPIRED"))
        .filter_map(|r| r.first().and_then(|t| t.parse().ok()))
        .next_back())
}

pub async fn run_b(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label_b: &str,
    seller_label: &str,
    world: &str,
) -> (Verdict, Value) {
    if !sql::is_uuid(world) {
        return (
            Verdict::Undecided,
            json!({"error": "--world 가 UUID 가 아니다"}),
        );
    }
    let (Some(a), Some(b)) = (actor_of(seller_label), actor_of(label_b)) else {
        return (
            Verdict::Undecided,
            json!({"error": "라벨에서 actor 를 못 만든다"}),
        );
    };
    // A 디스폰을 기다린다(잔류 30 s + 기록 지연). 너무 오래면 창을 놓친다 — 그대로 무효.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let mut despawn = despawn_tick(world, a);
    while matches!(despawn, Ok(None)) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(200)).await;
        despawn = despawn_tick(world, a);
    }
    let prior_b = sql::scalar_i64(&format!(
        "select count(*) from domain_events where world_id = '{world}' and event_type = 'SESSION_OPENED' and actor_id = '{b}'"
    ));
    let out = run_trace_b(site, url, secret, label_b).await;
    let b_ready_tick = out.ledger.session.as_ref().map(|s| s.ready_tick);
    let sale = sale_tick(world, a);
    let (v115, d115) = judge_trace_b(&out.mining, a);
    judge_b(sale, despawn, prior_b, b_ready_tick, v115, d115)
}

/// SC-114 의 ⊘ 넷을 먼저 본다: A 판매 존재, A 디스폰 tick < B 접속 tick, B 의 이전 세션 0,
/// (B 접속 − 판매) ≤ 1,200 tick. 하나라도 아니면 **무효**(SC-115 도 판정하지 않는다).
pub fn judge_b(
    sale: Result<Option<i64>, String>,
    despawn: Result<Option<i64>, String>,
    prior_b: Result<i64, String>,
    b_ready_tick: Option<u64>,
    v115: Verdict,
    d115: Value,
) -> (Verdict, Value) {
    let sale_t = sale.as_ref().ok().copied().flatten();
    let desp_t = despawn.as_ref().ok().copied().flatten();
    let gap = match (sale_t, b_ready_tick) {
        (Some(s), Some(r)) => Some(r as i64 - s),
        _ => None,
    };
    let mut d = json!({"a_sale_tick": sale_t, "a_despawn_tick": desp_t, "b_prior_sessions": prior_b.as_ref().ok(),
        "b_ready_tick": b_ready_tick, "tick_gap_b_minus_sale": gap, "window_ticks": RECOVERY_INTERVAL_TICKS,
        "sql_errors": [sale.err(), despawn.err(), prior_b.err()], "sc115": d115});
    let cond = sale_t.is_some()
        && matches!((desp_t, b_ready_tick), (Some(x), Some(r)) if x < r as i64)
        && d["b_prior_sessions"] == json!(0)
        && gap.is_some_and(|g| g > 0 && g <= RECOVERY_INTERVAL_TICKS as i64);
    d["sub_verdicts"] = json!({"sc114": if cond {"PASS"} else {"무효(조건 미발생)"},
                               "sc115": if cond {v115.as_str()} else {"무효(sc114 조건 미발생)"}});
    if !cond {
        return (Verdict::Undecided, d);
    }
    (v115, d)
}
