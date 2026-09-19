//! **계측기 자체의 검증.**
//!
//! 봇 하네스가 틀리면 이 슬라이스의 부하 판정(SC-52~69)이 통째로 거짓이 된다. 서버 없이
//! 합성 프레임을 넣어 "틀렸을 때 실제로 빨간불이 켜지는가"를 확인한다. 통과가 아니라
//! **실패를 만들어 보는 것**이 이 파일의 목적이다.

use starfall_bots::ledger::{Ledger, STATUS_ACCEPTED, STATUS_REJECTED, SessionRecord};
use starfall_bots::stats;
use uuid::Uuid;

/// 정상 왕복 N건: 모든 게이트가 통과하고 왕복 지연이 reply 기준으로 계산된다.
#[test]
fn healthy_run_passes_all_gates() {
    let mut l = Ledger::new("bot-000");
    let mut ids = Vec::new();
    for i in 0..10u32 {
        let id = Uuid::now_v7();
        ids.push(id);
        l.on_sent(id, i, 1_000 * u64::from(i));
    }
    for (i, id) in ids.iter().enumerate() {
        let base = 1_000 * i as u64;
        l.on_command_result(*id, STATUS_ACCEPTED, None, base + 20_000, 10);
        l.on_ping_reply(*id, i as u32, base + 30_000, 10);
    }
    let s = l.finish();
    assert_eq!(s.sent_total, 10);
    assert_eq!(s.results_total, 10);
    assert_eq!(s.replies_total, 10);
    assert_eq!(s.missing_results, 0);
    assert_eq!(s.missing_replies, 0);
    assert_eq!(s.order_violations, 0);
    assert!(s.one_to_one_holds(), "정상 실행인데 1:1 게이트가 깨졌다");
    assert!(s.accepted_reply_pairing_holds());
    // 왕복은 sent → reply 다. result 시각(20 ms)이 아니라 reply 시각(30 ms)을 써야 한다.
    assert_eq!(s.rtt.count, 10);
    assert_eq!(
        s.rtt.p50_ms,
        Some(30.0),
        "rtt 는 PING_REPLY 기준이어야 한다"
    );
    assert_eq!(
        s.ack.p50_ms,
        Some(20.0),
        "ack 는 COMMAND_RESULT 기준이어야 한다"
    );
}

/// **손실 1건을 의도적으로 만들면 손실 1로 잡히는가.** (리더 요구 항목)
#[test]
fn one_dropped_command_result_is_counted_as_loss() {
    let mut l = Ledger::new("bot-000");
    let ids: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
    for (i, id) in ids.iter().enumerate() {
        l.on_sent(*id, i as u32, 0);
    }
    // 5건 중 4건만 응답이 온다 — index 2 의 COMMAND_RESULT 를 서버가 안 보냈다고 하자.
    for (i, id) in ids.iter().enumerate() {
        if i == 2 {
            continue;
        }
        l.on_command_result(*id, STATUS_ACCEPTED, None, 10_000, 10);
        l.on_ping_reply(*id, i as u32, 20_000, 10);
    }
    let s = l.finish();
    assert_eq!(s.sent_total, 5);
    assert_eq!(s.results_total, 4);
    assert_eq!(s.missing_results, 1, "손실 1건이 잡히지 않았다");
    assert!(
        !s.one_to_one_holds(),
        "손실이 있는데 1:1 게이트가 통과했다 — 계측기가 쓸모없다는 뜻이다"
    );
}

/// ACCEPTED 인데 PING_REPLY 가 오지 않은 경우도 따로 잡힌다.
#[test]
fn missing_ping_reply_is_counted_separately() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 0, 0);
    l.on_command_result(id, STATUS_ACCEPTED, None, 10_000, 10);
    let s = l.finish();
    assert_eq!(s.missing_results, 0);
    assert_eq!(s.missing_replies, 1);
    assert!(
        s.one_to_one_holds(),
        "COMMAND_RESULT 는 왔으므로 1:1 은 성립한다"
    );
    assert!(!s.accepted_reply_pairing_holds());
}

