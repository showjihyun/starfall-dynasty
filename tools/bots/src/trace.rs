//! p1-02 `trace-abc` · `trace-c` — 같은 광물을 A 가 처음, B 가 다음에 캐고, A 가 떠난 뒤 C 가
//! **처음** 접속한다(디자인 S-1·S-4, 계약 SC-60·61·80·81·82·99).
//!
//! - `trace-abc`: A·B 두 연결을 **한 프로세스**에서 같은 시계로 돌린다(수신 시각 비교가 필요하다).
//!   A 는 B 가 광맥에 도착한 신호를 받고 캔다. B 는 A 의 신호 뒤 같은 광맥을 캔다. A 가 닫으면
//!   B 는 A 의 함선이 스냅샷에서 **사라질 때까지** 남아 본다(잔류 만료 디스폰).
//! - `trace-c`: **별도 프로세스**(계약 SC-82) — C 가 받은 메시지만 판정 입력이다. 사전 조건
//!   (A 디스폰 행, C 의 이전 세션 0) 은 읽기 전용 SQL 로 확인한다.
//!
//! 판정 라벨은 `ProbeCase::contract_item()` 하나다(규칙: 팔마다 리터럴 하나). 항목별 결과는
//! 출력 JSON 의 `items` 에 `sc_60` 같은 키로 싣는다.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::conn::{Behavior, BotSpec, Clock, ConnectionOutcome, FlyMode, MineStep, run_connection};
use crate::mine_cases::Verdict;
use crate::mine_run::DepositSite;
use crate::mining::{MiningObs, pair_mine_responses};
use crate::{sql, token};

fn actor_of(secret: &str, label: &str) -> Uuid {
    let (subject, _) = token::identity(secret, label);
    Uuid::parse_str(&subject.to_string()).unwrap_or(Uuid::nil())
}

fn and(vs: &[Verdict]) -> Verdict {
    if vs.contains(&Verdict::Undecided) {
        Verdict::Undecided
    } else if vs.contains(&Verdict::Fail) {
        Verdict::Fail
    } else {
        Verdict::Pass
    }
}

fn live_notices(o: &MiningObs) -> Vec<(u64, Uuid, Option<Uuid>, u64)> {
    o.notices
        .iter()
        .filter(|n| n.msg.payload.delivery == "LIVE")
        .map(|n| {
            (
                n.frame_seq,
                n.msg.payload.historical_event.historical_event_id,
                n.msg.payload.historical_event.discoverer(),
                n.at_us,
            )
        })
        .collect()
}

fn total_kg(o: &MiningObs, i: usize) -> i64 {
    o.inventory[i]
        .msg
        .payload
        .items
        .iter()
        .map(|x| x.quantity_kg)
        .sum()
}

/// A·B 한 실행.
pub async fn run_abc(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label_a: &str,
    label_b: &str,
    world: Option<&str>,
) -> (Verdict, Value) {
    let clock = Clock::start();
    let b_ready = Arc::new(tokio::sync::Notify::new());
    let a_done = Arc::new(tokio::sync::Notify::new());
    let actor_a = actor_of(secret, label_a);
    // 사전 조건(SC-80 ⊘): 이 월드에 역사 기록 0 행 — A 채굴 **전**.
    let hist_before = match world {
        Some(w) if sql::is_uuid(w) => sql::scalar_i64(&format!(
            "select count(*) from historical_events where world_id = '{w}'"
        ))
        .ok(),
        _ => None,
    };
    let stop = FlyMode::Stop {
        timeout: Duration::from_secs(240),
    };
    let beh = |steps: Vec<MineStep>| Behavior::MineScript {
        deposit_id: site.id.clone(),
        target_m: site.position_m,
        stop_radius_m: site.radius_m + 90.0,
        fly: stop,
        steps,
        grace: Duration::from_millis(500),
    };
    let a = beh(vec![
        MineStep::WaitSignal {
            notify: b_ready.clone(),
            timeout: Duration::from_secs(300),
        },
        MineStep::Mine,
        MineStep::Wait(Duration::from_secs(3)),
        MineStep::Signal(a_done.clone()),
    ]);
    let b = beh(vec![
        MineStep::Signal(b_ready.clone()),
        MineStep::WaitSignal {
            notify: a_done.clone(),
            timeout: Duration::from_secs(300),
        },
        MineStep::Mine,
        MineStep::Wait(Duration::from_secs(3)),
        MineStep::WaitActorGone {
            actor: actor_a,
            timeout: Duration::from_secs(90),
        },
    ]);
    let spec = |label: &str, behavior| {
        let (_s, tok) = token::identity(secret, label);
        BotSpec {
            label: label.to_owned(),
            url: url.to_owned(),
            token: tok,
            behavior,
            clock,
            live_corr: None,
            capture_raw: false,
        }
    };
    let (oa, ob) = tokio::join!(
        run_connection(spec(label_a, a)),
        run_connection(spec(label_b, b))
    );
    let hist_after = match world {
        Some(w) if sql::is_uuid(w) => sql::scalar_i64(&format!(
            "select count(*) from historical_events where world_id = '{w}'"
        ))
        .ok(),
        _ => None,
    };
    judge_abc(&oa, &ob, actor_a, hist_before, hist_after, world, clock)
}

