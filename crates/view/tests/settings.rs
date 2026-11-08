//! Player settings: the file, the levels the mixer gets, and that the windows draw.

use view::settings::{Display, Settings, settings_ui, system_menu};

#[test]
fn a_settings_file_round_trips_and_a_broken_one_never_stops_the_game() {
    let mut s = Settings { master: 0.8, music: 0.3, effects: 0.6, voice: 0.9, muted: false, display: Display::Borderless, vsync: false, show_fps: true };
    assert_eq!(Settings::parse(&s.to_text()), s);
    assert_eq!(Settings::parse("this is not ron"), Settings::default());
    assert_eq!(Settings::parse(""), Settings::default());
    // Missing fields take their defaults (files from older versions); levels are kept to 0–1.
    let old = Settings::parse("(master: 2.5, music: -1.0)");
    assert_eq!((old.master, old.music, old.effects, old.display), (1.0, 0.0, 1.0, Display::Windowed));
    // The mixer: master × level, nothing while muted.
    assert!((s.music_gain() - 0.24).abs() < 1e-6 && (s.effects_gain() - 0.48).abs() < 1e-6);
    s.muted = true;
    assert_eq!((s.music_gain(), s.effects_gain(), s.voice_gain()), (0.0, 0.0, 0.0));
    // Defaults keep the mix the game was tuned with (music 0.45 under full effects).
    let d = Settings::default();
    assert_eq!((d.music_gain(), d.effects_gain(), d.voice_gain(), d.vsync), (0.45, 1.0, 1.0, true));
}

#[test]
fn the_settings_window_and_the_menu_draw_without_input() {
    let ctx = egui::Context::default();
    let mut s = Settings::default();
    let mut changed = true;
    let mut picked = Some(view::settings::MenuChoice::Quit);
    for _ in 0..2 {
        ctx.begin_pass(egui::RawInput::default());
        egui::Area::new("settings".into()).show(&ctx, |ui| changed = settings_ui(ui, &mut s));
        picked = system_menu(&ctx);
        let _ = ctx.end_pass();
    }
    assert!(!changed, "nothing touched, nothing to save");
    assert_eq!(picked, None);
    assert_eq!(s, Settings::default());
}