/// 중복 응답(같은 command_id 로 COMMAND_RESULT 2건)은 중복으로 잡힌다 — I-15 위반.
#[test]
fn duplicate_command_result_is_counted() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 0, 0);
    l.on_command_result(id, STATUS_ACCEPTED, None, 10_000, 10);
    l.on_command_result(id, STATUS_ACCEPTED, None, 11_000, 10);
    l.on_ping_reply(id, 0, 12_000, 10);
    let s = l.finish();
    assert_eq!(s.duplicate_results, 1);
    assert!(!s.one_to_one_holds());
}

/// 서버의 중복 제거가 깨져 PING_REPLY 가 2건 오면 잡힌다 (AC-7b 의 반대 경우).
#[test]
fn duplicate_ping_reply_is_counted() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 7, 0);
    l.on_command_result(id, STATUS_ACCEPTED, None, 10_000, 10);
    l.on_ping_reply(id, 7, 12_000, 10);
    l.on_ping_reply(id, 7, 13_000, 10);
    let s = l.finish();
    assert_eq!(s.replies_total, 2);
    assert_eq!(s.duplicate_replies, 1);
    assert!(!s.accepted_reply_pairing_holds());
}

/// **I-15 위반: PING_REPLY 가 COMMAND_RESULT 보다 먼저 오면 잡히는가.**
#[test]
fn reply_before_result_is_an_order_violation() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 0, 0);
    l.on_ping_reply(id, 0, 10_000, 10); // 먼저 왔다
    l.on_command_result(id, STATUS_ACCEPTED, None, 11_000, 10);
    let s = l.finish();
    assert_eq!(s.order_violations, 1, "순서 위반이 잡히지 않았다");
    // 순서가 틀려도 1:1 자체는 성립한다 — 두 성질은 독립이어야 한다.
    assert!(s.one_to_one_holds());
}

/// 보낸 적 없는 command_id 로 온 응답(서버가 만들어낸 것)은 미대응으로 잡힌다.
#[test]
fn unmatched_frames_are_counted() {
    let mut l = Ledger::new("bot-000");
    let sent = Uuid::now_v7();
    let ghost = Uuid::now_v7();
    l.on_sent(sent, 0, 0);
    l.on_command_result(sent, STATUS_ACCEPTED, None, 10_000, 10);
    l.on_ping_reply(sent, 0, 11_000, 10);
    l.on_command_result(ghost, STATUS_ACCEPTED, None, 12_000, 10);
    l.on_ping_reply(ghost, 99, 13_000, 10);
    let s = l.finish();
    assert_eq!(s.unmatched_results, 1);
    assert_eq!(s.unmatched_replies, 1);
    assert!(!s.one_to_one_holds());
}

/// probe_seq 가 어긋난 PING_REPLY 는 조용히 통과하지 않는다.
#[test]
fn probe_seq_mismatch_is_counted() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 5, 0);
    l.on_command_result(id, STATUS_ACCEPTED, None, 10_000, 10);
    l.on_ping_reply(id, 6, 11_000, 10); // 다른 probe_seq
    let s = l.finish();
    assert_eq!(s.probe_seq_mismatches, 1);
    assert!(!s.accepted_reply_pairing_holds());
}

/// 거부는 손실이 아니다 — 거부도 응답이므로 1:1 이 성립한다(AC-18a 의 핵심).
#[test]
fn rejections_are_responses_not_losses() {
    let mut l = Ledger::new("bot-029");
    for i in 0..100u32 {
        let id = Uuid::now_v7();
        l.on_sent(id, i, 0);
        if i < 64 {
            l.on_command_result(id, STATUS_ACCEPTED, None, 10_000, 10);
            l.on_ping_reply(id, i, 11_000, 10);
        } else {
            l.on_command_result(id, STATUS_REJECTED, Some("TOO_MANY_IN_FLIGHT"), 10_000, 10);
        }
    }
    let s = l.finish();
    assert_eq!(s.sent_total, 100);
    assert_eq!(s.results_total, 100);
    assert_eq!(s.missing_results, 0);
    assert_eq!(s.accepted, 64);
    assert_eq!(s.rejected, 36);
    assert_eq!(s.rejected_by_reason.get("TOO_MANY_IN_FLIGHT"), Some(&36));
    assert!(s.one_to_one_holds(), "거부는 손실이 아니다");
    assert!(s.accepted_reply_pairing_holds());
}

