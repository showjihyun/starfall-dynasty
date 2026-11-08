//! Player settings (sound levels, display) and the system menu (Esc): what a shipped PC game is
//! expected to offer before anything else. Kept here as plain data and egui drawing so it is tested
//! headless; the client stores the file in the per-user config dir and applies it to the mixer and
//! the window.

use crate::keymap::{self, Action};
use crate::palette::{DIM, GOLD, RED};
use egui::RichText;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Display {
    #[default]
    Windowed,
    /// A window the size of the screen, no border (alt-tab friendly).
    Borderless,
    Fullscreen,
}

impl Display {
    pub const ALL: [Display; 3] = [Display::Windowed, Display::Borderless, Display::Fullscreen];

    pub fn name(self) -> &'static str {
        match self {
            Display::Windowed => "창 모드",
            Display::Borderless => "테두리 없는 창",
            Display::Fullscreen => "전체 화면",
        }
    }
}

/// Graphics quality: what a slower PC turns off first (the client maps it to sun shadows, bloom and
/// multisampling; High is the look the art reference was made with).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "낮음",
            Quality::Medium => "중간",
            Quality::High => "높음",
        }
    }

    /// Sun shadows, bloom, 4× multisampling.
    pub fn features(self) -> (bool, bool, bool) {
        match self {
            Quality::Low => (false, false, false),
            Quality::Medium => (false, true, true),
            Quality::High => (true, true, true),
        }
    }

    pub fn parse(s: &str) -> Option<Quality> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(Quality::Low),
            "medium" => Some(Quality::Medium),
            "high" => Some(Quality::High),
            _ => None,
        }
    }
}

/// The UI scale's range.
pub const UI_SCALE: (f32, f32) = (0.75, 1.5);

/// Levels are 0–1; `muted` silences everything without losing them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub master: f32,
    pub music: f32,
    pub effects: f32,
    pub voice: f32,
    pub muted: bool,
    pub display: Display,
    pub vsync: bool,
    pub quality: Quality,
    /// Size of every window, panel and label (egui zoom), 0.75–1.5: big screens, tired eyes.
    pub ui_scale: f32,
    /// Frame rate next to the ping, top left (off for players by default).
    pub show_fps: bool,
    /// Key bindings the player changed: action id → key name (`view::keymap`; the rest are defaults).
    pub keys: std::collections::BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        // Music under the effects, as the mix was tuned (music.rs used 0.45).
        Settings { master: 1.0, music: 0.45, effects: 1.0, voice: 1.0, muted: false, display: Display::Windowed, vsync: true, quality: Quality::High, ui_scale: 1.0, show_fps: false, keys: Default::default() }
    }
}

impl Settings {
    /// Read a settings file; anything missing or broken falls back to the default (a bad file must
    /// never keep the game from starting).
    pub fn parse(text: &str) -> Settings {
        let mut s: Settings = ron::from_str(text).unwrap_or_default();
        s.clamp();
        s
    }

    pub fn to_text(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).unwrap_or_default()
    }

    pub fn clamp(&mut self) {
        for v in [&mut self.master, &mut self.music, &mut self.effects, &mut self.voice] {
            *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 };
        }
        self.ui_scale = if self.ui_scale.is_finite() { self.ui_scale.clamp(UI_SCALE.0, UI_SCALE.1) } else { 1.0 };
        // Unknown actions (an older or newer file) and reserved keys are dropped.
        self.keys.retain(|id, key| Action::from_id(id).is_some() && !keymap::RESERVED.contains(&key.as_str()));
    }

    fn gain(&self, level: f32) -> f32 {
        if self.muted { 0.0 } else { self.master * level }
    }

    /// What the mixer multiplies each kind of sound by.
    pub fn music_gain(&self) -> f32 {
        self.gain(self.music)
    }

    pub fn effects_gain(&self) -> f32 {
        self.gain(self.effects)
    }

    pub fn voice_gain(&self) -> f32 {
        self.gain(self.voice)
    }
}

