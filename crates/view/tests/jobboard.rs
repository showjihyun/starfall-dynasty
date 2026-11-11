//! The job board screen checked headless: what it shows, and which click it returns.

use egui::epaint::Shape;
use egui::{Pos2, RawInput, Rect, Vec2};
use shared::jobs::{self, ActiveJob, Job, JobKind};
use view::jobboard::{self, BoardState, BoardView, JobRequest};

fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Draws the screen for a few frames; `click` (a text prefix) is clicked on the last frame.
fn frame<R>(mut draw: impl FnMut(&mut egui::Ui) -> Option<R>, click: Option<&str>) -> (Vec<String>, Option<R>) {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let (mut seen, mut got) = (vec![], None);
    let mut at: Option<Pos2> = None;
    for i in 0..4 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 900.0))), time: Some(i as f64 * 0.1), ..Default::default() };
        if let Some(p) = at.take() {
            input.events = vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
            ];
        }
        ctx.begin_pass(input);
        let mut r = None;
        egui::Area::new("panel".into()).fixed_pos(Pos2::new(10.0, 10.0)).show(&ctx, |ui| r = draw(ui));
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

const UNIX: u64 = 1_800_000_000;

fn board(station: u16) -> (Vec<Job>, u64) {
    let t = shared::tuning::get();
    let p = jobs::period(&t, UNIX);
    (jobs::board(&t, station, p), (p + 1) * t.trade.jobs.period_secs)
}

fn view<'a>(station: Option<u16>, jobs: &'a [Job], next: u64, held: &'a [ActiveJob]) -> BoardView<'a> {
    BoardView { station, jobs, next_board: next, held, unix: UNIX, max_active: 4, room: 90, here: station.unwrap_or(3) }
}

#[test]
fn the_board_shows_cards_and_a_briefing_and_takes_the_picked_job() {
    let (jobs, next) = board(3);
    assert!(jobs.len() >= 2);
    let mut st = BoardState::default();
    let v = view(Some(3), &jobs, next, &[]);
    let (seen, req) = frame(|ui| jobboard::job_board(ui, &v, &mut st), None);
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s == "Orion Drift 의뢰 게시판"), "{seen:?}");
    assert!(seen.iter().any(|s| s.ends_with("뒤 새 의뢰")));
    for j in &jobs {
        assert!(seen.iter().any(|s| s == &j.describe()), "a card for {:?}", j.kind.tag());
    }
    // The first job is briefed: its chart, its terms, its button.
    assert_eq!(st.selected, Some(jobs[0].id));
    assert!(seen.iter().any(|s| s.starts_with("의뢰처 ")), "the chart: {seen:?}");
    assert!(seen.iter().any(|s| s == "현재 위치"));
    assert!(seen.iter().any(|s| s == "기한"));
    // Pick the second card by its errand, then take it.
    let (_, _) = frame(|ui| jobboard::job_board(ui, &v, &mut st), Some(&jobs[1].describe()));
    assert_eq!(st.selected, Some(jobs[1].id));
    let (_, req) = frame(|ui| jobboard::job_board(ui, &v, &mut st), Some("의뢰 수락"));
    assert_eq!(req, Some(JobRequest::Accept(jobs[1].id)));
}

#[test]
fn a_held_job_shows_its_time_and_can_be_given_up_even_away_from_a_station() {
    let (jobs, next) = board(3);
    let held = vec![ActiveJob { job: jobs[0].clone(), accepted: UNIX - 600, visited: vec![] }];
    let mut st = BoardState::default();
    let v = view(Some(3), &jobs, next, &held);
    let (seen, _) = frame(|ui| jobboard::job_board(ui, &v, &mut st), None);
    assert!(seen.iter().any(|s| s == "맡은 의뢰 1/4"), "{seen:?}");
    assert!(seen.iter().any(|s| s == "수락함"));
    assert!(seen.iter().any(|s| s.starts_with("남은 시간 ")));
    // The first one not yet taken is picked.
    assert_eq!(st.selected, Some(jobs[1].id));
    // Away from a station: the board is closed, the held job is briefed and can be given up.
    let away = view(None, &[], 0, &held);
    let mut st = BoardState::default();
    let (seen, req) = frame(|ui| jobboard::job_board(ui, &away, &mut st), Some("의뢰 포기"));
    assert!(seen.iter().any(|s| s.contains("도킹하면 열립니다")), "{seen:?}");
    assert_eq!(req, Some(JobRequest::Abandon(jobs[0].id)));
}

#[test]
fn a_supply_run_charts_where_the_good_is_made() {
    let (jobs, next, supply) = (0..60u16)
        .find_map(|s| {
            let (b, n) = board(s);
            let j = b.iter().find(|j| matches!(j.kind, JobKind::Supply { .. }))?.clone();
            Some((b, n, j))
        })
        .expect("a station with a supply run");
    let v = view(Some(supply.station), &jobs, next, &[]);
    let mut st = BoardState { selected: Some(supply.id) };
    let (seen, _) = frame(|ui| jobboard::job_board(ui, &v, &mut st), None);
    assert!(seen.iter().filter(|s| s.ends_with(" · 생산")).count() >= 1, "{seen:?}");
    assert!(seen.iter().any(|s| s.contains("교역소(F3)")));
}

#[test]
fn a_full_slate_or_a_hold_too_small_cannot_take_a_job() {
    // A haul from somewhere, with a hold too small for it.
    let (jobs, next, haul) = (0..60u16)
        .find_map(|s| {
            let (b, n) = board(s);
            let h = b.iter().find(|j| matches!(j.kind, JobKind::Haul { .. }))?.clone();
            Some((b, n, h))
        })
        .expect("a station with a haul");
    let JobKind::Haul { units, .. } = haul.kind else { unreachable!() };
    let mut v = view(Some(haul.station), &jobs, next, &[]);
    v.room = units - 1;
    let mut st = BoardState { selected: Some(haul.id) };
    let (seen, req) = frame(|ui| jobboard::job_board(ui, &v, &mut st), Some("의뢰 수락"));
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s == "화물칸이 부족합니다"), "{seen:?}");
    // Four held: nothing more.
    let held: Vec<ActiveJob> = (0..4).map(|k| ActiveJob { job: Job { id: 9_000 + k, ..jobs[0].clone() }, accepted: UNIX, visited: vec![] }).collect();
    let v = view(Some(haul.station), &jobs, next, &held);
    let mut st = BoardState { selected: Some(haul.id) };
    let (seen, req) = frame(|ui| jobboard::job_board(ui, &v, &mut st), Some("의뢰 수락"));
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s == "맡을 수 있는 의뢰가 가득 찼습니다"));
}