/// 닫힌 집합 밖의 status 는 죽지 않고 드러난다(ADR-0005 §4 의 소비자 관용).
#[test]
fn unknown_status_is_surfaced_not_crashed() {
    let mut l = Ledger::new("bot-000");
    let id = Uuid::now_v7();
    l.on_sent(id, 0, 0);
    l.on_command_result(id, "DEFERRED_IN_A_FUTURE_VERSION", None, 10_000, 10);
    let s = l.finish();
    assert_eq!(s.unknown_status, 1);
    assert_eq!(s.accepted, 0);
    assert_eq!(s.rejected, 0);
    assert_eq!(s.results_total, 1);
}

/// AC-19(b): 중단 구간에 보낸 명령이 창 기준으로 집계되는가.
#[test]
fn window_slicing_counts_round_trips_inside_the_outage() {
    let mut l = Ledger::new("bot-000");
    // 창 밖(중단 전)
    let before = Uuid::now_v7();
    l.on_sent(before, 0, 1_000_000);
    l.on_command_result(before, STATUS_ACCEPTED, None, 1_010_000, 10);
    l.on_ping_reply(before, 0, 1_020_000, 10);
    // 창 안(중단 중) — 왕복은 계속되어야 한다
    for i in 0..3u32 {
        let id = Uuid::now_v7();
        let t = 5_000_000 + u64::from(i) * 100_000;
        l.on_sent(id, i, t);
        l.on_command_result(id, STATUS_ACCEPTED, None, t + 10_000, 10);
        l.on_ping_reply(id, i, t + 20_000, 10);
    }
    // 창 안이지만 응답이 오지 않은 1건
    let stuck = Uuid::now_v7();
    l.on_sent(stuck, 9, 5_400_000);

    let (sent, completed) = l.round_trips_in_window(5_000_000, 6_000_000);
    assert_eq!(sent, 4);
    assert_eq!(completed, 3);
    let (sent_all, _) = l.round_trips_in_window(0, u64::MAX);
    assert_eq!(sent_all, 5);
}

/// nearest-rank 분위수가 **실제 표본**을 돌려주는지. 보간하면 원본으로 되짚을 수 없다.
#[test]
fn percentiles_are_nearest_rank_and_return_real_samples() {
    let samples: Vec<f64> = (1..=100).map(f64::from).collect();
    let s = stats::summarize(samples);
    assert_eq!(s.count, 100);
    assert_eq!(s.min_ms, Some(1.0));
    assert_eq!(s.p50_ms, Some(50.0));
    assert_eq!(s.p90_ms, Some(90.0));
    assert_eq!(s.p99_ms, Some(99.0));
    assert_eq!(s.max_ms, Some(100.0));

    // 표본 1건이면 모든 분위수가 그 값이다 (0건 나눗셈·패닉이 없어야 한다).
    let one = stats::summarize(vec![7.5]);
    assert_eq!(one.p99_ms, Some(7.5));
    // 0건은 None — "측정 못 했다"와 "0 ms"를 구분한다.
    let none = stats::summarize(vec![]);
    assert_eq!(none.count, 0);
    assert_eq!(none.p50_ms, None);
}

