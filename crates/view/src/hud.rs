//! HUD drawing that needs only egui and plain data: gauge bars, the crosshair combat gauges and
//! the ship status panel. `tests/hud.rs` runs them in a headless egui context and checks the text
//! and geometry they produce.

use crate::palette::{ACCENT, GOLD, LOSS};
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2 as EVec2};
use shared::glam::Vec3;
use shared::protocol::{EntityKind, EntityState, FrontReport, SideId};
use shared::ships::{Ship, SizeClass};

/// Radius (px) of the crosshair gauge arcs and their width.
pub const CROSSHAIR_R: f32 = 92.0;
pub const CROSSHAIR_W: f32 = 9.0;
/// Each crosshair gauge spans this angle (radians).
pub const CROSSHAIR_SPAN: f32 = std::f32::consts::PI * 0.55;
/// Width of the shield and hull bars in the ship status panel.
pub const STATUS_BAR_W: f32 = 420.0;
/// Bottom-left chat: margin from the screen corner and width.
pub const CHAT_MARGIN: f32 = 14.0;
pub const CHAT_W: f32 = 380.0;

/// Every control, grouped, for the help window (F1).
pub const CONTROLS: &[(&str, &[(&str, &str)])] = &[
    ("비행", &[("W / S", "추진 · 역추진"), ("A / D", "선회"), ("R / F", "기수 상하"), ("Q / E", "롤"), ("Z / Ctrl", "상승 · 하강"), ("Alt + A / D", "좌우 평행이동"), ("방향키", "선회 · 상하"), ("우클릭 누른 채", "커서 쪽으로 조종"), ("Shift", "부스트")]),
    ("전투", &[("Space / 좌클릭", "사격 · 주포"), ("T", "목표 지정"), ("X", "미사일"), ("B", "호위함 공격 명령"), ("Y", "호위함 복귀")]),
    ("시점", &[("C", "1인칭 / 3인칭"), ("I", "함교 화면")]),
    ("행동", &[("G", "도킹"), ("N", "채굴"), ("V", "스캔")]),
    ("창", &[("M", "지도"), ("U", "정거장"), ("J", "미션"), ("O", "계약"), ("H", "연대기"), ("K", "함대"), ("L", "동맹"), ("F1", "도움말")]),
];

/// The always-on hint: the controls a new captain needs first; the rest is in the help window.
pub const KEY_HINTS: [&str; 2] = ["W/S 추진 · A/D 선회 · R/F 상하 · Q/E 롤 · Shift 부스트", "Space 사격 · T 목표 · X 미사일 · M 지도 · F1 모든 조작"];

/// Draws `KEY_HINTS` in the bottom-right corner (clear of the chat and the ship status panel).
pub fn key_hints(ctx: &egui::Context) {
    egui::Area::new("keys".into()).anchor(Align2::RIGHT_BOTTOM, EVec2::new(-CHAT_MARGIN, -CHAT_MARGIN)).interactable(false).show(ctx, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
            for l in KEY_HINTS {
                ui.label(RichText::new(l).size(11.0).color(crate::palette::DIM));
            }
        });
    });
}

/// Width of the controls table (fits a 1280-wide screen next to nothing else).
pub const CONTROLS_W: f32 = 600.0;

/// The controls table for the help window.
pub fn controls(ui: &mut egui::Ui) {
    // One wrapped row per group: group, then key (gold) and what it does, within CONTROLS_W.
    ui.set_max_width(CONTROLS_W);
    for (group, keys) in CONTROLS {
        ui.horizontal_wrapped(|ui| {
            ui.add_sized([36.0, 18.0], egui::Label::new(RichText::new(*group).color(ACCENT).strong()));
            for (k, what) in *keys {
                // A key and its meaning: one label that moves to the next line as a whole.
                let mut job = egui::text::LayoutJob::default();
                let font = egui::TextStyle::Body.resolve(ui.style());
                job.append(k, 0.0, egui::TextFormat::simple(font.clone(), GOLD));
                job.append(what, 6.0, egui::TextFormat::simple(font, ui.visuals().text_color()));
                ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend));
                ui.add_space(8.0);
            }
        });
        ui.add_space(4.0);
    }
}

