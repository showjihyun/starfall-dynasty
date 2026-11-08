//! Key bindings: every keyboard action, its default key and the player's own (settings). Keys are
//! named as the engine names physical keys (`KeyW`, `Space`, `ShiftLeft` — Bevy's `KeyCode` names),
//! so the client maps them without this crate knowing the engine. The help window, the corner hints
//! and the chapter step hints all show the player's keys.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    ThrustForward,
    ThrustBack,
    TurnLeft,
    TurnRight,
    NoseUp,
    NoseDown,
    RollLeft,
    RollRight,
    Rise,
    Sink,
    Boost,
    Fire,
    Target,
    Missile,
    EscortAttack,
    EscortReturn,
    HoldPost,
    Camera,
    Bridge,
    Dock,
    Mine,
    Scan,
    Map,
    Station,
    Missions,
    Contracts,
    Chronicle,
    Fleet,
    Alliance,
    Help,
    Rebuild,
}

impl Action {
    pub const ALL: [Action; 31] = [
        Action::ThrustForward, Action::ThrustBack, Action::TurnLeft, Action::TurnRight, Action::NoseUp, Action::NoseDown, Action::RollLeft, Action::RollRight, Action::Rise, Action::Sink, Action::Boost,
        Action::Fire, Action::Target, Action::Missile, Action::EscortAttack, Action::EscortReturn, Action::HoldPost,
        Action::Camera, Action::Bridge,
        Action::Dock, Action::Mine, Action::Scan, Action::Rebuild,
        Action::Map, Action::Station, Action::Missions, Action::Contracts, Action::Chronicle, Action::Fleet, Action::Alliance, Action::Help,
    ];

    /// The id stored in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Action::ThrustForward => "thrust_forward",
            Action::ThrustBack => "thrust_back",
            Action::TurnLeft => "turn_left",
            Action::TurnRight => "turn_right",
            Action::NoseUp => "nose_up",
            Action::NoseDown => "nose_down",
            Action::RollLeft => "roll_left",
            Action::RollRight => "roll_right",
            Action::Rise => "rise",
            Action::Sink => "sink",
            Action::Boost => "boost",
            Action::Fire => "fire",
            Action::Target => "target",
            Action::Missile => "missile",
            Action::EscortAttack => "escort_attack",
            Action::EscortReturn => "escort_return",
            Action::HoldPost => "hold_post",
            Action::Camera => "camera",
            Action::Bridge => "bridge",
            Action::Dock => "dock",
            Action::Mine => "mine",
            Action::Scan => "scan",
            Action::Map => "map",
            Action::Station => "station",
            Action::Missions => "missions",
            Action::Contracts => "contracts",
            Action::Chronicle => "chronicle",
            Action::Fleet => "fleet",
            Action::Alliance => "alliance",
            Action::Help => "help",
            Action::Rebuild => "rebuild",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::ThrustForward => "추진",
            Action::ThrustBack => "역추진",
            Action::TurnLeft => "왼쪽 선회",
            Action::TurnRight => "오른쪽 선회",
            Action::NoseUp => "기수 올림",
            Action::NoseDown => "기수 내림",
            Action::RollLeft => "왼쪽 롤",
            Action::RollRight => "오른쪽 롤",
            Action::Rise => "상승",
            Action::Sink => "하강",
            Action::Boost => "부스트",
            Action::Fire => "사격 · 주포",
            Action::Target => "목표 지정",
            Action::Missile => "미사일",
            Action::EscortAttack => "호위함 공격 명령",
            Action::EscortReturn => "호위함 복귀",
            Action::HoldPost => "함대 진형 자리 지키기",
            Action::Camera => "1인칭 / 3인칭",
            Action::Bridge => "함교 화면",
            Action::Dock => "도킹 · 출항",
            Action::Mine => "채굴",
            Action::Scan => "스캔",
            Action::Map => "지도",
            Action::Station => "정거장",
            Action::Missions => "미션",
            Action::Contracts => "계약",
            Action::Chronicle => "연대기",
            Action::Fleet => "함대",
            Action::Alliance => "동맹",
            Action::Help => "도움말",
            Action::Rebuild => "재건 (격침 뒤)",
        }
    }

    /// The key it had before bindings could change (what the tutorial was written against).
    pub fn default_key(self) -> &'static str {
        match self {
            Action::ThrustForward => "KeyW",
            Action::ThrustBack => "KeyS",
            Action::TurnLeft => "KeyA",
            Action::TurnRight => "KeyD",
            Action::NoseUp => "KeyR",
            Action::NoseDown => "KeyF",
            Action::RollLeft => "KeyQ",
            Action::RollRight => "KeyE",
            Action::Rise => "KeyZ",
            Action::Sink => "ControlLeft",
            Action::Boost => "ShiftLeft",
            Action::Fire => "Space",
            Action::Target => "KeyT",
            Action::Missile => "KeyX",
            Action::EscortAttack => "KeyB",
            Action::EscortReturn => "KeyY",
            Action::HoldPost => "KeyP",
            Action::Camera => "KeyC",
            Action::Bridge => "KeyI",
            Action::Dock => "KeyG",
            Action::Mine => "KeyN",
            Action::Scan => "KeyV",
            Action::Map => "KeyM",
            Action::Station => "KeyU",
            Action::Missions => "KeyJ",
            Action::Contracts => "KeyO",
            Action::Chronicle => "KeyH",
            Action::Fleet => "KeyK",
            Action::Alliance => "KeyL",
            Action::Help => "F1",
            Action::Rebuild => "Enter",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// Keys that cannot be bound: Esc (menu, cutscene skip) and the music/mute keys.
pub const RESERVED: [&str; 4] = ["Escape", "F2", "Minus", "Equal"];

/// The player's bindings: their overrides (action id → key name) over the defaults.
pub fn key(overrides: &BTreeMap<String, String>, a: Action) -> String {
    overrides
        .get(a.id())
        .cloned()
        .unwrap_or_else(|| a.default_key().to_string())
}

/// How a key name reads on screen: `KeyW` → "W", `Digit1` → "1", `ShiftLeft` → "Shift", `ArrowUp` → "↑".
pub fn display(key: &str) -> String {
    let named = match key {
        "Space" => "Space",
        "Enter" => "Enter",
        "Tab" => "Tab",
        "Backspace" => "Backspace",
        "ShiftLeft" | "ShiftRight" => "Shift",
        "ControlLeft" | "ControlRight" => "Ctrl",
        "AltLeft" | "AltRight" => "Alt",
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Backquote" => "`",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Backslash" => "\\",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    key.strip_prefix("Key").or_else(|| key.strip_prefix("Digit")).unwrap_or(key).to_string()
}

/// The display key of an action for this player.
pub fn shown(overrides: &BTreeMap<String, String>, a: Action) -> String {
    display(&key(overrides, a))
}

/// Pairs of actions bound to the same key (the settings window marks them).
pub fn conflicts(overrides: &BTreeMap<String, String>) -> Vec<(Action, Action)> {
    let mut out = vec![];
    for (i, a) in Action::ALL.iter().enumerate() {
        for b in &Action::ALL[i + 1..] {
            if key(overrides, *a) == key(overrides, *b) {
                out.push((*a, *b));
            }
        }
    }
    out
}

/// Bind `a` to `key` (a default binding is stored as no override). Reserved keys are refused.
pub fn bind(overrides: &mut BTreeMap<String, String>, a: Action, key: &str) -> bool {
    if RESERVED.contains(&key) {
        return false;
    }
    if key == a.default_key() {
        overrides.remove(a.id());
    } else {
        overrides.insert(a.id().to_string(), key.to_string());
    }
    true
}
