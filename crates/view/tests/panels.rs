//! Window panels checked headless: the artifact section shows what an unappraised find hides, and
//! clicking its buttons returns the right request.

use egui::epaint::Shape;
use egui::{Pos2, RawInput, Rect, Vec2};
use shared::artifacts::{Artifact, Culture, Holder};
use shared::chronicle::BioEntry;
use view::panels::{self, ArtifactPrices, ArtifactRequest};

fn artifact(id: u64, appraised: bool) -> Artifact {
    Artifact {
        id,
        name: "Hollow Crown의 잃어버린 왕관".into(),
        culture: Culture::Human,
        origin_system: 6,
        origin_site: "고대 유적 Alpha".into(),
        value: 3000,
        appraised,
        holder: Holder::Player(1),
        provenance: vec![BioEntry { date: "3827.001".into(), text: "발굴되었다.".into(), event: None }],
    }
}

const PRICES: ArtifactPrices = ArtifactPrices { appraisal: 400, unappraised_sale: 0.4, donation_share: 0.25, donation_reputation: 10 };

fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Draws the panel for a few frames; `click` (a button label prefix) is clicked on the last frame.
fn frame(held: &[Artifact], docked: bool, click: Option<&str>) -> (Vec<String>, Option<ArtifactRequest>) {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let (mut seen, mut got) = (vec![], None);
    let mut at: Option<Pos2> = None;
    for i in 0..4 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), time: Some(i as f64 * 0.1), ..Default::default() };
        // Press and release on the button found in the previous frame.
        if let Some(p) = at.take() {
            input.events = vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
            ];
        }
        ctx.begin_pass(input);
        let mut r = None;
        egui::Area::new("panel".into()).fixed_pos(Pos2::new(10.0, 10.0)).show(&ctx, |ui| r = panels::artifacts(ui, held, docked, &PRICES));
        let full = ctx.end_pass();
        if r.is_some() {
            got = r;
        }
        let mut t = vec![];
        for c in &full.shapes {
            texts(&c.shape, &mut t);
        }
        if i == 2
            && let Some(label) = click
        {
            at = t.iter().find(|(s, _)| s.starts_with(label)).map(|(_, r)| r.center());
        }
        seen = t.into_iter().map(|(s, _)| s).collect();
    }
    (seen, got)
}

#[test]
fn unappraised_find_hides_its_name_and_value() {
    let (t, _) = frame(&[artifact(7, false)], true, None);
    assert!(t.iter().any(|s| s == "미감정 유물 (고대 유적 Alpha)"), "{t:?}");
    assert!(!t.iter().any(|s| s.contains("왕관") || s.contains("3000")), "name and value stay hidden: {t:?}");
    assert!(t.iter().any(|s| s == "감정 (400 CR)"));
    assert!(t.iter().any(|s| s == "판매 (미감정 40%)"));
}

#[test]
fn appraised_artifact_shows_name_value_and_prices() {
    let (t, _) = frame(&[artifact(7, true)], true, None);
    assert!(t.iter().any(|s| s == "Hollow Crown의 잃어버린 왕관"));
    assert!(t.iter().any(|s| s.contains("인류 유물 · 감정가 3000 CR")), "{t:?}");
    assert!(!t.iter().any(|s| s.starts_with("감정 (")), "no appraisal button once appraised");
    assert!(t.iter().any(|s| s == "판매 (3000 CR)"));
    assert!(t.iter().any(|s| s == "박물관 기증 (평판 +10, 사례금 750 CR)"));
    assert!(t.iter().any(|s| s == "내력 (1)"));
}

#[test]
fn buttons_return_requests_only_when_docked() {
    assert_eq!(frame(&[artifact(7, true)], true, Some("판매")).1, Some(ArtifactRequest::Sell(7)));
    assert_eq!(frame(&[artifact(7, false)], true, Some("감정 (")).1, Some(ArtifactRequest::Appraise(7)));
    assert_eq!(frame(&[artifact(9, true)], true, Some("박물관 기증")).1, Some(ArtifactRequest::Donate(9)));
    assert_eq!(frame(&[artifact(7, true)], false, Some("판매")).1, None, "disabled away from the station");
    let (t, _) = frame(&[], true, None);
    assert!(t.iter().any(|s| s.starts_with("유물 없음")));
}
