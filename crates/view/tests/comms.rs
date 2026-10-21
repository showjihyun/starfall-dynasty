//! The chapter-end comms calls (crates/client/assets/cinematics/comms.ron, docs/STORY.md §4) checked
//! without the engine: the file is complete, each species hears its own voice, a call moves on with
//! the clock and with clicks, and the panel fits where the HUD leaves room.

use egui::{Event, PointerButton, Pos2, RawInput, Rect, Shape, Vec2};
use shared::galaxy::Species;
use view::comms::{Comms, MAX_ROWS, PANEL_POS, PANEL_W, Panel, check, hold, typed, typing};

fn comms() -> Comms {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets/cinematics/comms.ron");
    Comms::parse(&std::fs::read_to_string(path).expect("comms.ron")).expect("valid RON")
}

#[test]
fn every_chapter_ends_with_a_call_and_the_file_is_valid() {
    let c = comms();
    assert_eq!(check(&c), Vec::<String>::new());
    let after: Vec<u32> = c.chapters.iter().map(|ch| ch.after).collect();
    let scenarios: Vec<u32> = shared::missions::scenarios().iter().map(|s| s.id).collect();
    assert_eq!(after, scenarios, "one call after each campaign chapter");
}

#[test]
fn the_chapter_4_call_has_each_species_react_in_its_own_voice() {
    let c = comms();
    let voice = |s| c.lines(3, s).iter().filter(|l| l.only.is_some()).map(|l| c.speaker(&l.who).unwrap().voice).collect::<Vec<_>>();
    use view::comms::Voice;
    assert_eq!(voice(Species::Human), [Voice::Human]);
    assert_eq!(voice(Species::Veyr), [Voice::Veyr]);
    assert_eq!(voice(Species::Kharz), [Voice::Kharz]);
    // Everything else is the same call for everyone.
    assert_eq!(c.lines(3, Species::Human).len(), c.lines(3, Species::Kharz).len());
    assert!(c.lines(99, Species::Human).is_empty());
}

#[test]
fn check_catches_broken_files() {
    let mut c = comms();
    c.chapters[0].lines[0].who = "nobody".into();
    c.chapters[1].lines[0].text = "가".repeat(80);
    for l in &mut c.chapters[2].lines {
        l.only = Some(Species::Veyr);
    }
    let bad = check(&c);
    assert!(bad.iter().any(|b| b.contains("unknown speaker nobody")), "{bad:?}");
    assert!(bad.iter().any(|b| b.contains("longer than")), "{bad:?}");
    assert!(bad.iter().any(|b| b.contains("no lines for a Human captain")), "{bad:?}");
}

#[test]
fn a_line_types_out_then_holds_then_the_next_comes_on_its_own() {
    let text = "원본이다. 서명은 셋. 인류, Veyr, Kharz.";
    assert_eq!(typed(text, 0.0), "");
    assert_eq!(typed(text, 0.2).chars().count(), 5);
    assert_eq!(typed(text, typing(text) + 0.1), text);
    let lines = [("첫 줄입니다.", hold("첫 줄입니다.", None)), ("둘째 줄.", hold("둘째 줄.", None))];
    let mut p = Panel::new(3, 100.0);
    assert!(p.step(&lines, 100.0 + lines[0].1 as f64 - 0.05, false));
    assert_eq!(p.line, 0, "still holding");
    assert!(p.step(&lines, 100.0 + lines[0].1 as f64 + 0.01, false));
    assert_eq!(p.line, 1, "moved on by itself");
    let t1 = p.since;
    assert!(!p.step(&lines, t1 + lines[1].1 as f64 + 0.01, false), "the call closes after the last line");
}

#[test]
fn a_spoken_line_holds_until_its_voice_is_done() {
    let text = "문은 열렸다.";
    assert_eq!(hold(text, None), hold(text, Some(0.5)), "a short voice line: reading time decides");
    assert!((hold(text, Some(6.0)) - (6.0 + view::comms::VOICE_TAIL)).abs() < 1e-6, "a long one: the voice decides");
}