/// The settings window's contents; true when something changed (the client then saves and applies).
/// `listening`: the action waiting for its new key — clicking a key sets it, the client fills it from
/// the next key pressed (`keymap::bind`).
pub fn settings_ui(ui: &mut egui::Ui, s: &mut Settings, listening: &mut Option<Action>) -> bool {
    let before = s.clone();
    ui.label(RichText::new("소리").strong());
    egui::Grid::new("sound").num_columns(2).show(ui, |ui| {
        for (name, v) in [("전체", &mut s.master), ("음악", &mut s.music), ("효과음", &mut s.effects), ("음성", &mut s.voice)] {
            ui.label(name);
            ui.add(egui::Slider::new(v, 0.0..=1.0).show_value(false));
            ui.end_row();
        }
    });
    ui.checkbox(&mut s.muted, "음소거 (F2)");
    ui.separator();
    ui.label(RichText::new("화면").strong());
    ui.horizontal(|ui| {
        for d in Display::ALL {
            ui.radio_value(&mut s.display, d, d.name());
        }
    });
    ui.horizontal(|ui| {
        ui.label("그래픽 품질");
        for q in Quality::ALL {
            ui.radio_value(&mut s.quality, q, q.name());
        }
    })
    .response
    .on_hover_text("낮음: 그림자·블룸·안티에일리어싱 끔, 중간: 그림자 끔");
    ui.horizontal(|ui| {
        ui.label("UI 크기");
        ui.add(egui::Slider::new(&mut s.ui_scale, UI_SCALE.0..=UI_SCALE.1).step_by(0.05).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
    });
    ui.checkbox(&mut s.vsync, "수직 동기화");
    ui.checkbox(&mut s.show_fps, "프레임 수(FPS) 표시");
    ui.separator();
    let clash: Vec<Action> = keymap::conflicts(&s.keys).into_iter().flat_map(|(a, b)| [a, b]).collect();
    // Folded until opened (or while a key is awaited): the window then fits a 720-line screen.
    egui::CollapsingHeader::new(RichText::new("조작 키").strong()).open(listening.is_some().then_some(true)).show(ui, |ui| {
        if !s.keys.is_empty() && ui.small_button("모두 기본값으로").clicked() {
            s.keys.clear();
        }
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            egui::Grid::new("keys").num_columns(2).striped(true).show(ui, |ui| {
                for a in Action::ALL {
                    ui.label(RichText::new(a.label()).color(if clash.contains(&a) { RED } else { ui.visuals().text_color() }));
                    let text = if *listening == Some(a) { "키를 누르세요…".to_string() } else { keymap::shown(&s.keys, a) };
                    if ui.add_sized([120.0, 20.0], egui::Button::new(text)).clicked() {
                        *listening = if *listening == Some(a) { None } else { Some(a) };
                    }
                    ui.end_row();
                }
            });
        });
        if !clash.is_empty() {
            ui.label(RichText::new("빨간 동작은 같은 키를 쓰고 있습니다 — 둘 다 그 키에 반응합니다.").size(11.0).color(RED));
        }
        ui.label(RichText::new("화살표 키는 언제나 선회·기수, Alt + 선회 키는 좌우 평행이동입니다. Esc·F2·−·=는 바꿀 수 없습니다.").size(11.0).color(DIM));
    });
    ui.label(RichText::new("설정은 바로 적용되고 저장됩니다.").size(11.0).color(DIM));
    s.clamp();
    *s != before
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Resume,
    Settings,
    Help,
    Quit,
}

/// The system menu (Esc): resume, settings, help, quit. Returns what was clicked.
pub fn system_menu(ctx: &egui::Context) -> Option<MenuChoice> {
    let mut out = None;
    egui::Window::new("메뉴").collapsible(false).resizable(false).title_bar(false).anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO).show(ctx, |ui| {
        ui.set_width(220.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("STARFALL DYNASTY").color(GOLD).strong());
            ui.add_space(6.0);
            for (label, c) in [("계속하기", MenuChoice::Resume), ("설정", MenuChoice::Settings), ("도움말 (F1)", MenuChoice::Help), ("게임 종료", MenuChoice::Quit)] {
                if ui.add_sized([200.0, 30.0], egui::Button::new(label)).clicked() {
                    out = Some(c);
                }
            }
            ui.add_space(4.0);
            ui.label(RichText::new("게임은 멈추지 않습니다 — 다른 함장들의 세계는 계속 돌아갑니다. 다음 접속은 마지막 성계의 정거장에서 시작합니다.").size(11.0).color(DIM));
        });
    });
    out
}
