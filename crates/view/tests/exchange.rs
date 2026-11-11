//! The exchange screen checked headless: what it shows, and which trade a click returns.

use egui::epaint::Shape;
use egui::{Pos2, RawInput, Rect, Vec2};
use shared::market::{self, Good, Role};
use std::collections::BTreeMap;
use view::exchange::{self, ExchangeState, ExchangeView, TradeRequest};

fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        Shape::Text(t) => out.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
        _ => {}
    }
}

/// Draws a panel for a few frames; `click` (a text prefix) is clicked on the last frame.
/// Returns the texts of the last frame and the request the panel returned, if any.
fn frame<R>(mut draw: impl FnMut(&mut egui::Ui) -> Option<R>, click: Option<&str>) -> (Vec<String>, Option<R>) {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    let (mut seen, mut got) = (vec![], None);
    let mut at: Option<Pos2> = None;
    for i in 0..4 {
        let mut input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))), time: Some(i as f64 * 0.1), ..Default::default() };
        // Press and release on the text found in the previous frame.
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

fn quotes(sys: u16) -> Vec<market::Quote> {
    market::quotes(&shared::tuning::get().trade.market, sys, |_| 0.0)
}

#[test]
fn the_exchange_shows_cards_where_the_price_sits_and_where_it_sells() {
    let q = quotes(0);
    let hold = BTreeMap::new();
    let view = ExchangeView { station: 0, quotes: &q, hold: &hold, hold_cap: 90, credits: 50_000, docked: true, range: 3.0 };
    let mut st = ExchangeState::default();
    let (seen, req) = frame(|ui| exchange::exchange(ui, &view, &mut st), None);
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s == "Sol Haven 교역소"), "{seen:?}");
    for g in Good::ALL {
        assert!(seen.iter().any(|s| s == g.name()), "a card for {g:?}");
    }
    assert!(seen.iter().any(|s| s.starts_with("화물칸 0 / 90")));
    assert!(seen.iter().any(|s| s.starts_with("은하 최저")), "the price gauge");
    // A good the station makes is picked first, with where it sells best.
    let made = q.iter().find(|x| x.role == Role::Produce && x.buy.is_some()).unwrap().good;
    assert_eq!(st.selected, Some(made));
    assert!(seen.iter().any(|s| s.starts_with("어디에 팔까")));
    assert!(seen.iter().any(|s| s.starts_with('+')), "a margin on a route: {seen:?}");
    assert!(seen.iter().any(|s| s.contains("생산지")));
}

#[test]
fn picking_a_card_then_buying_returns_the_trade() {
    let q = quotes(0);
    let mut hold = BTreeMap::new();
    let wanted = q.iter().find(|x| x.role == Role::Demand && x.sell.is_some()).unwrap().good;
    hold.insert(wanted.key().to_string(), 7);
    let view = ExchangeView { station: 0, quotes: &q, hold: &hold, hold_cap: 90, credits: 50_000, docked: true, range: 3.0 };
    // Click the card of the good held: it is picked.
    let mut st = ExchangeState::default();
    let (_, _) = frame(|ui| exchange::exchange(ui, &view, &mut st), Some(wanted.name()));
    assert_eq!(st.selected, Some(wanted));
    // The station does not sell it: where to get it, and a sell button for what is held.
    let (seen, req) = frame(|ui| exchange::exchange(ui, &view, &mut st), Some("판매 7개"));
    assert!(seen.iter().any(|s| s.starts_with("어디서 살까")), "{seen:?}");
    assert_eq!(req, Some(TradeRequest::Sell(wanted, 7)));
    // A made good: the buy button buys the quantity set.
    let made = q.iter().find(|x| x.buy.is_some()).unwrap().good;
    let mut st = ExchangeState { selected: Some(made), qty: 25 };
    let (_, req) = frame(|ui| exchange::exchange(ui, &view, &mut st), Some("구매 25개"));
    assert_eq!(req, Some(TradeRequest::Buy(made, 25)));
}

#[test]
fn a_full_hold_or_an_empty_wallet_cannot_buy_and_undocked_nothing_trades() {
    let q = quotes(0);
    let made = q.iter().find(|x| x.buy.is_some()).unwrap().good;
    let mut hold = BTreeMap::new();
    hold.insert("Ferrite".to_string(), 90);
    let full = ExchangeView { station: 0, quotes: &q, hold: &hold, hold_cap: 90, credits: 50_000, docked: true, range: 3.0 };
    let mut st = ExchangeState { selected: Some(made), qty: 10 };
    let (seen, req) = frame(|ui| exchange::exchange(ui, &full, &mut st), Some("구매"));
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s.starts_with("구매 0개")));
    let empty = BTreeMap::new();
    let away = ExchangeView { station: 0, quotes: &q, hold: &empty, hold_cap: 90, credits: 50_000, docked: false, range: 3.0 };
    let mut st = ExchangeState { selected: Some(made), qty: 10 };
    let (seen, req) = frame(|ui| exchange::exchange(ui, &away, &mut st), Some("구매"));
    assert_eq!(req, None);
    assert!(seen.iter().any(|s| s.contains("도킹해야")));
}