#[test]
fn a_click_finishes_the_typing_then_goes_to_the_next_line() {
    let lines = [("세 개의 서명. 세 개의 기억. 우리는 기다렸다.", 5.0), ("문은 열렸다.", 3.0)];
    let mut p = Panel::new(4, 10.0);
    assert!(p.step(&lines, 10.1, true));
    assert_eq!(p.line, 0, "first click shows the whole line");
    assert_eq!(typed(lines[0].0, (10.1 - p.since) as f32), lines[0].0);
    assert!(p.step(&lines, 10.2, true));
    assert_eq!(p.line, 1, "second click goes on");
    assert!(p.step(&lines, 10.25, false), "and the new line starts typing");
    assert_eq!(typed(lines[1].0, (10.25 - p.since) as f32).chars().count(), 1);
}

/// Text shapes and the panel's clicked flag for one call frame on a 1600x900 screen.
fn drawn(c: &Comms, after: u32, line: usize, click: Option<Pos2>) -> (Vec<(String, Rect)>, bool) {
    let ctx = egui::Context::default();
    ctx.set_fonts(view::fonts::with_korean().0);
    let lines = c.lines(after, Species::Human);
    let l = lines[line];
    let who = c.speaker(&l.who).unwrap();
    let mut out = (vec![], false);
    for pass in 0..3 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))), ..Default::default() };
        if let (2, Some(at)) = (pass, click) {
            input.events = vec![
                Event::PointerMoved(at),
                Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
                Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
            ];
        }
        ctx.begin_pass(input);
        let clicked = view::comms::draw(&ctx, who, &l.text, true, (line, lines.len()), 1.0);
        let shapes = ctx.end_pass().shapes;
        let mut texts = vec![];
        for s in shapes {
            if let Shape::Text(t) = s.shape {
                texts.push((t.galley.text().to_string(), t.visual_bounding_rect()));
            }
        }
        out = (texts, out.1 || clicked);
    }
    out
}

#[test]
fn the_panel_sits_left_under_the_location_panel_and_wraps_every_line_in_three_rows() {
    let (fonts, found) = view::fonts::with_korean();
    drop(fonts);
    let c = comms();
    let mut most = 0;
    for ch in &c.chapters {
        for (i, l) in c.lines(ch.after, Species::Human).iter().enumerate() {
            let (texts, _) = drawn(&c, ch.after, i, None);
            let name = &c.speaker(&l.who).unwrap().name;
            let n = texts.iter().find(|(t, _)| t == name).unwrap_or_else(|| panic!("speaker name drawn: {name}"));
            let body = texts.iter().find(|(t, _)| t == &l.text).expect("line drawn");
            assert!(n.1.min.x >= PANEL_POS[0] && n.1.min.y >= PANEL_POS[1], "{:?}", n.1);
            assert!(body.1.max.x <= PANEL_POS[0] + PANEL_W, "\"{}\" runs out of the panel: {:?}", l.text, body.1);
            if found.is_some() {
                let rows = (body.1.height() / (view::comms::TEXT_PX * 1.1)).round() as usize;
                assert!(rows <= MAX_ROWS, "\"{}\" wraps to {rows} rows", l.text);
                most = most.max(rows);
            }
            // Clear of the ship panel at the bottom centre and the tracker at the top right.
            assert!(body.1.max.y < 900.0 * 0.6, "{:?}", body.1);
        }
    }
    // The measurement is real: the longest lines do wrap.
    assert!(found.is_none() || most >= 2, "no line wrapped ({most} rows max)");
}

#[test]
fn clicking_the_panel_is_reported_and_clicking_elsewhere_is_not() {
    let c = comms();
    let inside = Pos2::new(PANEL_POS[0] + 200.0, PANEL_POS[1] + 40.0);
    assert!(drawn(&c, 0, 0, Some(inside)).1);
    assert!(!drawn(&c, 0, 0, Some(Pos2::new(1200.0, 600.0))).1);
}