/// Where to point at something the camera cannot show (off screen or behind): a spot on an inset
/// ring around the screen and the direction to turn. `local` is the object in camera space (−z ahead,
/// +y up); `projected` is where it lands on screen, if in front. None when it is on screen.
pub fn offscreen_marker(screen: egui::Rect, local: Vec3, projected: Option<Pos2>, inset: f32) -> Option<(Pos2, EVec2)> {
    let inner = screen.shrink(inset);
    if local.z < 0.0 && projected.is_some_and(|p| inner.contains(p)) {
        return None;
    }
    // Screen-space direction: right is +x, up the screen is −y. Straight behind: point down.
    let dir = EVec2::new(local.x, -local.y);
    let dir = if dir.length() < 1e-4 { EVec2::new(0.0, 1.0) } else { dir.normalized() };
    let half = inner.size() / 2.0;
    // Scale the direction until it touches the inner rectangle.
    let k = (half.x / dir.x.abs().max(1e-6)).min(half.y / dir.y.abs().max(1e-6));
    Some((inner.center() + dir * k, dir))
}

/// "▲ 0.42 km · +23°": how far above (▲) or below (▼) me something is, and at what elevation angle.
pub fn elevation_text(me: Vec3, it: Vec3) -> String {
    let d = it - me;
    let dist = d.length().max(1e-3);
    let angle = (d.y / dist).clamp(-1.0, 1.0).asin().to_degrees();
    let arrow = if d.y >= 0.0 { "▲" } else { "▼" };
    if d.y.abs() >= 1000.0 { format!("{arrow} {:.2} km · {angle:+.0}°", d.y.abs() / 1000.0) } else { format!("{arrow} {:.0} m · {angle:+.0}°", d.y.abs()) }
}

/// Target panel line on its turret arcs (`shared::ships::batteries_reaching`): how many of its
/// batteries can reach me, and the hint to dive when all of them can. None: it has no turrets.
pub fn battery_text(reach: u8, total: u8) -> Option<String> {
    if total == 0 {
        return None;
    }
    Some(if reach >= total { format!("포탑 {total}문 모두 나를 겨눔 — 위·아래로 파고들면 절반") } else { format!("포탑 {reach}/{total}문만 닿음 — 사격호 밖") })
}

/// The defender's side of the same rule: when my own batteries cannot all reach my target (it sits
/// above or below me), how to bring them to bear. None: no turrets, or every battery already reaches.
pub fn roll_hint(reach: u8, total: u8) -> Option<String> {
    (total > 0 && reach < total).then(|| format!("내 포탑 {reach}/{total}문만 닿음\nQ/E로 굴러 선체 옆에 두면 {total}/{total}문"))
}

/// Target panel warning (docs/3D_전투_설계.md, 함대 필수): captains the rules make hostile to me, other
/// than my target, inside my turret reach `range` — while I hold the trigger my spare batteries shoot
/// them too (`sim` turret_search). Usually someone fighting the same enemy outside my fleet.
/// None: I have no turrets (`turrets`), or nobody is there.
pub fn stray_fire_warning<'a>(viewer: &crate::relation::Viewer, me: &EntityState, target: &EntityState, turrets: u8, range: f32, ents: impl IntoIterator<Item = &'a EntityState>) -> Option<String> {
    if turrets == 0 {
        return None;
    }
    let at = Vec3::from(me.pos);
    let near: Vec<&EntityState> = ents
        .into_iter()
        .filter(|e| e.kind == EntityKind::Player && e.id != target.id && !e.docked && Vec3::from(e.pos).distance(at) <= range + e.radius && viewer.relation(e) == crate::relation::Relation::Hostile)
        .collect();
    let first = near.iter().min_by_key(|e| e.id)?;
    let more = if near.len() > 1 { format!(" 외 {}척", near.len() - 1) } else { String::new() };
    Some(format!("함대 아님: {}{more}\n사격하면 내 포탑이 맞힐 수 있다 — K로 함대", first.name))
}

pub fn bar(ui: &mut egui::Ui, frac: f32, color: Color32, label: String, w: f32) {
    bar_with_loss(ui, frac, frac, color, label, w, 16.0);
}

