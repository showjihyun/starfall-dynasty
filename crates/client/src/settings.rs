//! Player settings (`view::settings`): loaded from the per-user config dir at start, applied to the
//! window when they change, saved when the player changes them. Automated runs (screenshots,
//! showroom, replays, benches) use the defaults and never touch the file or the window mode, so the
//! same command keeps giving the same image.

use crate::state::Session;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use std::collections::HashMap;
use view::keymap::Action;
use view::settings::{Display, Settings};

/// The keys the game can bind (named as `view::keymap` names them: `KeyCode`'s own names).
const BINDABLE: [KeyCode; 75] = [
    KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG, KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM,
    KeyCode::KeyN, KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU, KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
    KeyCode::F1, KeyCode::F3, KeyCode::F4, KeyCode::F5, KeyCode::F6, KeyCode::F7, KeyCode::F8, KeyCode::F9, KeyCode::F10, KeyCode::F11, KeyCode::F12,
    KeyCode::Space, KeyCode::Enter, KeyCode::Tab, KeyCode::Backspace, KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::AltLeft, KeyCode::AltRight,
    KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::Backquote, KeyCode::BracketLeft, KeyCode::BracketRight, KeyCode::Semicolon, KeyCode::Quote,
    KeyCode::Comma, KeyCode::Period, KeyCode::Slash, KeyCode::Backslash, KeyCode::CapsLock, KeyCode::Insert, KeyCode::Delete, KeyCode::Home, KeyCode::End,
];

fn code(name: &str) -> Option<KeyCode> {
    BINDABLE.into_iter().find(|k| format!("{k:?}") == name)
}

/// Every action's key, from the settings (rebuilt when they change).
#[derive(Resource)]
pub struct KeyBinds(HashMap<Action, KeyCode>);

impl KeyBinds {
    pub fn from(s: &Settings) -> Self {
        KeyBinds(Action::ALL.into_iter().map(|a| (a, code(&view::keymap::key(&s.keys, a)).or_else(|| code(a.default_key())).unwrap_or(KeyCode::F12))).collect())
    }

    pub fn key(&self, a: Action) -> KeyCode {
        self.0[&a]
    }
}

/// The action whose key the settings window waits for.
#[derive(Resource, Default)]
pub struct Listening(pub Option<Action>);

#[derive(Resource)]
pub struct ClientSettings {
    pub s: Settings,
    /// Changed by the player since the last save.
    pub dirty: bool,
    /// Screenshots, showroom, replay, bench: defaults, nothing saved or applied to the window.
    pub automated: bool,
}

impl ClientSettings {
    pub fn load(automated: bool) -> Self {
        let s = if automated { Settings::default() } else { path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| Settings::parse(&t)).unwrap_or_default() };
        ClientSettings { s, dirty: false, automated }
    }

    pub fn changed(&mut self) {
        self.dirty = true;
    }
}

/// `<config>/StarfallDynasty/settings.ron`.
fn path() -> Option<std::path::PathBuf> {
    crate::state::config_dir().map(|d| d.join("settings.ron"))
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Listening>().add_systems(PreUpdate, capture_key).add_systems(PostUpdate, (apply, quit, rebind, apply_quality));
    }
}

/// The window follows the display settings; a player's change is written out.
fn apply(mut cs: ResMut<ClientSettings>, mut windows: Query<&mut Window, With<PrimaryWindow>>, mut applied: Local<Option<(Display, bool)>>) {
    if cs.automated {
        return;
    }
    let want = (cs.s.display, cs.s.vsync);
    if *applied != Some(want)
        && let Ok(mut w) = windows.single_mut()
    {
        w.mode = match want.0 {
            Display::Windowed => WindowMode::Windowed,
            Display::Borderless => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            Display::Fullscreen => WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Current),
        };
        if std::env::var("STARFALL_NOVSYNC").is_err() {
            w.present_mode = if want.1 { PresentMode::AutoVsync } else { PresentMode::AutoNoVsync };
        }
        *applied = Some(want);
    }
    if cs.dirty {
        cs.dirty = false;
        if let Some(p) = path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Err(e) = std::fs::write(&p, cs.s.to_text()) {
                warn!("settings not saved to {}: {e}", p.display());
            }
        }
    }
}

/// The settings window waits for a key: the next key pressed is bound (Esc cancels; reserved keys are refused).
fn capture_key(keys: Option<ResMut<ButtonInput<KeyCode>>>, mut listening: ResMut<Listening>, mut cs: ResMut<ClientSettings>) {
    let (Some(a), Some(mut keys)) = (listening.0, keys) else { return };
    let Some(k) = keys.get_just_pressed().next().copied() else { return };
    // Taken for the binding: the key does not also open a window or the menu this frame.
    keys.clear_just_pressed(k);
    listening.0 = None;
    if k == KeyCode::Escape {
        return;
    }
    if view::keymap::bind(&mut cs.s.keys, a, &format!("{k:?}")) {
        cs.changed();
    }
}

