//! The exchange: a station's trade goods market as a screen of its own (F3, or the station
//! window's button). Goods are cards with their own icon, where they are made and needed at a
//! glance; the picked good shows where this price sits in the galaxy, where it sells best nearby,
//! and a quantity to buy or sell with the total before the click. Drawn from plain data and checked
//! headless (`tests/exchange.rs`); the client sends what it returns.

use crate::palette::{DIM, GOLD, exchange as px, tier_color};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2};
use shared::galaxy::{self, SystemId};
use shared::market::{self, Good, Quote, Role};
use std::collections::BTreeMap;

/// What the exchange shows.
pub struct ExchangeView<'a> {
    pub station: SystemId,
    pub quotes: &'a [Quote],
    /// The ship's hold (ores, goods, sealed cargo).
    pub hold: &'a BTreeMap<String, u32>,
    pub hold_cap: u32,
    pub credits: u64,
    pub docked: bool,
    /// How far (ly) to look for where a good sells or is made.
    pub range: f32,
}

/// What the player has picked on the exchange (kept between frames).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExchangeState {
    pub selected: Option<Good>,
    pub qty: u32,
}

/// A trade the player asked for: buy or sell this many units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TradeRequest {
    Buy(Good, u32),
    Sell(Good, u32),
}

/// A good's icon: a coloured well with its glyph, `r` across.
pub fn icon(p: &egui::Painter, c: Pos2, r: f32, g: Good) {
    let col = px::good(g);
    p.circle_filled(c, r, px::WELL);
    p.circle_stroke(c, r, Stroke::new(1.5, col.gamma_multiply(0.7)));
    let s = r * 0.5;
    let w = Stroke::new((r * 0.12).max(1.2), col);
    let at = |x: f32, y: f32| c + Vec2::new(x * s, y * s);
    match g {
        // A ration tin: a squat drum with its bands.
        Good::Rations => {
            p.rect_stroke(Rect::from_min_max(at(-0.8, -0.9), at(0.8, 0.9)), r * 0.15, w, StrokeKind::Middle);
            p.line_segment([at(-0.8, -0.3), at(0.8, -0.3)], w);
            p.line_segment([at(-0.8, 0.3), at(0.8, 0.3)], w);
        }
        // A medical cross.
        Good::Medicine => {
            p.rect_filled(Rect::from_center_size(c, Vec2::new(s * 0.55, s * 1.8)), 1.0, col);
            p.rect_filled(Rect::from_center_size(c, Vec2::new(s * 1.8, s * 0.55)), 1.0, col);
        }
        // A gear: a ring and its teeth.
        Good::Machinery => {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * s * 0.7, c + d * s * 1.05], Stroke::new(w.width * 1.6, col));
            }
            p.circle_stroke(c, s * 0.72, w);
            p.circle_filled(c, s * 0.25, col);
        }
        // A cell with its terminal and a bolt.
        Good::FuelCells => {
            p.rect_stroke(Rect::from_min_max(at(-0.55, -0.75), at(0.55, 1.0)), r * 0.08, w, StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(at(-0.25, -1.0), at(0.25, -0.75)), 0.0, col);
            p.add(Shape::line(vec![at(0.15, -0.45), at(-0.2, 0.15), at(0.2, 0.15), at(-0.15, 0.75)], w));
        }
        // A chip with its pins.
        Good::Electronics => {
            p.rect_stroke(Rect::from_min_max(at(-0.6, -0.6), at(0.6, 0.6)), 1.0, w, StrokeKind::Middle);
            for k in [-0.3, 0.0, 0.3] {
                p.line_segment([at(k, -0.6), at(k, -1.0)], w);
                p.line_segment([at(k, 0.6), at(k, 1.0)], w);
                p.line_segment([at(-0.6, k), at(-1.0, k)], w);
                p.line_segment([at(0.6, k), at(1.0, k)], w);
            }
        }
        // A cut gem.
        Good::Luxuries => {
            let pts = vec![at(-0.9, -0.25), at(-0.45, -0.85), at(0.45, -0.85), at(0.9, -0.25), at(0.0, 0.95)];
            p.add(Shape::convex_polygon(pts, col.gamma_multiply(0.35), w));
            p.line_segment([at(-0.9, -0.25), at(0.9, -0.25)], w);
        }
        // A shell.
        Good::Arms => {
            p.rect_stroke(Rect::from_min_max(at(-0.4, -0.2), at(0.4, 1.0)), 0.0, w, StrokeKind::Middle);
            p.add(Shape::convex_polygon(vec![at(-0.4, -0.2), at(0.0, -1.0), at(0.4, -0.2)], col, Stroke::NONE));
            p.line_segment([at(-0.4, 0.6), at(0.4, 0.6)], w);
        }
        // A capsule, half red.
        Good::Stims => {
            let a = at(-0.65, 0.65);
            let b = at(0.65, -0.65);
            p.line_segment([a, b], Stroke::new(s * 0.75, col.gamma_multiply(0.45)));
            p.line_segment([c, b], Stroke::new(s * 0.75, col));
        }
    }
}

