//! The campaign opening (crates/client/assets/cinematics/opening.ron, docs/STORY.md §5) checked
//! without the engine: it parses, every anchor exists, captions fit and can be read, and the
//! frame at any time is a pure function of the file, the species and the clock.

use shared::galaxy::Species;
use view::cinematic::{CUT, Cinematic, check, frame_at};

fn opening() -> Cinematic {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/cinematics/opening.ron");
    Cinematic::parse(&std::fs::read_to_string(path).expect("opening.ron")).expect("valid RON")
}

#[test]
fn the_opening_is_valid_and_about_ninety_seconds() {
    let cin = opening();
    assert_eq!(check(&cin), Vec::<String>::new());
    assert!((85.0..=95.0).contains(&cin.duration()), "{} s", cin.duration());
}

#[test]
fn each_species_hears_its_own_history_first() {
    let cin = opening();
    let ids = |s| cin.sequence(s).iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
    assert_eq!(&ids(Species::Human)[2..5], ["record", "memory", "contract"]);
    assert_eq!(&ids(Species::Veyr)[2..5], ["memory", "record", "contract"]);
    assert_eq!(&ids(Species::Kharz)[2..5], ["contract", "record", "memory"]);
    // Ungrouped shots keep their place: the gate opens, the signal and title close, for everyone.
    for s in Species::ALL {
        let v = ids(s);
        assert_eq!((v[0], v[1], v[5], v[6], v[7]), ("gate_far", "gate_close", "war", "signal", "title"));
    }
}

#[test]
fn frames_are_a_function_of_the_clock() {
    let cin = opening();
    for t in [0.0, 3.3, 20.0, 47.9, 89.0] {
        assert_eq!(frame_at(&cin, Species::Veyr, t), frame_at(&cin, Species::Veyr, t));
    }
    // The camera moves within a shot and cuts between shots.
    let a = frame_at(&cin, Species::Human, 1.0);
    let b = frame_at(&cin, Species::Human, 7.0);
    assert_eq!((a.shot, b.shot), ("gate_far", "gate_far"));
    assert!(a.eye.distance(b.eye) > 1000.0, "the dolly moves");
    assert_eq!(a.look_at, b.look_at);
    let c = frame_at(&cin, Species::Human, 8.0 + 0.01);
    assert_eq!(c.shot, "gate_close");
}

#[test]
fn cuts_go_through_black_and_captions_fade() {
    let cin = opening();
    assert_eq!(frame_at(&cin, Species::Human, 0.0).black, 1.0, "starts black");
    assert_eq!(frame_at(&cin, Species::Human, 4.0).black, 0.0);
    assert!(frame_at(&cin, Species::Human, 8.0 - CUT * 0.25).black > 0.4, "into the cut");
    // gate_close starts at 8 s; its caption is at 2..8 s into the shot.
    let f = frame_at(&cin, Species::Human, 8.0 + 2.2);
    assert_eq!(f.captions.len(), 1);
    assert!(f.captions[0].2 > 0.0 && f.captions[0].2 < 1.0, "fading in");
    assert_eq!(frame_at(&cin, Species::Human, 8.0 + 5.0).captions[0].2, 1.0);
    assert!(frame_at(&cin, Species::Human, 8.0 + 9.0).captions.is_empty());
}

#[test]
fn the_end_shows_the_captains_ship_and_the_title() {
    let cin = opening();
    let end = frame_at(&cin, Species::Kharz, cin.duration() - 2.0);
    assert_eq!(end.shot, "title");
    assert!(end.title.is_some_and(|t| t.contains("오리온의 신호")), "names chapter 1");
    let ship = end.ship.expect("own ship in the last shot");
    assert_eq!(ship, end.look_at, "the camera looks at the ship");
    assert!(frame_at(&cin, Species::Kharz, cin.duration() + 5.0).done);
    assert!(!end.done);
}

#[test]
fn check_catches_broken_files() {
    let mut cin = opening();
    cin.shots[1].captions[0].secs = 30.0;
    cin.shots[2].system = 1; // Lumen Reach: same anchor, still fine
    cin.shots[3].anchor = view::cinematic::Anchor::Poi(shared::galaxy::PoiKind::AncientGate); // no gate in Hollow Crown
    cin.shots[6].captions[1].text = "가".repeat(40);
    let bad = check(&cin);
    assert!(bad.iter().any(|b| b.contains("runs into a cut")), "{bad:?}");
    assert!(bad.iter().any(|b| b.contains("no Poi(AncientGate) in system 6")), "{bad:?}");
    assert!(bad.iter().any(|b| b.contains("longer than")), "{bad:?}");
}

#[test]
fn every_line_fits_the_subtitle_band_in_the_korean_font() {
    use egui::{Color32, FontId, LayerId, Pos2, RawInput, Rect, Vec2};
    use view::cinematic::{SOURCE_PX, SUBTITLE_BAND_W, SUBTITLE_PX, TITLE_PX};
    let (fonts, found) = view::fonts::with_korean();
    let Some(font) = found else {
        // CI installs fonts-nanum; a machine without any Korean font cannot measure Korean text.
        eprintln!("no Korean font found, skipping the width check");
        return;
    };
    let ctx = egui::Context::default();
    ctx.set_fonts(fonts);
    let cin = opening();
    let mut widest = (0.0f32, String::new());
    for _ in 0..2 {
        ctx.begin_pass(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 720.0))), ..Default::default() });
        let painter = ctx.layer_painter(LayerId::background());
        for shot in &cin.shots {
            let lines = shot.captions.iter().flat_map(|c| c.text.lines().map(|l| (l, SUBTITLE_PX)).chain(c.source.as_deref().map(|s| (s, SOURCE_PX))));
            let title = shot.title.iter().flat_map(|t| t.lines().map(|l| (l, TITLE_PX)));
            for (line, px) in lines.chain(title) {
                let w = painter.layout_no_wrap(line.to_string(), FontId::proportional(px), Color32::WHITE).size().x;
                if w > widest.0 {
                    widest = (w, line.to_string());
                }
            }
        }
        let _ = ctx.end_pass();
    }
    assert!(widest.0 > 100.0, "the font measured something ({font})");
    assert!(widest.0 <= SUBTITLE_BAND_W, "\"{}\" is {:.0} px, the band is {SUBTITLE_BAND_W} px", widest.1, widest.0);
}
