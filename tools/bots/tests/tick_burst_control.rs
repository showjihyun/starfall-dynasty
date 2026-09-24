//! SC-89 (g) — **양성 대조 자신이 겨냥한 조건을 만드는가**.
//!
//! 계약 §7a: *"관찰이 겨냥한 조건이 실제로 발생했는지를 함께 단언하지 않으면 초록불은 아무것도
//! 뜻하지 않는다."* 양성 대조도 예외가 아니다 — **위반을 만들어내지 못하는 대조는 아무것도
//! 증명하지 못한다.**
//!
//! # 이 파일은 한 번 틀렸다 (2026-09-23, qa3)
//!
//! 처음 쓸 때 나는 `RATE_LIMITED` 를 **평균 속도 문턱**으로 보고 `average_hz() < 40` 을
//! 단언했다. 그리고 "`gap = 500 ms` 가 두 문턱을 가른다"고 계약에까지 적었다.
//!
//! **실측이 그것을 뒤집었다.** 한 번 돌리니 `RATE_LIMITED` 가 **48**, 틱상한 드롭이 **9** —
//! "훨씬 작을 것"이라던 기대와 정반대였다. 코드를 읽어 보니 이유가 분명했다: **두 층이
//! 있고 둘 다 "tick 당 건수"를 본다.** 평균 속도 문턱은 **없다.**
//!
//! | 층 | 상한 | 초과하면 |
//! |---|---|---|
//! | 게이트웨이 | 8 / tick | 제출 안 함, `COMMAND_RESULT` 없음, **그 tick 에 위반 1회** |
//! | 시뮬레이션 | `40.div_ceil(20)` = **2** / tick | `RATE_LIMITED` 거부 |
//!
//! 즉 **몰아 보내기는 두 층을 반드시 함께 건드린다. 간격으로 갈라지지 않는다.**
//! 내 옛 단언은 **서버를 모델하지 않는 수식이었고, 그 초록불은 아무것도 뜻하지 않았다** —
//! 이 대조가 막으려던 바로 그 병이 대조를 검사하는 테스트 안에 있었다.
//!
//! 그래서 지금은 **비율을 유도해서 단언한다.** 유도가 실측과 맞는지는
//! `derivation_matches_the_measured_run` 이 붙잡는다 — 그 실측이 이 모델의 유일한 근거다.
//!
//! 숫자는 `scenario.rs` 에서 읽는다. **테스트에 복사하지 않는다** — 복사하면 이 테스트가
//! 사본을 검사하게 되고, 그것이 SC-89 (c) 가 막으려는 결함이다.

use std::time::Duration;

use starfall_bots::scenario::{
    ProbeCase, SERVER_TICK_COMMAND_CAP, SIM_RATE_LIMIT_PER_TICK_CAP, TickBurstPlan,
    VIOLATION_BUDGET, VIOLATION_WINDOW, tick_burst_plan,
};

#[test]
fn probe_case_round_trips() {
    let case = ProbeCase::parse("tick-burst").expect("tick-burst 를 파싱해야 한다");
    assert_eq!(case.as_str(), "tick-burst");
    assert!(
        case.contract_item().contains("SC-89"),
        "설명이 어느 항목의 대조인지 말해야 한다: {}",
        case.contract_item()
    );
}

/// 기본 계획이 **위반 경로를 실제로 밟는다**.
#[test]
fn default_plan_charges_protocol_violations() {
    let plan = tick_burst_plan(0);

    assert!(
        plan.charges_protocol_violations(),
        "게이트웨이 상한({SERVER_TICK_COMMAND_CAP})을 넘겨야 위반이 계수된다: per_tick = {}",
        plan.per_tick
    );
    assert!(
        plan.fills_violation_budget(),
        "위반 예산({VIOLATION_BUDGET}건 / {VIOLATION_WINDOW:?})을 채워야 서버가 닫는다: \
         rounds = {}, gap = {:?}",
        plan.rounds,
        plan.gap
    );
}

/// `--count` 를 키워도 두 조건이 유지된다.
#[test]
fn larger_counts_still_charge_violations() {
    for count in [1, 8, 10, 20, 40, 1000] {
        let plan = tick_burst_plan(count);
        assert!(plan.charges_protocol_violations(), "count = {count}");
        assert!(plan.fills_violation_budget(), "count = {count}");
    }
}

