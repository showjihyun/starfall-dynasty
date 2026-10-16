//! HUD drawing that needs only egui and plain data: gauge bars, the crosshair combat gauges and
//! the ship status panel. `tests/hud.rs` runs them in a headless egui context and checks the text
//! and geometry they produce.

use crate::palette::{ACCENT, GOLD, GREEN, LOSS, RED};
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2 as EVec2};
use shared::glam::Vec3;
use shared::protocol::EntityState;
use shared::ships::{Ship, SizeClass};

/// Radius (px) of the crosshair gauge arcs and their width.
pub const CROSSHAIR_R: f32 = 92.0;
pub const CROSSHAIR_W: f32 = 9.0;
/// Each crosshair gauge spans this angle (radians).
pub const CROSSHAIR_SPAN: f32 = std::f32::consts::PI * 0.55;
/// Width of the shield and hull bars in the ship status panel.
pub const STATUS_BAR_W: f32 = 420.0;

pub fn bar(ui: &mut egui::Ui, frac: f32, color: Color32, label: String, w: f32) {
    bar_with_loss(ui, frac, frac, color, label, w, 16.0);
}

/// Bar whose recently lost part (`lag` > `frac`) lingers in bright orange before draining away.
pub fn bar_with_loss(ui: &mut egui::Ui, frac: f32, lag: f32, color: Color32, label: String, w: f32, h: f32) {
    let (rect, _) = ui.allocate_exact_size(EVec2::new(w, h), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(20, 25, 35, 200));
    let (frac, lag) = (frac.clamp(0.0, 1.0), lag.clamp(0.0, 1.0));
    if lag > frac {
        let mut l = rect;
        l.set_width(rect.width() * lag);
        p.rect_filled(l, 3.0, LOSS);
    }
    let mut f = rect;
    f.set_width(rect.width() * frac);
    p.rect_filled(f, 3.0, color);
    for k in 1..10 {
        let x = rect.left() + rect.width() * k as f32 / 10.0;
        p.line_segment([Pos2::new(x, rect.bottom() - h * 0.35), Pos2::new(x, rect.bottom())], Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 90)));
    }
    p.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(h * 0.7), Color32::WHITE);
}

/// Arc from angle `a0` to `a1` (radians, screen space: 0 = right, +pi/2 = down).
pub fn arc(p: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, stroke: Stroke) {
    if (a1 - a0).abs() < 1e-3 {
        return;
    }
    let n = ((a1 - a0).abs() * 24.0).ceil().max(2.0) as usize;
    let pts: Vec<Pos2> = (0..=n).map(|i| a0 + (a1 - a0) * i as f32 / n as f32).map(|a| c + EVec2::new(a.cos(), a.sin()) * r).collect();
    p.add(egui::Shape::line(pts, stroke));
}

/// Hull gauge colour: green, amber below 60%, pulsing red below 30%.
pub fn hull_color(hull: f32, pulse: f32) -> Color32 {
    if hull > 0.6 {
        GREEN
    } else if hull > 0.3 {
        Color32::from_rgb(255, 200, 80)
    } else {
        Color32::from_rgb(255, (70.0 * pulse) as u8, (60.0 * pulse) as u8)
    }
}

/// Shield gauge colour: blue, blinking below 25%.
pub fn shield_color(shield: f32, pulse: f32) -> Color32 {
    if shield > 0.25 { Color32::from_rgb(80, 160, 255) } else { Color32::from_rgba_unmultiplied(80, 160, 255, (255.0 * pulse) as u8) }
}

