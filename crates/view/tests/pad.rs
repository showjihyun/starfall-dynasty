//! Gamepad flight: sticks, deadzone and triggers to the keyboard's controls.

use view::pad::{DEADZONE, PadState, axis, flight};

#[test]
fn sticks_have_a_deadzone_and_reach_full_travel() {
    assert_eq!(axis(0.0), 0.0);
    assert_eq!(axis(DEADZONE * 0.9), 0.0, "drift is ignored");
    assert_eq!(axis(1.0), 1.0);
    assert_eq!(axis(-1.0), -1.0);
    assert!((axis(0.5) - (0.5 - DEADZONE) / (1.0 - DEADZONE)).abs() < 1e-6, "no jump at the deadzone's edge");
    assert_eq!(axis(f32::NAN), 0.0);
}

#[test]
fn a_pad_flies_like_the_keyboard() {
    let idle = flight(&PadState::default());
    assert_eq!((idle.thrust, idle.yaw, idle.pitch, idle.roll, idle.rise, idle.fire, idle.boost), (0.0, 0.0, 0.0, 0.0, 0.0, false, false));
    let f = flight(&PadState { left: (1.0, 1.0), right: (-1.0, -1.0), left_trigger: 0.8, right_trigger: 1.0, right_bumper: true, ..Default::default() });
    assert_eq!((f.thrust, f.yaw, f.pitch, f.roll, f.rise), (1.0, 1.0, -1.0, -1.0, 1.0), "stick up = forward / nose up, right = turn / roll right");
    assert!(f.fire && f.boost);
    assert!(!flight(&PadState { right_trigger: 0.2, ..Default::default() }).fire, "a trigger resting on the finger does not fire");
}