/// **실측과의 대조 — 이 모델의 유일한 근거다.**
///
/// 2026-09-23 전용 인스턴스(`start_tick = 473421`, `evidence/R4-B9/sc89/`)에서 기본 계획을
/// 한 번 돌린 결과. 연결은 **위반 예산이 차는 순간 닫히므로** 마지막 라운드들은 돌지 않는다 —
/// 그래서 라운드 수가 아니라 **한 라운드당 비율**을 대조한다.
///
/// | 실측 | 값 |
/// |---|---|
/// | `protocol_violations_total` | 9 |
/// | `commands_dropped_over_tick_cap_total` | 9 |
/// | `RATE_LIMITED` 델타 | 48 |
/// | `accepted` | 16 |
///
/// 유도: 라운드당 드롭 1 · `RATE_LIMITED` 6 · 수락 2. 실측 9 : 48 : 16 은 **8 라운드분**의
/// 1 : 6 : 2 와 정확히 맞는다(9번째 라운드에서 예산이 차 닫혔다).
#[test]
fn derivation_matches_the_measured_run() {
    let plan = tick_burst_plan(0);

    assert_eq!(
        plan.expected_tick_cap_drops_per_round(),
        1,
        "9건 중 8건이 통과하고 1건이 버려진다 → 그 tick 에 위반 1회"
    );
    assert_eq!(
        plan.expected_rate_limited_per_round(),
        6,
        "게이트웨이를 통과한 8건 중 {SIM_RATE_LIMIT_PER_TICK_CAP}건만 수락되고 나머지가 RATE_LIMITED"
    );

    // 실측 비율과 맞는가. 이 줄이 깨지면 모델이 서버와 어긋난 것이다.
    let rounds_until_close = 8;
    assert_eq!(
        plan.expected_tick_cap_drops_per_round() * rounds_until_close + 1,
        9,
        "실측 commands_dropped_over_tick_cap_total = 9 (닫히는 라운드 포함)"
    );
    assert_eq!(
        plan.expected_rate_limited_per_round() * rounds_until_close,
        48,
        "실측 RATE_LIMITED 델타 = 48"
    );
    assert_eq!(
        SIM_RATE_LIMIT_PER_TICK_CAP * rounds_until_close,
        16,
        "실측 accepted = 16"
    );

    // **RATE_LIMITED 가 틱상한 드롭보다 많은 것이 정상이다.** 리더가 처음 제시한 판별 기준
    // ("RATE_LIMITED 가 훨씬 작을 것")은 이 구조에서 **성립할 수 없다**.
    assert!(
        plan.expected_rate_limited_per_round() > plan.expected_tick_cap_drops_per_round(),
        "두 층이 다 tick 당 건수를 보므로 RATE_LIMITED 가 더 많이 난다 — 결함이 아니다"
    );
}

/// **고장 주입 — 대조가 망가지는 방식이 각각 빨간불을 켜는가** (§3.3).
#[test]
fn each_way_of_breaking_the_control_is_detected() {
    let good = tick_burst_plan(0);

    // ① 게이트웨이 상한을 안 넘긴다 → 위반이 한 건도 안 매겨진다.
    //    (RATE_LIMITED 는 여전히 나므로 "거부가 있었다"만 보면 속는다.)
    let no_violation = TickBurstPlan {
        per_tick: SERVER_TICK_COMMAND_CAP,
        ..good
    };
    assert!(
        !no_violation.charges_protocol_violations(),
        "상한과 같은 건수는 위반을 만들지 못한다 — 검사가 이것을 잡아야 한다"
    );
    assert!(
        no_violation.expected_rate_limited_per_round() > 0,
        "그런데도 RATE_LIMITED 는 난다 — **거부 수만 보면 이 고장을 놓친다.** \
         그래서 판정은 protocol_violations_total 로 한다"
    );

    // ② 라운드가 모자라 예산을 못 채운다 → 서버가 닫지 않는다.
    let too_short = TickBurstPlan {
        rounds: VIOLATION_BUDGET - 1,
        ..good
    };
    assert!(
        !too_short.fills_violation_budget(),
        "예산보다 적은 라운드로는 닫히지 않는다"
    );

    // ③ gap 이 너무 커서 예산 창 밖으로 나간다.
    let too_slow = TickBurstPlan {
        gap: Duration::from_millis(2000),
        ..good
    };
    assert!(
        !too_slow.fills_violation_budget(),
        "2 s × 7 = 14 s 는 10 s 창 밖이라 예산이 안 찬다"
    );
}