/// Combat gauges hugging the crosshair: shield on the left, hull on the right. Both fill from the
/// bottom up, the just-lost part glows orange while it drains, and low values pulse.
pub fn crosshair_gauges(p: &egui::Painter, c: Pos2, shield: f32, hull: f32, lag: [f32; 2], now: f64) {
    use std::f32::consts::PI;
    let (r, span) = (CROSSHAIR_R, CROSSHAIR_SPAN);
    let track = Stroke::new(CROSSHAIR_W, Color32::from_rgba_unmultiplied(15, 20, 30, 150));
    let pulse = 0.55 + 0.45 * ((now * 8.0).sin() as f32 * 0.5 + 0.5);
    let hull_col = hull_color(hull, pulse);
    let shield_col = shield_color(shield, pulse);
    let loss = Stroke::new(CROSSHAIR_W, LOSS);
    // Screen angles grow clockwise (+y is down): each arc starts below the horizon and climbs.
    let (l_bot, r_bot) = (PI - span / 2.0, span / 2.0);
    for (frac, lagf, col, bottom, dir) in [(shield, lag[0], shield_col, l_bot, -1.0f32), (hull, lag[1], hull_col, r_bot, 1.0f32)] {
        let at = |f: f32| bottom - dir * span * f.clamp(0.0, 1.0);
        arc(p, c, r, at(0.0), at(1.0), track);
        if lagf > frac {
            arc(p, c, r, at(frac), at(lagf), loss);
        }
        arc(p, c, r, at(0.0), at(frac), Stroke::new(CROSSHAIR_W, col));
    }
    p.text(c + EVec2::new(-r - 10.0, 0.0), Align2::RIGHT_CENTER, format!("실드\n{:.0}%", shield * 100.0), FontId::proportional(15.0), shield_col);
    p.text(c + EVec2::new(r + 10.0, 0.0), Align2::LEFT_CENTER, format!("선체\n{:.0}%", hull * 100.0), FontId::proportional(15.0), hull_col);
}

/// The ship status panel (bottom centre): name and class, the growth line, shield/hull bars with
/// the lost part trailing, and speed, cargo and warp range.
pub fn ship_status(ui: &mut egui::Ui, ship: &Ship, cargo_used: u32, me: Option<&EntityState>, lag: [f32; 2]) {
    let st = ship.stats();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("「{}」", ship.name)).strong().size(16.0));
        ui.label(RichText::new(format!("{} · {}급", ship.hull.name(), ship.designation())).color(ACCENT));
    });
    let class = ship.size_class();
    let next = match class {
        SizeClass::Small => format!(" · Rank 3에서 순양함으로 재건조 (현재 Rank {})", ship.rank()),
        SizeClass::Cruiser => format!(" · Rank 7에서 주력함으로 (현재 Rank {})", ship.rank()),
        SizeClass::Capital => String::new(),
    };
    ui.label(
        RichText::new(format!(
            "{} · 전장 {:.0}m · 포탑 {}문{}{}{}",
            class.name(),
            st.radius * 2.2,
            st.turrets,
            if st.main_damage > 0.0 { " · 함수 주포" } else { "" },
            if st.escorts > 0 { format!(" · 호위함 {}척", st.escorts) } else { String::new() },
            next
        ))
        .size(11.0)
        .color(GOLD),
    );
    if let Some(m) = me {
        bar_with_loss(ui, m.shield, lag[0], Color32::from_rgb(60, 140, 255), format!("에너지 실드 {:.0}/{:.0}", m.shield * st.max_shield, st.max_shield), STATUS_BAR_W, 22.0);
        bar_with_loss(ui, m.hull, lag[1], if m.hull > 0.35 { Color32::from_rgb(70, 190, 110) } else { RED }, format!("선체 내구도 {:.0}/{:.0}", m.hull * st.max_hull, st.max_hull), STATUS_BAR_W, 22.0);
        let speed = Vec3::from(m.vel).length();
        ui.label(RichText::new(format!("속도 {:.0} u/s{} · 화물 {}/{:.0} · 워프 {:.2} ly{}", speed, if m.boosting { " ⚡" } else { "" }, cargo_used, st.cargo, st.warp_range, if m.docked { " · 도킹 중" } else { "" })).size(12.0));
    }
}
