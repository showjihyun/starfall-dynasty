//! p1-03 재기동·변조 계열 판매 단계 — **판정하지 않는다**(관측용 `Reference`). 판정은 오케스트레이터
//! `tests/e2e/restart_cases.py` 가 이 단계들의 사실 + 서버 종료 코드·로그 + SQL 로 낸다(계약 §3.2 —
//! SC-34·35·36·51·112 의 지명 도구가 그것이다).
//!
//! 단계(`--phase`):
//! * `sell-dup-restart`  run: 400 kg 채굴 → 도킹 → 200 kg 판매(수락) → 상태 파일 / resume: 재기동 뒤 같은 원문 재전송
//! * `sell-cas-halt`     run: 400 kg → 판매 200(수락) → ready 파일 → (오케스트레이터가 SQL 변조) → tampered 파일 대기 → 판매 100
//! * `sell-cas-reload`   resume: 재기동 뒤 거래소로 가서 판매 100
//! * `sell-recover-restart` run: 14,000 kg 한 번 판매(바닥) → 회복 배수 한 번 관측(13,000) → 종료 /
//!   resume: 재기동 뒤 첫 `MARKET_STATE`(보존)와 다음 회복 변화를 관측

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::conn::{
    Behavior, BotSpec, Clock, ConnectionOutcome, FlyMode, MineStep, SellQty, run_connection,
};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::token;
use crate::trade::{Exchange, FERROSITE, baseline, mine_and_dock};

#[derive(Debug, Clone, Default)]
pub struct PhaseFiles {
    pub phase: String,
    pub state: Option<std::path::PathBuf>,
    pub ready: Option<std::path::PathBuf>,
    pub tampered: Option<std::path::PathBuf>,
}

fn spec(secret: &str, url: &str, label: &str, behavior: Behavior) -> BotSpec {
    let (_s, tok) = token::identity(secret, label);
    BotSpec {
        label: label.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior,
        clock: Clock::start(),
        live_corr: None,
        capture_raw: false,
    }
}

fn stay(site: &DepositSite, steps: Vec<MineStep>) -> Behavior {
    Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: 1.0,
        fly: FlyMode::Stay,
        steps,
        grace: Duration::from_secs(1),
    }
}

fn fly(site: &DepositSite, steps: Vec<MineStep>) -> Behavior {
    Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        fly: FlyMode::Stop {
            timeout: Duration::from_secs(300),
        },
        steps,
        grace: Duration::from_secs(1),
    }
}

