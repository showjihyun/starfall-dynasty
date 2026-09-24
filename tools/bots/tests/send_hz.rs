//! `--send-hz` 의 대조 (계약 §3.1, SC-25 / 리더 판정 R24).
//!
//! **방어를 넣었으면 그 방어가 걸려야 할 입력에서 실제로 걸리는지 보인다**(계약 §7b 규칙 6).
//! 여기서 재는 것은 둘이다: (1) 봇별로 **다른** 주기가 나오는가 — 같은 값이 나오면
//! SC-25 는 두 속도를 비교하는 것이 아니라 같은 속도를 두 번 재는 것이 된다.
//! (2) 목록이 짧을 때 나머지 봇이 **마지막 값**을 받는가.

use starfall_bots::scenario::send_interval;

#[test]
fn per_bot_rates_differ_when_two_values_are_given() {
    let hz = vec![20.0, 200.0];
    let a = send_interval(&hz, 0);
    let b = send_interval(&hz, 1);
    assert_eq!(a.as_millis(), 50, "20 Hz 는 50 ms 여야 한다");
    assert_eq!(b.as_millis(), 5, "200 Hz 는 5 ms 여야 한다");
    assert_ne!(
        a, b,
        "두 봇의 주기가 같으면 SC-25 는 아무것도 대조하지 않는다"
    );
}

#[test]
fn short_list_applies_last_value_to_remaining_bots() {
    let hz = vec![20.0];
    assert_eq!(send_interval(&hz, 0), send_interval(&hz, 29));
    assert_eq!(send_interval(&hz, 29).as_millis(), 50);
}

#[test]
fn default_is_not_the_ping_interval() {
    // 옛 코드는 `cfg.interval`(기본 500 ms = 2 Hz)을 조작 송신에 썼다. 계약이 말하는
    // 기본값은 **20 Hz** 이고, 둘을 섞으면 계약과 무관한 수를 재게 된다.
    assert_eq!(send_interval(&[20.0], 0).as_millis(), 50);
    assert_ne!(send_interval(&[20.0], 0).as_millis(), 500);
}