fn role_chip(role: Role, illegal: bool) -> Option<(&'static str, Color32)> {
    if illegal {
        return Some(("밀수품", px::ILLEGAL));
    }
    match role {
        Role::Produce => Some(("생산지", px::MADE)),
        Role::Demand => Some(("수요지", px::NEEDED)),
        Role::Neutral => None,
    }
}

/// The galaxy's cheapest and dearest station price for `g` (at their own levels).
fn galaxy_span(g: Good) -> (f32, f32) {
    let t = shared::tuning::get();
    let prices: Vec<f32> = galaxy::systems().iter().filter_map(|s| market::sell_price(&t.trade.market, s.id, g, 0.0)).collect();
    (prices.iter().copied().fold(f32::MAX, f32::min), prices.iter().copied().fold(0.0, f32::max))
}

fn distance(a: SystemId, b: SystemId) -> f32 {
    shared::glam::Vec3::from(galaxy::system(a).map_pos).distance(shared::glam::Vec3::from(galaxy::system(b).map_pos))
}

/// An estimate of what `qty` units come to, the price moving as they trade (`market::buy_total`).
fn estimate(unit: f32, qty: u32, buying: bool) -> f32 {
    let depth = shared::tuning::get().trade.market.depth.max(1.0);
    let drift = (qty as f32 / (2.0 * depth)).exp();
    unit * qty as f32 * if buying { drift } else { 1.0 / drift }
}

