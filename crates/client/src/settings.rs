//! Player settings (`view::settings`): loaded from the per-user config dir at start, applied to the
//! window when they change, saved when the player changes them. Automated runs (screenshots,
//! showroom, replays, benches) use the defaults and never touch the file or the window mode, so the
//! same command keeps giving the same image.

use crate::state::Session;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, VideoModeSelection, WindowMode};
use view::settings::{Display, Settings};

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
        app.add_systems(PostUpdate, (apply, quit));
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