/// 순수 판정(SQL 값은 인자로).
pub fn judge_abc(
    oa: &ConnectionOutcome,
    ob: &ConnectionOutcome,
    actor_a: Uuid,
    hist_before: Option<i64>,
    hist_after: Option<i64>,
    world: Option<&str>,
    clock: Clock,
) -> (Verdict, Value) {
    let (ma, mb) = (&oa.mining, &ob.mining);
    let a_mine = ma.mine_sent.first().map(|s| s.command_id);
    let b_mine = mb.mine_sent.first().map(|s| s.command_id);
    let status = |m: &MiningObs, id: Option<Uuid>| {
        id.and_then(|id| m.results.iter().find(|r| r.command_id == id))
            .map(|r| r.status.clone())
    };
    let a_acc = status(ma, a_mine).as_deref() == Some("ACCEPTED");
    let b_acc = status(mb, b_mine).as_deref() == Some("ACCEPTED");
    let (la, lb) = (live_notices(ma), live_notices(mb));
    let coordination_ok = ma.signal_timeouts == 0 && mb.signal_timeouts == 0;

    // SC-80: A·B 모두 LIVE 1, 같은 기록, 발견자 = A. ⊘: 새 월드 기록 0 행에서 시작.
    let same_event = la.len() == 1 && lb.len() == 1 && la[0].1 == lb[0].1;
    let discoverer_a = la.first().and_then(|x| x.2) == Some(actor_a)
        && lb.first().and_then(|x| x.2) == Some(actor_a);
    let v80 = if hist_before != Some(0) || !a_acc || !coordination_ok {
        Verdict::Undecided
    } else if same_event && discoverer_a {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-60: 열린 세션 2 (A 채굴 시각에 B 는 이미 READY), 받은 세션 = 열린 세션.
    // B 가 A 채굴 전에 열려 있었다는 근거는 **순서**다: A 는 B 의 도착 신호(B 는 READY 뒤에만 난다)를
    // 받고서야 캔다 — 신호가 시간 초과 없이 전달됐으면 성립한다.
    let open = usize::from(oa.ledger.session.is_some()) + usize::from(ob.ledger.session.is_some());
    let b_open_before = coordination_ok && mb.session_ready_frame_seq.is_some();
    let received = usize::from(!la.is_empty()) + usize::from(!lb.is_empty());
    let v60 = if !b_open_before || !a_acc || open < 2 {
        Verdict::Undecided
    } else if received == open {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-81: B 수락 + B 인벤토리 증가 + 새 역사 기록 0(B 에게 LIVE 추가 없음 + SQL 행 수 1).
    let b_inv_up =
        mb.inventory.len() >= 2 && total_kg(mb, mb.inventory.len() - 1) > total_kg(mb, 0);
    let b_extra_live = b_mine
        .and_then(|id| mb.results.iter().find(|r| r.command_id == id))
        .map(|r| lb.iter().filter(|n| n.0 > r.frame_seq).count())
        .unwrap_or(0);
    let accepted_x = usize::from(a_acc) + usize::from(b_acc);
    let v81 = if !b_acc || !b_inv_up || accepted_x < 2 || hist_after.is_none() {
        Verdict::Undecided
    } else if b_extra_live == 0 && hist_after == Some(1) {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-99: 봇마다 수락 수 == 짝 지은 INVENTORY_STATE 수, 수락 0 이면 FAIL.
    let pa = pair_mine_responses(
        ma,
        &ma.mine_sent
            .iter()
            .map(|s| s.command_id)
            .collect::<Vec<_>>(),
    );
    let pb = pair_mine_responses(
        mb,
        &mb.mine_sent
            .iter()
            .map(|s| s.command_id)
            .collect::<Vec<_>>(),
    );
    let v99 = if pa.ok() && pb.ok() {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-82 전제(A 디스폰)는 여기서 관측만 — 판정은 trace-c.
    let a_gone = mb.actor_seen && mb.actor_gone_tick.is_some();

    let overall = and(&[v60, v80, v81, v99]);
    (
        overall,
        json!({
            "world_id": world,
            "items": {
                "sc_60": {"verdict": v60.as_str(), "open_sessions": open, "received_live": received},
                "sc_80": {"verdict": v80.as_str(), "historical_events_before": hist_before,
                          "a_accepted": a_acc, "live_a": la.len(), "live_b": lb.len(),
                          "same_event": same_event, "discoverer_is_a": discoverer_a,
                          "actor_a": actor_a.to_string()},
                "sc_81": {"verdict": v81.as_str(), "b_accepted": b_acc, "b_inventory_increased": b_inv_up,
                          "accepted_mine_of_x": accepted_x, "b_live_after_its_mine": b_extra_live,
                          "historical_events_after": hist_after},
                "sc_99": {"verdict": v99.as_str(),
                          "a": {"accepted": pa.accepted_mine, "paired": pa.paired},
                          "b": {"accepted": pb.accepted_mine, "paired": pb.paired}},
            },
            "coordination": {"signal_timeouts_a": ma.signal_timeouts, "signal_timeouts_b": mb.signal_timeouts},
            "a_despawn_observed_by_b": {"seen": mb.actor_seen, "gone_tick": mb.actor_gone_tick},
            "a_despawned": a_gone,
            // SC-64 재료: LIVE 수신 벽시계(ms) — recorded_at 과의 조인은 수신 뒤 SQL 로.
            "live_receipt_wall_ms": {
                "a": la.first().map(|x| clock.wall_ms_at(x.3)),
                "b": lb.first().map(|x| clock.wall_ms_at(x.3)),
            },
            "mine_command_ids": {"a": a_mine.map(|x| x.to_string()), "b": b_mine.map(|x| x.to_string())},
        }),
    )
}

/// C 한 실행(별도 프로세스).
pub async fn run_c(
    site: &DepositSite,
    url: &str,
    secret: &str,
    label_c: &str,
    discoverer_label: &str,
    world: Option<&str>,
) -> (Verdict, Value) {
    let actor_c = actor_of(secret, label_c);
    let actor_a = actor_of(secret, discoverer_label);
    let w = world.filter(|w| sql::is_uuid(w));
    // 사전 조건(SC-82 ⊘): A 디스폰 행(LINGER_EXPIRED), C 의 이전 세션 0.
    // 계약 필드 이름은 `despawn_reason` 이다. 처음에 `reason` 으로 적었다가 NULL 비교로 **조용히 0** 이
    // 나왔다(계약 §0.10 의 함정 그대로) — 그래서 값을 **그대로 가져와** 세고, 키가 없으면 드러낸다.
    let despawn = w.map(|w| {
        sql::rows(&format!(
            "select coalesce(payload->>'despawn_reason', '<missing>') from domain_events \
             where world_id = '{w}' and event_type = 'SHIP_DESPAWNED' and actor_id = '{actor_a}'"
        ))
        .and_then(|rows| {
            let vals: Vec<String> = rows
                .into_iter()
                .filter_map(|r| r.into_iter().next())
                .collect();
            if vals.iter().any(|v| v == "<missing>") {
                return Err(format!(
                    "SHIP_DESPAWNED payload 에 despawn_reason 키가 없다: {vals:?}"
                ));
            }
            Ok(vals.iter().filter(|v| *v == "LINGER_EXPIRED").count() as i64)
        })
    });
    let c_prior = w.map(|w| {
        sql::scalar_i64(&format!(
            "select count(*) from domain_events where world_id = '{w}' and event_type = 'SESSION_OPENED' \
             and actor_id = '{actor_c}'"
        ))
    });
    let (_s, tok) = token::identity(secret, label_c);
    let o = run_connection(BotSpec {
        label: label_c.to_owned(),
        url: url.to_owned(),
        token: tok,
        behavior: Behavior::MineScript {
            deposit_id: site.id.clone(),
            target_m: site.position_m,
            stop_radius_m: site.radius_m + 90.0,
            fly: FlyMode::Stay,
            steps: vec![MineStep::Wait(Duration::from_secs(3))],
            grace: Duration::from_millis(500),
        },
        clock: Clock::start(),
        live_corr: None,
        capture_raw: false,
    })
    .await;
    judge_c(&o, &site.id, actor_a, despawn, c_prior)
}

type SqlFact = Option<Result<i64, String>>;

pub fn judge_c(
    o: &ConnectionOutcome,
    deposit_id: &str,
    actor_a: Uuid,
    despawn: SqlFact,
    c_prior: SqlFact,
) -> (Verdict, Value) {
    let m = &o.mining;
    let ready = m.session_ready_frame_seq;
    let backfill: Vec<(u64, Option<Uuid>)> = m
        .notices
        .iter()
        .filter(|n| n.msg.payload.delivery == "BACKFILL")
        .map(|n| (n.frame_seq, n.msg.payload.historical_event.discoverer()))
        .collect();
    let all_after_ready = ready.is_some_and(|r| backfill.iter().all(|b| b.0 > r));
    // SC-61: READY 뒤 BACKFILL ≥ 1.
    let v61 = if ready.is_none() {
        Verdict::Undecided
    } else if !backfill.is_empty() && all_after_ready {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    // SC-82: C 가 BACKFILL 로 A 의 발견 + DEPOSIT_FIELD_STATE 에서 그 광맥이 드러남.
    let has_a = backfill.iter().any(|b| b.1 == Some(actor_a));
    let revealed = m.deposits.iter().any(|d| {
        d.msg
            .payload
            .deposits
            .iter()
            .any(|x| x.deposit_id == deposit_id && x.mineral_id.is_some())
    });
    let despawn_n = despawn.as_ref().and_then(|r| r.as_ref().ok()).copied();
    let prior_n = c_prior.as_ref().and_then(|r| r.as_ref().ok()).copied();
    let v82 = if despawn_n.unwrap_or(0) < 1 || prior_n != Some(0) {
        Verdict::Undecided
    } else if has_a && revealed {
        Verdict::Pass
    } else {
        Verdict::Fail
    };
    (
        and(&[v61, v82]),
        json!({
            "items": {
                "sc_61": {"verdict": v61.as_str(), "session_ready_frame_seq": ready,
                          "backfill_frame_seqs": backfill.iter().map(|b| b.0).collect::<Vec<_>>(),
                          "backfill_count": backfill.len(), "all_after_ready": all_after_ready},
                "sc_82": {"verdict": v82.as_str(), "backfill_has_a_discovery": has_a,
                          "deposit_revealed_in_field_state": revealed,
                          "precondition_a_linger_expired_rows": despawn.map(|r| r.map_err(|e| e.to_string())),
                          "precondition_c_prior_sessions": c_prior.map(|r| r.map_err(|e| e.to_string())),
                          "actor_a": actor_a.to_string()},
            },
        }),
    )
}
