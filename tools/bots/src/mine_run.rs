//! `bots mine` — 광맥 하나로 날아가 채굴하고, 받은 것을 JSON 한 덩어리로 찍는다.
//!
//! **관측 도구다. verdict 를 내지 않는다**(계약 §7b 규칙 7 — 판정 도구는 계약이 이름 댄 케이스만).
//! 계약 케이스(`mine-dup`·`leak-scan`·`trace-abc` …)는 이 위에 판정을 얹는다.
//!
//! 광맥 위치·반경은 `data/world/deposits/*.json` 에서 읽는다 — **길 찾기 입력**이지 기대값이 아니다
//! (계약 §0.6 은 판정 기대값을 `data/` 에서 읽지 말라는 규칙이다).

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::conn::{Behavior, BotSpec, Clock, ConnectionOutcome, run_connection};
use crate::token;

/// 광맥 하나의 길 찾기 입력.
#[derive(Debug, Clone)]
pub struct DepositSite {
    pub id: String,
    pub position_m: [f64; 3],
    pub radius_m: f64,
}

/// `data/world/deposits/*.json` 에서 `deposit_id` 를 찾는다.
pub fn find_deposit(data_dir: &Path, deposit_id: &str) -> Result<DepositSite, String> {
    let dir = data_dir.join("world").join("deposits");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let v: Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        for d in v["deposits"].as_array().into_iter().flatten() {
            if d["id"].as_str() == Some(deposit_id) {
                let p = d["position_m"].as_array().ok_or("position_m 없음")?;
                let pos = [
                    p.first().and_then(Value::as_f64).ok_or("position_m[0]")?,
                    p.get(1).and_then(Value::as_f64).ok_or("position_m[1]")?,
                    p.get(2).and_then(Value::as_f64).ok_or("position_m[2]")?,
                ];
                let r = d["radius_m"].as_f64().ok_or("radius_m 없음")?;
                return Ok(DepositSite {
                    id: deposit_id.to_owned(),
                    position_m: pos,
                    radius_m: r,
                });
            }
        }
    }
    Err(format!("{} 에 deposit {deposit_id} 가 없다", dir.display()))
}

/// 한 연결로 채굴 실행.
#[derive(Debug, Clone)]
pub struct MineRun {
    pub url: String,
    pub label: String,
    pub site: DepositSite,
    /// 광맥 **표면**에서 멈출 거리(m). 서버 사거리 150 m 보다 작게.
    pub stop_from_surface_m: f64,
    pub mines: u32,
    pub gap: Duration,
    pub resend_last: bool,
    pub arrive_timeout: Duration,
    pub capture_raw: bool,
}

pub async fn run(cfg: &MineRun, secret: &str) -> ConnectionOutcome {
    let (_subject, tok) = token::identity(secret, &cfg.label);
    run_connection(BotSpec {
        label: cfg.label.clone(),
        url: cfg.url.clone(),
        token: tok,
        behavior: Behavior::MineAt {
            deposit_id: cfg.site.id.clone(),
            target_m: cfg.site.position_m,
            stop_radius_m: cfg.site.radius_m + cfg.stop_from_surface_m,
            arrive_timeout: cfg.arrive_timeout,
            mines: cfg.mines,
            gap: cfg.gap,
            resend_last: cfg.resend_last,
            grace: Duration::from_secs(2),
        },
        clock: Clock::start(),
        live_corr: None,
        capture_raw: cfg.capture_raw,
    })
    .await
}

/// 받은 것 요약 — 판정이 아니라 기록이다.
pub fn summary(cfg: &MineRun, o: &ConnectionOutcome) -> Value {
    let m = &o.mining;
    let session = o.ledger.session.as_ref().map(|s| {
        json!({"session_id": s.session_id.to_string(), "actor_id": s.actor_id.to_string(),
               "ready_tick": s.ready_tick})
    });
    let nav = m.nav.as_ref().map(|n| {
        json!({"deposit_id": n.deposit_id, "start_dist_m": n.start_dist_m, "arrived": n.arrived_at_us.is_some(),
               "arrive_dist_m": n.arrive_dist_m, "arrive_speed_mps": n.arrive_speed_mps,
               "control_frames": n.control_frames, "max_speed_mps": n.max_speed_mps,
               "stop_radius_m": cfg.site.radius_m + cfg.stop_from_surface_m})
    });
    let sent: Vec<Value> = m
        .mine_sent
        .iter()
        .map(|s| {
            let res: Vec<Value> = m
                .results
                .iter()
                .filter(|r| r.command_id == s.command_id)
                .map(|r| json!({"status": r.status, "reason": r.reason_code, "tick": r.tick, "frame_seq": r.frame_seq}))
                .collect();
            json!({"command_id": s.command_id.to_string(), "dist_m": s.dist_m, "speed_mps": s.speed_mps,
                   "last_snapshot_tick": s.last_snapshot_tick, "results": res})
        })
        .collect();
    let inventory: Vec<Value> = m
        .inventory
        .iter()
        .map(|i| json!({"frame_seq": i.frame_seq, "tick": i.tick, "payload": serde_json::to_value(&i.msg.payload).unwrap_or(Value::Null)}))
        .collect();
    json!({
        "tool": "bots mine (관측 — verdict 없음)",
        "label": cfg.label,
        "session": session,
        "nav": nav,
        "mine_sent": sent,
        "inventory_states": inventory,
        "deposit_field_states": m.deposits.len(),
        "historical_event_notices": m.notices.iter().map(|n| serde_json::to_value(&n.msg).unwrap_or(Value::Null)).collect::<Vec<_>>(),
        "close": o.ledger.session.as_ref().map(|s| json!({"code": s.close_code, "initiator": s.close_initiator})),
        "connect_error": o.connect_error,
    })
}