/// Bar whose recently lost part (`lag` > `frac`) lingers in bright orange before draining away.
pub fn bar_with_loss(ui: &mut egui::Ui, frac: f32, lag: f32, color: Color32, label: String, w: f32, h: f32) {
    let (rect, _) = ui.allocate_exact_size(EVec2::new(w, h), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, crate::palette::chrome::TRACK);
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
        p.line_segment([Pos2::new(x, rect.bottom() - h * 0.35), Pos2::new(x, rect.bottom())], Stroke::new(1.0, crate::palette::chrome::TICK));
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

pub use crate::palette::hull_color;

/// Shield gauge colour: blue, blinking below 25%.
pub fn shield_color(shield: f32, pulse: f32) -> Color32 {
    if shield > 0.25 { crate::palette::SHIELD } else { crate::palette::alpha(crate::palette::SHIELD, (255.0 * pulse) as u8) }
}

/// Combat gauges hugging the crosshair: shield on the left, hull on the right. Both fill from the
/// bottom up, the just-lost part glows orange while it drains, and low values pulse.
pub fn crosshair_gauges(p: &egui::Painter, c: Pos2, shield: f32, hull: f32, lag: [f32; 2], now: f64) {
    use std::f32::consts::PI;
    let (r, span) = (CROSSHAIR_R, CROSSHAIR_SPAN);
    let track = Stroke::new(CROSSHAIR_W, crate::palette::chrome::GAUGE_TRACK);
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

/// Where the crosshair and its gauges are drawn around the screen centre `c` (arcs with their
/// "실드"/"선체" readouts on each side, the cross in the middle): world labels stay out of it.
pub fn crosshair_keepout(c: Pos2) -> [egui::Rect; 3] {
    let (r, half) = (CROSSHAIR_R, CROSSHAIR_SPAN / 2.0);
    let inner = r * half.cos() - CROSSHAIR_W;
    let tall = r * half.sin() + CROSSHAIR_W;
    // The readouts reach about 45 px beyond the arcs (two lines of 15 px text).
    let side = |s: f32| egui::Rect::from_two_pos(Pos2::new(c.x + s * inner, c.y - tall), Pos2::new(c.x + s * (r + 10.0 + 48.0), c.y + tall));
    [side(-1.0), side(1.0), egui::Rect::from_center_size(c, EVec2::splat(60.0))]
}

/// Where to put a world label of `size` next to its marker at `anchor`: to the right (the usual
/// spot), left, below or above — the first that stays clear of `keepout` — or else just below the
/// keep-out it hits.
pub fn place_label(anchor: Pos2, size: EVec2, keepout: &[egui::Rect]) -> egui::Rect {
    let at = |min: Pos2| egui::Rect::from_min_size(min, size);
    let spots = [
        at(anchor + EVec2::new(9.0, -6.0)),
        at(anchor + EVec2::new(-9.0 - size.x, -6.0)),
        at(anchor + EVec2::new(-size.x / 2.0, 10.0)),
        at(anchor + EVec2::new(-size.x / 2.0, -10.0 - size.y)),
    ];
    if let Some(r) = spots.iter().find(|r| !keepout.iter().any(|k| k.intersects(**r))) {
        return *r;
    }
    let mut r = spots[2];
    while let Some(k) = keepout.iter().find(|k| k.intersects(r)) {
        r = r.translate(EVec2::new(0.0, k.bottom() + 2.0 - r.top()));
    }
    r
}

/// How much of a ship's world marker to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Brackets and the name label.
    Full,
    /// Brackets only.
    Bracket,
    /// A dot in its relation colour.
    Dot,
}

/// At most this many ships get a name label, and this many their brackets, in a crowded battle.
pub const MAX_LABELS: usize = 24;
pub const MAX_BRACKETS: usize = 96;
/// Size assumed for a name label when checking that two don't overlap.
pub const LABEL_SIZE: EVec2 = EVec2::new(150.0, 14.0);

/// A ship on screen, for [`declutter`]: where it is, the half size of its brackets, its distance.
#[derive(Clone, Copy, Debug)]
pub struct MarkCand {
    pub at: Pos2,
    pub r: f32,
    pub dist: f32,
    pub target: bool,
    pub hostile: bool,
}

/// Where a ship's name label goes: centred under its brackets, unless that is on the crosshair
/// gauges — then beside them ([`place_label`]).
pub fn label_rect(c: &MarkCand, keepout: &[egui::Rect]) -> egui::Rect {
    let under = egui::Rect::from_min_size(c.at + EVec2::new(-LABEL_SIZE.x / 2.0, c.r + 4.0), LABEL_SIZE);
    if keepout.iter().any(|k| k.intersects(under)) { place_label(c.at, LABEL_SIZE, keepout) } else { under }
}

/// Which marks to draw when thousands of ships are on screen: the target always in full, then the
/// nearest ships (hostiles first within the same distance band) — a label only where it doesn't
/// overlap one already placed, at most [`MAX_LABELS`] — then brackets for the next nearest up to
/// [`MAX_BRACKETS`], and a dot for every other ship, so the battle stays readable. Each mark comes
/// with the rect its label goes in ([`label_rect`]).
pub fn declutter(cands: &[MarkCand], keepout: &[egui::Rect]) -> Vec<(Mark, egui::Rect)> {
    let mut order: Vec<usize> = (0..cands.len()).collect();
    // Hostiles count as half as far: a nearby enemy outranks an ally at the same range.
    let rank = |c: &MarkCand| if c.hostile { c.dist * 0.5 } else { c.dist };
    order.sort_by(|a, b| {
        let (a, b) = (&cands[*a], &cands[*b]);
        b.target.cmp(&a.target).then(rank(a).total_cmp(&rank(b)))
    });
    let mut marks = vec![(Mark::Dot, egui::Rect::NOTHING); cands.len()];
    let mut placed: Vec<egui::Rect> = Vec::with_capacity(MAX_LABELS);
    for (n, i) in order.into_iter().enumerate() {
        let c = &cands[i];
        if n >= MAX_BRACKETS && !c.target {
            break;
        }
        let r = label_rect(c, keepout);
        let mark = if c.target || (placed.len() < MAX_LABELS && !placed.iter().any(|p| p.intersects(r))) {
            placed.push(r);
            Mark::Full
        } else {
            Mark::Bracket
        };
        marks[i] = (mark, r);
    }
    marks
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
        bar_with_loss(ui, m.shield, lag[0], crate::palette::SHIELD, format!("에너지 실드 {:.0}/{:.0}", m.shield * st.max_shield, st.max_shield), STATUS_BAR_W, 22.0);
        bar_with_loss(ui, m.hull, lag[1], hull_color(m.hull, 1.0), format!("선체 내구도 {:.0}/{:.0}", m.hull * st.max_hull, st.max_hull), STATUS_BAR_W, 22.0);
        let speed = Vec3::from(m.vel).length();
        ui.label(RichText::new(format!("속도 {:.0} u/s{} · 화물 {}/{:.0} · 워프 {:.2} ly{}", speed, if m.boosting { " ⚡" } else { "" }, cargo_used, st.cargo, st.warp_range, if m.docked { " · 도킹 중" } else { "" })).size(12.0));
    }
}

