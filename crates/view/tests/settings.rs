//! Player settings: the file, the levels the mixer gets, and that the windows draw.

use view::settings::{Display, Settings, settings_ui, system_menu};

#[test]
fn a_settings_file_round_trips_and_a_broken_one_never_stops_the_game() {
    let mut s = Settings { master: 0.8, music: 0.3, effects: 0.6, voice: 0.9, muted: false, display: Display::Borderless, vsync: false, quality: view::settings::Quality::Low, ui_scale: 1.25, show_fps: true, keys: [("fire".to_string(), "KeyF".to_string())].into_iter().collect() };
    assert_eq!(Settings::parse(&s.to_text()), s);
    assert_eq!(Settings::parse("this is not ron"), Settings::default());
    assert_eq!(Settings::parse(""), Settings::default());
    // Missing fields take their defaults (files from older versions); levels are kept to 0–1.
    let old = Settings::parse("(master: 2.5, music: -1.0, ui_scale: 9.0)");
    assert_eq!(old.ui_scale, view::settings::UI_SCALE.1, "the UI scale is kept in range");
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
        egui::Area::new("settings".into()).show(&ctx, |ui| changed = settings_ui(ui, &mut s, &mut None));
        picked = system_menu(&ctx);
        let _ = ctx.end_pass();
    }
    assert!(!changed, "nothing touched, nothing to save");
    assert_eq!(picked, None);
    assert_eq!(s, Settings::default());
}

#[test]
fn keys_can_be_rebound_and_clashes_are_found() {
    use view::keymap::{self, Action};
    let mut s = Settings::default();
    assert_eq!(keymap::shown(&s.keys, Action::Fire), "Space");
    assert_eq!(keymap::shown(&s.keys, Action::Boost), "Shift");
    assert!(keymap::conflicts(&s.keys).is_empty(), "the defaults never clash");
    assert!(keymap::bind(&mut s.keys, Action::Fire, "KeyF"));
    assert_eq!(keymap::shown(&s.keys, Action::Fire), "F");
    assert_eq!(keymap::conflicts(&s.keys), vec![(Action::NoseDown, Action::Fire)], "F was nose-down");
    assert!(!keymap::bind(&mut s.keys, Action::Map, "Escape"), "Esc is the menu");
    // Back to the default: no override is stored; a file with an unknown action or a reserved key loads clean.
    assert!(keymap::bind(&mut s.keys, Action::Fire, "Space"));
    assert!(s.keys.is_empty());
    let odd = Settings::parse("(keys: {\"warp_drive\": \"KeyW\", \"map\": \"Escape\", \"scan\": \"KeyQ\"})");
    assert_eq!(odd.keys.len(), 1);
    assert_eq!(keymap::shown(&odd.keys, Action::Scan), "Q");
}

#[test]
fn quality_presets_turn_off_the_costly_things_first() {
    use view::settings::Quality;
    assert_eq!(Settings::default().quality, Quality::High, "the art reference look");
    assert_eq!(Quality::High.features(), (true, true, true));
    assert_eq!(Quality::Medium.features(), (false, true, true), "shadows go first");
    assert_eq!(Quality::Low.features(), (false, false, false));
    assert_eq!(Quality::parse("LOW"), Some(Quality::Low));
    assert_eq!(Settings::parse("(quality: Medium)").quality, Quality::Medium);
}