/// The exchange screen. Returns the trade the player asked for this frame.
pub fn exchange(ui: &mut egui::Ui, v: &ExchangeView, st: &mut ExchangeState) -> Option<TradeRequest> {
    let t = shared::tuning::get();
    let mut req = None;
    let def = galaxy::system(v.station);
    let used: u32 = v.hold.values().sum();
    let room = v.hold_cap.saturating_sub(used);
    if st.selected.is_none() {
        st.selected = v.quotes.iter().find(|q| q.buy.is_some()).or(v.quotes.first()).map(|q| q.good);
    }

    // ---- Header: where, the wallet, the hold.
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} 교역소", def.name)).size(20.0).strong().color(px::TEXT));
        ui.label(RichText::new(def.tier.label()).color(tier_color(def.tier)));
        ui.add_space(16.0);
        ui.label(RichText::new(format!("{} CR", v.credits)).size(18.0).color(GOLD));
    });
    hold_bar(ui, v.hold, v.hold_cap);
    if !v.docked {
        ui.label(RichText::new("정거장에 도킹해야 거래할 수 있습니다.").color(px::ILLEGAL));
    }
    ui.add_space(6.0);

    ui.horizontal_top(|ui| {
        // ---- Left: the goods, one card each.
        ui.vertical(|ui| {
            ui.set_width(330.0);
            for q in v.quotes {
                let held = v.hold.get(q.good.key()).copied().unwrap_or(0);
                if card(ui, q, held, st.selected == Some(q.good), market::contraband(&t.trade.market, q.good)) {
                    st.selected = Some(q.good);
                }
            }
        });
        ui.add_space(10.0);
        // ---- Right: the picked good.
        ui.vertical(|ui| {
            ui.set_width(440.0);
            let Some(q) = st.selected.and_then(|g| v.quotes.iter().find(|q| q.good == g)) else { return };
            let g = q.good;
            let held = v.hold.get(g.key()).copied().unwrap_or(0);
            let illegal = market::contraband(&t.trade.market, g);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(440.0, 64.0), Sense::hover());
            let p = ui.painter();
            icon(p, rect.left_center() + Vec2::new(30.0, 0.0), 26.0, g);
            p.text(rect.left_top() + Vec2::new(68.0, 6.0), Align2::LEFT_TOP, g.name(), FontId::proportional(22.0), px::TEXT);
            if let Some((chip, col)) = role_chip(q.role, illegal) {
                let at = rect.left_top() + Vec2::new(68.0, 38.0);
                let r = p.text(at + Vec2::new(6.0, 2.0), Align2::LEFT_TOP, chip, FontId::proportional(12.0), col);
                p.rect_stroke(r.expand2(Vec2::new(6.0, 2.0)), 6.0, Stroke::new(1.0, col), StrokeKind::Outside);
            }
            ui.label(RichText::new(g.blurb()).size(12.0).color(DIM));
            ui.add_space(4.0);

            // Where this price sits in the galaxy.
            let (lo, hi) = galaxy_span(g);
            ui.label(RichText::new("은하 시세 위치").size(12.0).color(DIM));
            price_gauge(ui, lo, hi, q.buy, q.sell);
            ui.add_space(6.0);

            // Where it sells best, or where to get it.
            let near: Vec<&galaxy::SystemDef> = galaxy::systems().iter().filter(|s| s.id != v.station && distance(v.station, s.id) <= v.range).collect();
            if let Some(cost) = q.buy {
                let mut sells: Vec<(SystemId, f32)> = near.iter().filter_map(|s| market::sell_price(&t.trade.market, s.id, g, 0.0).map(|p| (s.id, p))).collect();
                sells.sort_by(|a, b| b.1.total_cmp(&a.1));
                ui.label(RichText::new(format!("어디에 팔까 — {:.1} ly 안, 단위당 차익", v.range)).size(12.0).color(DIM));
                route_bars(ui, &sells[..sells.len().min(5)], |p| p - cost, v.station);
            } else {
                let mut buys: Vec<(SystemId, f32)> = near.iter().filter_map(|s| market::buy_price(&t.trade.market, s.id, g, 0.0).map(|p| (s.id, p))).collect();
                buys.sort_by(|a, b| a.1.total_cmp(&b.1));
                ui.label(RichText::new(format!("어디서 살까 — {:.1} ly 안, 여기 판매가와의 차익", v.range)).size(12.0).color(DIM));
                let here = q.sell.unwrap_or(0.0);
                route_bars(ui, &buys[..buys.len().min(5)], |p| here - p, v.station);
            }
            ui.add_space(8.0);

            // Quantity and the deal.
            let buy_max = q.buy.map_or(0, |p| room.min((v.credits as f32 / (p * 1.1)).floor() as u32));
            let sell_max = if q.sell.is_some() { held } else { 0 };
            let max = buy_max.max(sell_max);
            st.qty = st.qty.min(max);
            if st.qty == 0 && max > 0 {
                st.qty = max.min(10);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("수량").color(DIM));
                ui.add_enabled(max > 0, egui::Slider::new(&mut st.qty, 0..=max.max(1)).show_value(true));
                for (label, n) in [("1", 1), ("10", 10), ("최대", max)] {
                    if ui.add_enabled(max > 0, egui::Button::new(label).small()).clicked() {
                        st.qty = n.min(max);
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let qty = st.qty;
                if let Some(p) = q.buy {
                    let n = qty.min(buy_max);
                    let text = RichText::new(format!("구매 {n}개\n≈ {:.0} CR", estimate(p, n, true))).size(15.0).strong().color(Color32::WHITE);
                    let ok = v.docked && n > 0;
                    let why = if room == 0 { "화물칸이 가득 찼습니다" } else { "크레딧이 부족합니다" };
                    if ui.add_enabled(ok, egui::Button::new(text).fill(px::BUY).min_size(Vec2::new(210.0, 50.0))).on_disabled_hover_text(why).clicked() {
                        req = Some(TradeRequest::Buy(g, n));
                    }
                }
                if let Some(p) = q.sell {
                    let n = qty.min(held);
                    let text = RichText::new(format!("판매 {n}개\n≈ {:.0} CR", estimate(p, n, false))).size(15.0).strong().color(Color32::WHITE);
                    if ui.add_enabled(v.docked && n > 0, egui::Button::new(text).fill(px::SELL).min_size(Vec2::new(210.0, 50.0))).on_disabled_hover_text("보유량이 없습니다").clicked() {
                        req = Some(TradeRequest::Sell(g, n));
                    }
                } else {
                    ui.label(RichText::new("이 정거장은 이 상품을 사지 않습니다").color(px::ILLEGAL));
                }
            });
            if illegal {
                ui.label(RichText::new("⚠ 중심(Core) 성계 정거장에 도킹하면 세관이 압수하고 벌금을 물립니다.").size(12.0).color(px::ILLEGAL));
            }
        });
    });
    req
}