/// 이 연결이 본 것 — 오케스트레이터의 판정 입력.
pub fn facts(o: &ConnectionOutcome) -> Value {
    let m = &o.mining;
    json!({
        "actor_id": o.ledger.session.as_ref().map(|s| s.actor_id),
        "ready_tick": o.ledger.session.as_ref().map(|s| s.ready_tick),
        "own_ship_id": m.own_ship_id,
        "sells": m.sell_sent.iter().map(|s| json!({
            "command_id": s.command_id, "quantity_kg": s.quantity_kg,
            "results": m.results_of(s.command_id).iter().map(|r| json!({"status": r.status, "reason": r.reason_code, "tick": r.tick})).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),
        "wallets": m.wallets.iter().map(|w| json!({"tick": w.tick, "balance_cr": w.msg.payload.balance_cr})).collect::<Vec<_>>(),
        "receipts": m.receipts.iter().map(|r| json!({"tick": r.tick, "command_id": r.msg.payload.command_id,
            "proceeds_cr": r.msg.payload.proceeds_cr})).collect::<Vec<_>>(),
        "ferrosite_market": m.markets.iter().filter_map(|o| o.msg.listing(FERROSITE).map(|l| json!({
            "tick": o.tick, "saturation_kg": l.saturation_kg, "price_cr_per_kg": l.price_cr_per_kg,
            "last_sale": l.last_sale.as_ref().map(|s| json!({"actor_id": s.actor_id, "quantity_kg": s.quantity_kg, "tick": s.tick}))
        }))).collect::<Vec<_>>(),
        "inventory_ferrosite_last": m.holding(FERROSITE),
        "open_at_end": m.open_at_end,
        "goto_failures": m.goto_failures,
        "close_initiator": o.ledger.session.as_ref().map(|s| s.close_initiator.clone()),
    })
}

fn read_state(p: &Option<std::path::PathBuf>) -> Result<Value, String> {
    let p = p.as_ref().ok_or("--state 가 필요하다")?;
    serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| e.to_string())
}

fn write_state(p: &Option<std::path::PathBuf>, v: &Value) -> Result<(), String> {
    let p = p.as_ref().ok_or("--state 가 필요하다")?;
    std::fs::write(p, serde_json::to_string_pretty(v).unwrap_or_default())
        .map_err(|e| e.to_string())
}

fn done(o: &ConnectionOutcome, phase: &str) -> (Verdict, Value) {
    let mut d = facts(o);
    d["phase"] = json!(phase);
    // 판정이 아니다 — 단계를 끝까지 돌았다는 표시(exit 0). 판정은 restart_cases.py.
    (Verdict::Pass, d)
}

pub async fn run_dup_restart(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
    f: &PhaseFiles,
) -> (Verdict, Value) {
    match f.phase.as_str() {
        "run" => {
            let o = run_connection(spec(secret, url, label, fly(site, baseline(site, ex)))).await;
            let Some(s) = o.mining.sell_sent.first() else {
                return (Verdict::Undecided, json!({"why": "판매를 보내지 못했다"}));
            };
            let st = json!({"command_id": s.command_id, "market_id": s.market_id, "mineral_id": s.mineral_id,
                            "quantity_kg": s.quantity_kg, "balance_after": o.mining.balance()});
            if let Err(e) = write_state(&f.state, &st) {
                return (Verdict::Undecided, json!({"error": e}));
            }
            done(&o, "run")
        }
        "resume" => {
            let st = match read_state(&f.state) {
                Ok(v) => v,
                Err(e) => return (Verdict::Undecided, json!({"error": e})),
            };
            let Some(id) = st["command_id"]
                .as_str()
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
            else {
                return (
                    Verdict::Undecided,
                    json!({"error": "상태 파일에 command_id 없음"}),
                );
            };
            let steps = vec![
                MineStep::Wait(Duration::from_millis(800)),
                MineStep::SellId {
                    id,
                    market_id: st["market_id"].as_str().unwrap_or_default().to_owned(),
                    mineral_id: st["mineral_id"].as_str().unwrap_or_default().to_owned(),
                    qty: SellQty::Kg(st["quantity_kg"].as_i64().unwrap_or(1)),
                },
                MineStep::Wait(Duration::from_millis(1500)),
            ];
            let o = run_connection(spec(secret, url, label, stay(site, steps))).await;
            let mut d = done(&o, "resume");
            d.1["state"] = st;
            d
        }
        p => (
            Verdict::Undecided,
            json!({"error": format!("--phase run|resume (받음: {p})")}),
        ),
    }
}

pub async fn run_cas_halt(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
    f: &PhaseFiles,
) -> (Verdict, Value) {
    let (Some(ready), Some(tampered)) = (f.ready.clone(), f.tampered.clone()) else {
        return (
            Verdict::Undecided,
            json!({"error": "--ready-file 과 --tampered-file 이 필요하다"}),
        );
    };
    let mut s = baseline(site, ex);
    s.push(MineStep::TouchFile(ready));
    s.push(MineStep::WaitFile {
        path: tampered,
        timeout: Duration::from_secs(180),
    });
    s.push(MineStep::Sell {
        market_id: ex.id.clone(),
        mineral_id: FERROSITE.to_owned(),
        qty: SellQty::Kg(100),
    });
    s.push(MineStep::Wait(Duration::from_secs(6)));
    let o = run_connection(spec(secret, url, label, fly(site, s))).await;
    done(&o, "run")
}

pub async fn run_cas_reload(
    site: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
) -> (Verdict, Value) {
    let steps = vec![
        MineStep::GoTo {
            target_m: ex.position_m,
            stop_radius_m: ex.stop_radius(),
            timeout: Duration::from_secs(240),
        },
        MineStep::Wait(Duration::from_millis(1500)),
        MineStep::Sell {
            market_id: ex.id.clone(),
            mineral_id: FERROSITE.to_owned(),
            qty: SellQty::Kg(100),
        },
        MineStep::Wait(Duration::from_secs(2)),
    ];
    let o = run_connection(spec(secret, url, label, stay(site, steps))).await;
    done(&o, "resume")
}

/// 회복 관측: 바닥(14,000) 판매 → 포화가 한 번 줄어든 `MARKET_STATE` 를 볼 때까지 머문다(≤ 75 s).
pub async fn run_recover(
    ib1: &DepositSite,
    ib3: &DepositSite,
    ex: &Exchange,
    url: &str,
    secret: &str,
    label: &str,
    f: &PhaseFiles,
) -> (Verdict, Value) {
    match f.phase.as_str() {
        "run" => {
            let mut s = vec![MineStep::MineUntil {
                deposit_id: ib1.id.clone(),
                target_m: ib1.position_m,
                mineral_id: FERROSITE.to_owned(),
                kg: 12_000,
                every: crate::trade::MINE_EVERY,
                timeout: Duration::from_secs(420),
            }];
            s.push(crate::trade::back_to(ib3));
            s.extend(mine_and_dock(ib3, ex, FERROSITE, 14_000));
            s.push(MineStep::Sell {
                market_id: ex.id.clone(),
                mineral_id: FERROSITE.to_owned(),
                qty: SellQty::Kg(14_000),
            });
            // 회복 배수(60 s) 하나를 넘길 만큼 머문다.
            s.push(MineStep::Wait(Duration::from_secs(75)));
            let o = run_connection(spec(secret, url, label, fly(ib1, s))).await;
            done(&o, "run")
        }
        "resume" => {
            let o = run_connection(spec(
                secret,
                url,
                label,
                stay(ib1, vec![MineStep::Wait(Duration::from_secs(75))]),
            ))
            .await;
            done(&o, "resume")
        }
        p => (
            Verdict::Undecided,
            json!({"error": format!("--phase run|resume (받음: {p})")}),
        ),
    }
}

/// 오케스트레이터가 쓰는 경로 도우미.
pub fn path_opt(s: &str) -> Option<std::path::PathBuf> {
    (!s.is_empty()).then(|| Path::new(s).to_path_buf())
}