/// New bindings take effect at once.
fn rebind(mut commands: Commands, cs: Res<ClientSettings>, binds: Option<Res<KeyBinds>>) {
    if binds.is_none() || cs.is_changed() {
        commands.insert_resource(KeyBinds::from(&cs.s));
    }
}

/// Graphics quality (`view::settings::Quality`): sun shadows, bloom and multisampling on the main camera.
/// Runs for automated runs too (a bench may ask for `--quality low`); their default is High, the art
/// reference look.
fn apply_quality(mut commands: Commands, cs: Res<ClientSettings>, mut suns: Query<&mut DirectionalLight, With<crate::scene::StarLight>>, cams: Query<(Entity, Has<bevy::post_process::bloom::Bloom>, Option<&Msaa>), With<crate::control::MainCamera>>) {
    let (shadows, bloom, msaa) = cs.s.quality.features();
    let shadows = shadows && std::env::var("STARFALL_NO_SHADOWS").is_err();
    for mut l in &mut suns {
        if l.shadow_maps_enabled != shadows {
            l.shadow_maps_enabled = shadows;
        }
    }
    for (cam, has_bloom, m) in &cams {
        if has_bloom != bloom {
            if bloom {
                commands.entity(cam).insert(crate::control::main_bloom());
            } else {
                commands.entity(cam).remove::<bevy::post_process::bloom::Bloom>();
            }
        }
        // Ray-traced lighting needs MSAA off (raytracing.rs): that build leaves it alone.
        let want = if msaa && !cfg!(feature = "raytracing_dlss") { Msaa::Sample4 } else { Msaa::Off };
        if m.is_some_and(|m| *m != want) {
            commands.entity(cam).insert(want);
        }
    }
}

/// "게임 종료" in the system menu.
fn quit(s: Res<Session>, mut exit: MessageWriter<AppExit>) {
    if s.ui.quit {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_closes_windows_before_opening_the_menu() {
        let mut ui = crate::state::UiFlags { map: true, missions: true, ..Default::default() };
        ui.escape();
        assert!(!ui.map && !ui.missions && !ui.menu, "first Esc closes what is open");
        ui.escape();
        assert!(ui.menu, "then the system menu");
        ui.escape();
        assert!(!ui.menu, "and Esc again closes it");
    }

    #[test]
    fn the_next_key_pressed_is_bound_and_used_at_once() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(SettingsPlugin).insert_resource(Session::default()).init_resource::<ButtonInput<KeyCode>>();
        let cs = ClientSettings::load(true);
        app.insert_resource(KeyBinds::from(&cs.s)).insert_resource(cs);
        assert_eq!(app.world().resource::<KeyBinds>().key(Action::Fire), KeyCode::Space);
        app.world_mut().resource_mut::<Listening>().0 = Some(Action::Fire);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
        app.update();
        assert_eq!(app.world().resource::<Listening>().0, None, "taken");
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::KeyF), "the key does not also act this frame");
        assert_eq!(app.world().resource::<ClientSettings>().s.keys.get("fire").map(String::as_str), Some("KeyF"));
        assert_eq!(app.world().resource::<KeyBinds>().key(Action::Fire), KeyCode::KeyF, "in effect at once");
        // Esc cancels.
        app.world_mut().resource_mut::<Listening>().0 = Some(Action::Map);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Escape);
        app.update();
        assert_eq!(app.world().resource::<KeyBinds>().key(Action::Map), KeyCode::KeyM);
    }

    #[test]
    fn every_default_key_is_bindable() {
        for a in Action::ALL {
            assert!(code(a.default_key()).is_some(), "{a:?} → {}", a.default_key());
        }
    }

    #[test]
    fn automated_runs_keep_the_window_and_the_defaults() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(SettingsPlugin).insert_resource(Session::default());
        let mut cs = ClientSettings::load(true);
        assert_eq!(cs.s, Settings::default());
        cs.s.display = Display::Fullscreen;
        cs.changed();
        app.insert_resource(cs);
        let w = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();
        app.update();
        assert_eq!(app.world().get::<Window>(w).unwrap().mode, WindowMode::Windowed, "a screenshot run never goes fullscreen");
        assert!(app.world().resource::<ClientSettings>().dirty, "nor writes the player's file");
        // A player's run follows the settings.
        app.world_mut().resource_mut::<ClientSettings>().automated = false;
        app.world_mut().resource_mut::<ClientSettings>().dirty = false;
        app.update();
        assert!(matches!(app.world().get::<Window>(w).unwrap().mode, WindowMode::Fullscreen(..)));
    }
}