/// One good's card: icon, name and where it stands here, the prices, what is held. True when clicked.
fn card(ui: &mut egui::Ui, q: &Quote, held: u32, picked: bool, illegal: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(326.0, 54.0), Sense::click());
    let p = ui.painter();
    let bg = if picked { px::CARD_PICKED } else if resp.hovered() { px::CARD_HOVER } else { px::CARD };
    p.rect_filled(rect, 8.0, bg);
    if picked {
        p.rect_stroke(rect, 8.0, Stroke::new(1.5, px::good(q.good)), StrokeKind::Inside);
    }
    icon(p, rect.left_center() + Vec2::new(26.0, 0.0), 19.0, q.good);
    p.text(rect.left_top() + Vec2::new(54.0, 8.0), Align2::LEFT_TOP, q.good.name(), FontId::proportional(15.0), px::TEXT);
    if let Some((chip, col)) = role_chip(q.role, illegal) {
        p.text(rect.left_top() + Vec2::new(54.0, 30.0), Align2::LEFT_TOP, format!("● {chip}"), FontId::proportional(11.0), col);
    }
    if held > 0 {
        p.text(rect.left_top() + Vec2::new(130.0, 30.0), Align2::LEFT_TOP, format!("보유 {held}"), FontId::proportional(11.0), GOLD);
    }
    let price = |v: Option<f32>| v.map_or("—".to_string(), |x| format!("{x:.0}"));
    p.text(rect.right_top() + Vec2::new(-12.0, 7.0), Align2::RIGHT_TOP, format!("살 때 {}", price(q.buy)), FontId::proportional(14.0), if q.buy.is_some() { px::MADE } else { DIM });
    p.text(rect.right_top() + Vec2::new(-12.0, 29.0), Align2::RIGHT_TOP, format!("팔 때 {}", price(q.sell)), FontId::proportional(13.0), if q.sell.is_some() { px::NEEDED } else { DIM });
    ui.add_space(4.0);
    resp.clicked()
}

/// The hold: a bar split by what fills it.
fn hold_bar(ui: &mut egui::Ui, hold: &BTreeMap<String, u32>, cap: u32) {
    let used: u32 = hold.values().sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(780.0), 22.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, px::TRACK);
    let mut x = rect.left();
    for (key, n) in hold {
        let w = rect.width() * *n as f32 / cap.max(1) as f32;
        let col = Good::of_key(key).map_or(DIM, px::good);
        p.rect_filled(Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w.min(rect.right() - x), rect.height())), 3.0, col.gamma_multiply(0.85));
        x += w;
    }
    p.text(rect.center(), Align2::CENTER_CENTER, format!("화물칸 {used} / {cap}"), FontId::proportional(13.0), px::TEXT);
}