/// Width of the war-zone report (top centre).
pub const WAR_W: f32 = 460.0;

/// Who is winning, from the front reports: (ours, theirs) strength and the two names. "Ours" is
/// `mine` when it fights here; otherwise (a bystander) the strongest side against all the rest.
pub fn war_balance(reports: &[FrontReport], mine: Option<SideId>) -> Option<(u32, u32, String, String)> {
    let mut total: std::collections::BTreeMap<SideId, (u32, String)> = Default::default();
    for s in reports.iter().flat_map(|r| &r.sides) {
        let t = total.entry(s.side).or_insert((0, s.name.clone()));
        t.0 += s.strength;
    }
    let us = mine.filter(|m| total.contains_key(m)).or_else(|| overall_leader(reports))?;
    let ours = total[&us].0;
    let them: u32 = total.iter().filter(|(k, _)| **k != us).map(|(_, v)| v.0).sum();
    let mut rivals: Vec<(&SideId, &(u32, String))> = total.iter().filter(|(k, _)| **k != us).collect();
    rivals.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    let their_name = match rivals.as_slice() {
        [] => "—".to_string(),
        [one] => one.1.1.clone(),
        [first, rest @ ..] => format!("{} 외 {}", first.1.1, rest.len()),
    };
    Some((ours, them, total[&us].1.clone(), their_name))
}

