//! Gamepad flight (an Xbox-style pad; Steam Deck): sticks and triggers to the same controls the
//! keyboard sends. Left stick: thrust (up) and turn; right stick: nose (up) and roll; right trigger
//! fires, left trigger boosts; bumpers sink (left) and rise (right). Buttons: A target · B dock ·
//! X scan · Y missile · Select map · Start menu (the client maps them).

/// Stick travel ignored around the centre (worn sticks drift).
pub const DEADZONE: f32 = 0.15;
/// How far a trigger goes before it counts as pulled.
pub const TRIGGER: f32 = 0.3;

/// The pad's analog state, -1..1 sticks (up and right positive), 0..1 triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadState {
    pub left: (f32, f32),
    pub right: (f32, f32),
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_bumper: bool,
    pub right_bumper: bool,
}

/// What the pad asks for this frame: (thrust, yaw, pitch, roll, rise, fire, boost).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadFlight {
    pub thrust: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub rise: f32,
    pub fire: bool,
    pub boost: bool,
}

/// A stick axis past the deadzone, rescaled so the edge of the deadzone is 0 and full travel 1.
pub fn axis(v: f32) -> f32 {
    let a = v.abs();
    if !v.is_finite() || a <= DEADZONE {
        return 0.0;
    }
    v.signum() * ((a - DEADZONE) / (1.0 - DEADZONE)).min(1.0)
}

pub fn flight(p: &PadState) -> PadFlight {
    PadFlight {
        thrust: axis(p.left.1),
        yaw: axis(p.left.0),
        pitch: axis(p.right.1),
        roll: axis(p.right.0),
        rise: (p.right_bumper as i32 - p.left_bumper as i32) as f32,
        fire: p.right_trigger > TRIGGER,
        boost: p.left_trigger > TRIGGER,
    }
}

/// The help window's gamepad rows (button, what it does).
pub const HELP: [(&str, &str); 11] = [
    ("왼쪽 스틱", "추진 · 선회"),
    ("오른쪽 스틱", "기수 · 롤"),
    ("RT / LT", "사격 · 부스트"),
    ("RB / LB", "상승 · 하강"),
    ("A", "목표 지정"),
    ("B", "도킹 · 출항"),
    ("X", "스캔"),
    ("십자 위", "탐지 펄스"),
    ("Y", "미사일"),
    ("Select", "지도"),
    ("Start", "메뉴"),
];