/// From the galaxy's cheapest to its dearest station price, with this station's two marked.
fn price_gauge(ui: &mut egui::Ui, lo: f32, hi: f32, buy: Option<f32>, sell: Option<f32>) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(430.0, 40.0), Sense::hover());
    let p = ui.painter();
    let bar = Rect::from_min_max(rect.left_top() + Vec2::new(0.0, 12.0), rect.right_top() + Vec2::new(0.0, 20.0));
    let n = 24;
    for k in 0..n {
        let f = k as f32 / n as f32;
        let col = lerp_col(px::MADE, px::NEEDED, f);
        let r = Rect::from_min_max(Pos2::new(bar.left() + bar.width() * f, bar.top()), Pos2::new(bar.left() + bar.width() * (f + 1.0 / n as f32), bar.bottom()));
        p.rect_filled(r, 0.0, col.gamma_multiply(0.55));
    }
    let x = |v: f32| bar.left() + bar.width() * ((v - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
    if let Some(b) = buy {
        let at = Pos2::new(x(b), bar.top());
        p.add(Shape::convex_polygon(vec![at, at + Vec2::new(-6.0, -10.0), at + Vec2::new(6.0, -10.0)], px::MADE, Stroke::NONE));
    }
    if let Some(s) = sell {
        let at = Pos2::new(x(s), bar.bottom());
        p.add(Shape::convex_polygon(vec![at, at + Vec2::new(6.0, 10.0), at + Vec2::new(-6.0, 10.0)], px::NEEDED, Stroke::NONE));
    }
    p.text(bar.left_bottom() + Vec2::new(0.0, 10.0), Align2::LEFT_TOP, format!("은하 최저 {lo:.0}"), FontId::proportional(11.0), DIM);
    p.text(bar.right_bottom() + Vec2::new(0.0, 10.0), Align2::RIGHT_TOP, format!("최고 {hi:.0}"), FontId::proportional(11.0), DIM);
    p.text(bar.center_bottom() + Vec2::new(0.0, 10.0), Align2::CENTER_TOP, "▲ 여기서 살 때   ▼ 여기서 팔 때", FontId::proportional(11.0), DIM);
}

fn lerp_col(a: Color32, b: Color32, f: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// Stations with their price, a bar for the margin each gives, their distance.
fn route_bars(ui: &mut egui::Ui, list: &[(SystemId, f32)], margin: impl Fn(f32) -> f32, from: SystemId) {
    if list.is_empty() {
        ui.label(RichText::new("가까운 곳에 거래처가 없습니다 — 더 멀리 가 보세요.").size(12.0).color(DIM));
        return;
    }
    let best = list.iter().map(|(_, p)| margin(*p)).fold(1.0f32, f32::max);
    for (s, price) in list {
        let m = margin(*price);
        let def = galaxy::system(*s);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(430.0, 22.0), Sense::hover());
        let p = ui.painter();
        p.text(rect.left_center(), Align2::LEFT_CENTER, def.name, FontId::proportional(12.0), tier_color(def.tier));
        let bar = Rect::from_min_size(rect.left_top() + Vec2::new(170.0, 5.0), Vec2::new(150.0, 12.0));
        p.rect_filled(bar, 3.0, px::TRACK);
        if m > 0.0 {
            p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * (m / best).clamp(0.0, 1.0), bar.height())), 3.0, px::MADE.gamma_multiply(0.8));
        }
        let txt = if m > 0.0 { format!("+{m:.0}") } else { format!("{m:.0}") };
        p.text(bar.right_center() + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, txt, FontId::proportional(12.0), if m > 0.0 { px::MADE } else { DIM });
        p.text(rect.right_center(), Align2::RIGHT_CENTER, format!("{:.1} ly", distance(from, *s)), FontId::proportional(11.0), DIM);
    }
}