/// A two-colour balance bar: `ours` share from the left in `ours_col`, the rest in `theirs_col`.
fn balance_bar(ui: &mut egui::Ui, ours: u32, theirs: u32, w: f32, h: f32, ours_col: Color32, theirs_col: Color32) -> f32 {
    let (rect, _) = ui.allocate_exact_size(EVec2::new(w, h), egui::Sense::hover());
    let p = ui.painter();
    let total = (ours + theirs).max(1) as f32;
    let share = if ours + theirs == 0 { 0.5 } else { ours as f32 / total };
    p.rect_filled(rect, 2.0, theirs_col);
    p.rect_filled(egui::Rect::from_min_size(rect.min, EVec2::new(w * share, h)), 2.0, ours_col);
    // The even line, so a lead reads at a glance.
    p.line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, crate::palette::chrome::INK));
    share
}

/// The war-zone report under the top edge: one bar for the whole battle (us against them, by hull +
/// shield left), then a row per front — its name, a bar, ships and captains lost on each side —
/// with the front we are on marked. `names` are the system's fronts (`galaxy::fronts`).
pub fn war_report(ui: &mut egui::Ui, names: &[String], reports: &[FrontReport], mine: Option<SideId>, here: Option<usize>) {
    use crate::palette::{GREEN, RED};
    let Some((ours, theirs, our_name, their_name)) = war_balance(reports, mine) else { return };
    let involved = mine.is_some_and(|m| reports.iter().flat_map(|r| &r.sides).any(|s| s.side == m));
    let (us_label, them_label) = if involved { ("아군", "적") } else { ("우세", "도전") };
    ui.set_width(WAR_W);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{us_label} {our_name}")).color(GREEN).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("{them_label} {their_name}")).color(RED).strong());
        });
    });
    let share = balance_bar(ui, ours, theirs, WAR_W, 12.0, GREEN, RED);
    ui.label(RichText::new(format!("전체 전력 {:.0}% : {:.0}%", share * 100.0, (1.0 - share) * 100.0)).size(11.0).color(crate::palette::DIM));
    // The same "us" as the top bar: our side, or for a bystander the strongest overall.
    let us = mine.filter(|_| involved).or_else(|| overall_leader(reports));
    for r in reports {
        let name = names.get(r.front as usize).cloned().unwrap_or_else(|| format!("전선 {}", r.front + 1));
        // (strength, ships, captains lost) for us and for everyone else on this front.
        let (mut o, mut t) = ((0u32, 0u32, 0u32), (0u32, 0u32, 0u32));
        for s in &r.sides {
            let acc = if Some(s.side) == us { &mut o } else { &mut t };
            acc.0 += s.strength;
            acc.1 += s.ships;
            acc.2 += s.lost;
        }
        let mark = if here == Some(r.front as usize) { "▶ " } else { "" };
        ui.horizontal(|ui| {
            ui.add_sized([150.0, 16.0], egui::Label::new(RichText::new(format!("{mark}{name}")).size(12.0).color(if mark.is_empty() { crate::palette::DIM } else { GOLD })).truncate());
            balance_bar(ui, o.0, t.0, 120.0, 8.0, GREEN, RED);
            ui.label(RichText::new(format!("{}척 : {}척 · 손실 {} : {}", o.1, t.1, o.2, t.2)).size(11.0));
        });
    }
}

/// The strongest side over all fronts.
fn overall_leader(reports: &[FrontReport]) -> Option<SideId> {
    let mut total: std::collections::BTreeMap<SideId, u32> = Default::default();
    for s in reports.iter().flat_map(|r| &r.sides) {
        *total.entry(s.side).or_default() += s.strength;
    }
    total.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(k, _)| k)
}
