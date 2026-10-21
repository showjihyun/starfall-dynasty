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

type Texts = Vec<(String, egui::Rect)>;
type Rects = Vec<(egui::Rect, egui::Color32)>;

/// Text and filled rectangles drawn for the opening at time `t`, on a 1600x900 screen.
fn drawn_at(t: f32) -> (Texts, Rects) {
    use egui::{Pos2, RawInput, Rect, Shape, Vec2};
    let cin = opening();
    let ctx = egui::Context::default();
    ctx.set_fonts(view::fonts::with_korean().0);
    let (mut texts, mut rects) = (vec![], vec![]);
    for _ in 0..2 {
        ctx.begin_pass(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() });
        view::cinematic::draw(&ctx, &frame_at(&cin, Species::Human, t), true);
        let out = ctx.end_pass();
        (texts, rects) = (vec![], vec![]);
        for c in out.shapes {
            match c.shape {
                Shape::Text(tx) => texts.push((tx.galley.text().to_string(), tx.visual_bounding_rect())),
                Shape::Rect(r) => rects.push((r.rect, r.fill)),
                _ => {}
            }
        }
    }
    (texts, rects)
}

#[test]
fn subtitles_sit_centred_above_the_letterbox_with_their_source() {
    // record shot (Human order) starts at 20 s; its first caption is up 1..6.5 s into it.
    let (texts, rects) = drawn_at(20.0 + 3.0);
    let line = texts.iter().rev().find(|(t, _)| t.starts_with("게이트 연쇄 고장")).expect("subtitle drawn");
    let source = texts.iter().rev().find(|(t, _)| t.contains("인류 제국 기록보관소")).expect("source drawn");
    let bar_top = 900.0 * (1.0 - view::cinematic::LETTERBOX);
    assert!((line.1.center().x - 800.0).abs() < 2.0, "centred: {:?}", line.1);
    assert!(line.1.max.y < source.1.min.y && source.1.max.y < bar_top, "line, then source, above the bar");
    assert!(line.1.min.y > 900.0 * 0.6, "in the lower part of the picture");
    assert!(texts.iter().any(|(t, _)| t.contains("건너뛰기")), "skip hint");
    let bars = rects.iter().filter(|(r, _)| r.width() >= 1600.0 && (r.height() - 81.0).abs() < 1.0).count();
    assert_eq!(bars, 2, "letterbox top and bottom");
    assert!(!rects.iter().any(|(r, _)| r.height() >= 900.0), "no fade mid-shot");
}

#[test]
fn a_cut_draws_black_over_everything_and_the_title_names_the_chapter() {
    let (_, rects) = drawn_at(0.0);
    let fade = rects.iter().find(|(r, _)| r.height() >= 900.0).expect("full-screen fade at the start");
    assert_eq!(fade.1.a(), 255);
    let (texts, _) = drawn_at(opening().duration() - 2.5);
    let title = texts.iter().find(|(t, _)| t == "STARFALL DYNASTY").expect("title");
    assert!(title.1.max.y < 900.0 * 0.4, "title in the upper part, clear of the ship in the middle: {:?}", title.1);
    assert!(texts.iter().any(|(t, _)| t.contains("1장 · 오리온의 신호")));
}

#[test]
fn the_camera_distance_follows_the_ship_size() {
    use view::cinematic::{SHIP_SHOT_RADIUS, eye_for_ship};
    let cin = opening();
    let f = frame_at(&cin, Species::Human, cin.duration() - 3.0);
    let ratio = |r: f32| eye_for_ship(&f, r).distance(f.look_at) / f.eye.distance(f.look_at);
    // A Talon (11 m) is filmed from closer than written, a capital (225 m) from further.
    assert!((ratio(11.0) - 11.0 / SHIP_SHOT_RADIUS).abs() < 1e-3);
    assert!((ratio(225.0) - 225.0 / SHIP_SHOT_RADIUS).abs() < 1e-3);
    assert_eq!(eye_for_ship(&f, 11.0).normalize_or_zero(), eye_for_ship(&f, 11.0).normalize_or_zero(), "same direction");
    let gate = frame_at(&cin, Species::Human, 3.0);
    assert_eq!(eye_for_ship(&gate, 500.0), gate.eye, "shots without the ship are not scaled");
}

#[test]
fn each_line_of_a_two_line_subtitle_is_centred() {
    // war shot (Human order: 60..72 s), caption 2..10 s in: "그 날부터, 우리는\n서로의 역사를 쏘고 있다."
    let (texts, _) = drawn_at(60.0 + 5.0);
    let a = texts.iter().rev().find(|(t, _)| t == "그 날부터, 우리는").expect("first line");
    let b = texts.iter().rev().find(|(t, _)| t == "서로의 역사를 쏘고 있다.").expect("second line");
    assert!((a.1.center().x - 800.0).abs() < 2.0 && (b.1.center().x - 800.0).abs() < 2.0, "{:?} {:?}", a.1, b.1);
    assert!(a.1.max.y <= b.1.min.y, "first line above the second");
}