/// 여러 봇을 합칠 때 분위수를 **표본에서 다시 계산**하는가(요약끼리 평균내면 거짓이 된다).
#[test]
fn aggregate_recomputes_percentiles_from_raw_samples() {
    let mut a = Ledger::new("bot-000");
    let mut b = Ledger::new("bot-001");
    for (l, base) in [(&mut a, 1_000u64), (&mut b, 100_000u64)] {
        for i in 0..50u32 {
            let id = Uuid::now_v7();
            l.on_sent(id, i, 0);
            l.on_command_result(id, STATUS_ACCEPTED, None, base / 2, 10);
            l.on_ping_reply(id, i, base, 10);
        }
    }
    let sa = a.finish();
    let sb = b.finish();
    let mut samples = a.rtt_samples_ms();
    samples.extend(b.rtt_samples_ms());
    let agg = starfall_bots::ledger::aggregate(&[sa, sb], samples, vec![]);
    assert_eq!(agg.sent_total, 100);
    assert_eq!(agg.rtt.count, 100);
    // 50건이 1 ms, 50건이 100 ms → p50 = 1 ms, p99 = 100 ms (요약 평균이면 50.5 가 나온다)
    assert_eq!(agg.rtt.p50_ms, Some(1.0));
    assert_eq!(agg.rtt.p99_ms, Some(100.0));
}

/// **실서버에서 드러난 회귀(2026-09-19)**: 정상 종료는 Close 프레임 *교환*이라 우리가 먼저 닫아도
/// 서버의 Close 가 뒤따라 온다. 나중 값으로 덮어쓰면 모든 정상 세션이 `initiator=server` 가 되어
/// SC-52("서버가 먼저 닫은 연결 0건")가 거짓 FAIL 이 된다.
#[test]
fn close_initiator_is_decided_by_whoever_closed_first() {
    let mut l = Ledger::new("bot-000");
    l.on_session_ready(SessionRecord {
        bot: "bot-000".to_owned(),
        session_id: Uuid::now_v7(),
        correlation_id: Some(Uuid::now_v7()),
        actor_id: Uuid::now_v7(),
        tick_hz: 20,
        server_version: "0.1.0".to_owned(),
        ready_tick: 1,
        connected_at_us: 0,
        ready_at_us: 10,
        closed_at_us: None,
        close_code: None,
        peer_close_code: None,
        close_reason_text: None,
        close_initiator: "none".to_owned(),
    });

    // 봇이 먼저 Close 를 보냈다 → 그 뒤 서버의 Close 응답이 도착한다.
    l.on_close(1_000, Some(1000), None, "client");
    l.on_close(1_500, Some(1000), Some(String::new()), "server");

    let s = l.session.as_ref().expect("session");
    assert_eq!(
        s.close_initiator, "client",
        "먼저 닫은 쪽이 기록되어야 한다"
    );
    assert_eq!(s.closed_at_us, Some(1_000), "시각도 첫 기록을 유지한다");
    assert_eq!(
        s.peer_close_code,
        Some(1000),
        "상대가 보낸 code 는 따로 남는다"
    );
}

/// 반대 경우: 서버가 먼저 닫으면(SLOW_CONSUMER·IDLE_TIMEOUT 등) 그대로 server 로 남는다.
#[test]
fn server_initiated_close_is_recorded_as_server() {
    let mut l = Ledger::new("bot-029");
    l.on_session_ready(SessionRecord {
        bot: "bot-029".to_owned(),
        session_id: Uuid::now_v7(),
        correlation_id: Some(Uuid::now_v7()),
        actor_id: Uuid::now_v7(),
        tick_hz: 20,
        server_version: "0.1.0".to_owned(),
        ready_tick: 1,
        connected_at_us: 0,
        ready_at_us: 10,
        closed_at_us: None,
        close_code: None,
        peer_close_code: None,
        close_reason_text: None,
        close_initiator: "none".to_owned(),
    });
    l.on_close(
        2_000,
        Some(1011),
        Some("slow consumer".to_owned()),
        "server",
    );
    let s = l.session.as_ref().expect("session");
    assert_eq!(s.close_initiator, "server");
    assert_eq!(s.close_code, Some(1011));
}
