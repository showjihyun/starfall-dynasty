//! The job board: a station's posted work and the jobs a captain carries, as a screen of its own
//! (F4, or the station window's button). Each job is a card with its kind's emblem, what it pays
//! and how far it sends you; the picked one opens a briefing — a chart of where it goes from here,
//! what waits there, the terms, and how it is done — with the one button that takes it or gives it
//! up. The jobs held sit below the board with the time each has left. Drawn from plain data and
//! checked headless (`tests/jobboard.rs`); the client sends what it returns.

use crate::palette::{DIM, GOLD, jobs as pj, tier_color};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Vec2};
use shared::galaxy::{self, SystemId};
use shared::jobs::{ActiveJob, Job, JobId, JobKind, JobTag};
use shared::market::{self, Role};

/// What the job board shows.
pub struct BoardView<'a> {
    /// The station docked at and its board (None: not docked anywhere).
    pub station: Option<SystemId>,
    pub jobs: &'a [Job],
    /// Game time the board changes.
    pub next_board: u64,
    pub held: &'a [ActiveJob],
    /// Game time now.
    pub unix: u64,
    pub max_active: usize,
    /// Free room in the hold.
    pub room: u32,
    /// The system the ship is in.
    pub here: SystemId,
}

/// The job picked on the board (kept between frames).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardState {
    pub selected: Option<JobId>,
}

/// A job board click.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobRequest {
    Accept(JobId),
    Abandon(JobId),
}

fn distance(a: SystemId, b: SystemId) -> f32 {
    shared::glam::Vec3::from(galaxy::system(a).map_pos).distance(shared::glam::Vec3::from(galaxy::system(b).map_pos))
}

fn hours(secs: u64) -> String {
    if secs >= 3600 { format!("{}시간 {:02}분", secs / 3600, secs % 3600 / 60) } else { format!("{}분", secs.div_ceil(60)) }
}

/// A job kind's emblem: a coloured well with its glyph, `r` across.
pub fn emblem(p: &egui::Painter, c: Pos2, r: f32, tag: JobTag) {
    let col = pj::tag(tag);
    p.circle_filled(c, r, pj::WELL);
    p.circle_stroke(c, r, Stroke::new(1.5, col.gamma_multiply(0.7)));
    let s = r * 0.5;
    let w = Stroke::new((r * 0.12).max(1.2), col);
    let at = |x: f32, y: f32| c + Vec2::new(x * s, y * s);
    match tag {
        // Cross-hairs.
        JobTag::Bounty => {
            p.circle_stroke(c, s * 0.75, w);
            for (a, b) in [((0.0, -1.1), (0.0, -0.4)), ((0.0, 1.1), (0.0, 0.4)), ((-1.1, 0.0), (-0.4, 0.0)), ((1.1, 0.0), (0.4, 0.0))] {
                p.line_segment([at(a.0, a.1), at(b.0, b.1)], w);
            }
            p.circle_filled(c, s * 0.14, col);
        }
        // A sealed crate: its box, the straps and the seal.
        JobTag::Haul => {
            p.rect_stroke(Rect::from_min_max(at(-0.9, -0.7), at(0.9, 0.8)), r * 0.06, w, StrokeKind::Middle);
            p.line_segment([at(-0.9, -0.15), at(0.9, -0.15)], w);
            p.line_segment([at(-0.3, -0.7), at(-0.3, 0.8)], w);
            p.circle_filled(at(0.35, 0.35), s * 0.18, col);
        }
        // A survey sweep: rings, the beam, a contact.
        JobTag::Survey => {
            p.circle_stroke(c, s * 0.95, w);
            p.circle_stroke(c, s * 0.5, Stroke::new(w.width * 0.8, col.gamma_multiply(0.7)));
            p.line_segment([c, at(0.67, -0.67)], w);
            p.circle_filled(at(-0.45, 0.4), s * 0.16, col);
        }
        // A crystal.
        JobTag::Prospect => {
            let pts = vec![at(0.0, -1.05), at(0.6, -0.35), at(0.4, 0.9), at(-0.4, 0.9), at(-0.6, -0.35)];
            p.add(Shape::convex_polygon(pts, col.gamma_multiply(0.3), w));
            p.line_segment([at(0.0, -1.05), at(0.0, 0.9)], Stroke::new(w.width * 0.7, col));
        }
        // A fleet: three arrowheads in a wedge.
        JobTag::Armada => {
            for (x, y, k) in [(0.0, -0.45, 1.0), (-0.6, 0.45, 0.7), (0.6, 0.45, 0.7)] {
                let tip = at(x, y - 0.5 * k);
                let pts = [tip, at(x + 0.38 * k, y + 0.45 * k), at(x, y + 0.2 * k), at(x - 0.38 * k, y + 0.45 * k)];
                p.add(Shape::convex_polygon(vec![pts[0], pts[1], pts[2]], col, Stroke::NONE));
                p.add(Shape::convex_polygon(vec![pts[0], pts[2], pts[3]], col, Stroke::NONE));
            }
        }
        // Goods going in: a box and the arrow down into it.
        JobTag::Supply => {
            p.add(Shape::line(vec![at(-0.9, 0.0), at(-0.9, 0.9), at(0.9, 0.9), at(0.9, 0.0)], w));
            p.line_segment([at(0.0, -1.0), at(0.0, 0.45)], w);
            p.add(Shape::convex_polygon(vec![at(-0.42, 0.05), at(0.42, 0.05), at(0.0, 0.6)], col, Stroke::NONE));
        }
    }
}

/// How a job is done, in a line.
fn how(kind: &JobKind) -> &'static str {
    match kind {
        JobKind::Bounty { .. } => "목표 성계로 워프하면 표적이 호위와 함께 나타납니다. 격침하는 즉시 보상이 들어옵니다.",
        JobKind::Haul { .. } => "수락하면 봉인 화물이 화물칸에 실립니다. 목적지 정거장에 도킹하면 완수 — 도중에 격침되면 화물과 함께 실패합니다.",
        JobKind::Survey { .. } => "순서와 상관없이 모든 성계에 들르면 완수입니다. 먼 성계일수록 측량료가 높습니다.",
        JobKind::Prospect { .. } => "등급 이상의 광물 표본을 들고 이 정거장에 도킹하면 납품됩니다. 위험한 성계의 소행성대에 희귀 표본이 많습니다.",
        JobKind::Armada { .. } => "목표 성계에 도착하면 해적 함대가 워프해 옵니다. 호위함과 함께 전부 격파하면 완수 — 대함대전입니다.",
        JobKind::Supply { .. } => "다른 성계 교역소(F3)에서 상품을 싸게 사 와 이 정거장에 도킹하면 납품됩니다.",
    }
}

/// The job board screen. Returns the click the player made this frame.
pub fn job_board(ui: &mut egui::Ui, v: &BoardView, st: &mut BoardState) -> Option<JobRequest> {
    let mut req = None;
    let known = |id: JobId| v.jobs.iter().any(|j| j.id == id) || v.held.iter().any(|a| a.job.id == id);
    if st.selected.is_none_or(|id| !known(id)) {
        st.selected = v.jobs.iter().find(|j| !v.held.iter().any(|a| a.job.id == j.id)).or(v.jobs.first()).map(|j| j.id).or(v.held.first().map(|a| a.job.id));
    }

    // ---- Header: where, when it changes, the slots.
    ui.horizontal(|ui| {
        match v.station {
            Some(s) => {
                let def = galaxy::system(s);
                ui.label(RichText::new(format!("{} 의뢰 게시판", def.name)).size(20.0).strong().color(pj::TEXT));
                ui.label(RichText::new(def.tier.label()).color(tier_color(def.tier)));
                ui.add_space(12.0);
                ui.label(RichText::new(format!("{} 뒤 새 의뢰", hours(v.next_board.saturating_sub(v.unix)))).color(DIM));
            }
            None => {
                ui.label(RichText::new("의뢰 게시판").size(20.0).strong().color(pj::TEXT));
            }
        }
    });
    slots(ui, v.held, v.max_active);
    ui.add_space(6.0);

    ui.horizontal_top(|ui| {
        // ---- Left: the board, then the jobs held.
        ui.vertical(|ui| {
            ui.set_width(370.0);
            match v.station {
                Some(_) if v.jobs.is_empty() => {
                    ui.label(RichText::new("지금은 올라온 의뢰가 없습니다.").color(DIM));
                }
                Some(_) => {
                    for j in v.jobs {
                        let taken = v.held.iter().any(|a| a.job.id == j.id);
                        let far = distance(v.here, j.kind.destinations(j.station).first().copied().unwrap_or(j.station));
                        let foot = format!("기한 {} · {far:.1} ly · 평판 +{}", hours(j.limit_secs), j.reputation);
                        if card(ui, j, &foot, None, taken, st.selected == Some(j.id)) {
                            st.selected = Some(j.id);
                        }
                    }
                }
                None => {
                    ui.label(RichText::new("게시판은 정거장에 도킹하면 열립니다. 정거장마다, 30분마다 다른 일이 올라옵니다.").color(GOLD));
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new(format!("맡은 의뢰 {}/{}", v.held.len(), v.max_active)).strong().color(pj::TEXT));
            if v.held.is_empty() {
                ui.label(RichText::new("아직 맡은 의뢰가 없습니다 — 어느 정거장에서든 여러 건을 함께 맡을 수 있습니다.").size(11.0).color(DIM));
            }
            for a in v.held {
                let left = a.deadline().saturating_sub(v.unix);
                let frac = left as f32 / a.job.limit_secs.max(1) as f32;
                let progress = match &a.job.kind {
                    JobKind::Survey { systems } => format!(" · 측량 {}/{}", a.visited.len(), systems.len()),
                    _ => String::new(),
                };
                let next = a.remaining().first().map_or(String::new(), |s| format!(" · → {}", galaxy::system(*s).name));
                let foot = format!("남은 시간 {}{progress}{next}", hours(left));
                if card(ui, &a.job, &foot, Some(frac), false, st.selected == Some(a.job.id)) {
                    st.selected = Some(a.job.id);
                }
            }
        });
        ui.add_space(10.0);
        // ---- Right: the briefing of the picked job.
        ui.vertical(|ui| {
            ui.set_width(450.0);
            let held = st.selected.and_then(|id| v.held.iter().find(|a| a.job.id == id));
            let Some(j) = held.map(|a| &a.job).or_else(|| st.selected.and_then(|id| v.jobs.iter().find(|j| j.id == id))) else {
                ui.label(RichText::new("의뢰를 고르면 브리핑이 열립니다.").color(DIM));
                return;
            };
            req = briefing(ui, v, j, held);
        });
    });
    req
}

/// The picked job: what it is, where it goes, what waits there, the terms and the button.
fn briefing(ui: &mut egui::Ui, v: &BoardView, j: &Job, held: Option<&ActiveJob>) -> Option<JobRequest> {
    let mut req = None;
    let tag = j.kind.tag();
    let dests = j.kind.destinations(j.station);
    let main = dests.first().copied().unwrap_or(j.station);
    let mdef = galaxy::system(main);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(450.0, 60.0), Sense::hover());
    let p = ui.painter();
    emblem(p, rect.left_center() + Vec2::new(28.0, 0.0), 25.0, tag);
    p.text(rect.left_top() + Vec2::new(64.0, 4.0), Align2::LEFT_TOP, tag.name(), FontId::proportional(22.0), pj::tag(tag));
    let chip = p.text(rect.left_top() + Vec2::new(70.0, 38.0), Align2::LEFT_TOP, mdef.tier.label(), FontId::proportional(11.0), tier_color(mdef.tier));
    p.rect_stroke(chip.expand2(Vec2::new(6.0, 2.0)), 6.0, Stroke::new(1.0, tier_color(mdef.tier)), StrokeKind::Outside);
    p.text(rect.right_top() + Vec2::new(-4.0, 6.0), Align2::RIGHT_TOP, j.reward.describe(), FontId::proportional(17.0), GOLD);
    ui.label(RichText::new(j.describe()).size(14.0).color(pj::TEXT));
    ui.add_space(4.0);
    chart(ui, v.here, j, held.map_or(&[][..], |a| &a.visited[..]));
    ui.add_space(4.0);

    // The terms, two to a row.
    let far = distance(v.here, main);
    let mut facts: Vec<(&str, String)> = vec![("보상", j.reward.describe()), ("평판", format!("+{}", j.reputation))];
    match held {
        Some(a) => facts.push(("남은 시간", hours(a.deadline().saturating_sub(v.unix)))),
        None => facts.push(("기한", format!("수락 후 {}", hours(j.limit_secs)))),
    }
    facts.push(("거리", format!("{far:.1} ly · {}", mdef.name)));
    match &j.kind {
        JobKind::Bounty { archetype, level, escorts, .. } => facts.push(("적 전력", format!("{} Lv{level} + 호위 {escorts}척", archetype.name()))),
        JobKind::Armada { level, wave, .. } => {
            let w: Vec<String> = wave.iter().map(|(a, n)| format!("{} {n}", a.name())).collect();
            facts.push(("적 함대", format!("{} · Lv{level}", w.join(" · "))));
        }
        JobKind::Haul { units, .. } => facts.push(("화물칸", format!("{units}칸 필요 (여유 {})", v.room))),
        JobKind::Survey { systems } => facts.push(("측량", format!("{}곳 · 방문 {}", systems.len(), held.map_or(0, |a| a.visited.len())))),
        JobKind::Prospect { rarity, count } => facts.push(("납품", format!("{} 이상 표본 {count}점", rarity.label()))),
        JobKind::Supply { good, units } => facts.push(("납품", format!("{} {units}단위", good.name()))),
    }
    egui::Grid::new(("job_terms", j.id)).num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
        for (k, (name, val)) in facts.iter().enumerate() {
            ui.label(RichText::new(*name).size(12.0).color(DIM));
            ui.label(RichText::new(val).size(13.0).color(pj::TEXT));
            if k % 2 == 1 {
                ui.end_row();
            }
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new(how(&j.kind)).size(12.0).color(DIM));
    ui.add_space(8.0);

    match held {
        Some(_) => {
            let text = RichText::new("의뢰 포기\n평판 −1").size(15.0).strong().color(Color32::WHITE);
            if ui.add(egui::Button::new(text).fill(pj::ABANDON).min_size(Vec2::new(200.0, 50.0))).on_hover_text("운송 의뢰의 봉인 화물은 반납됩니다").clicked() {
                req = Some(JobRequest::Abandon(j.id));
            }
        }
        None => {
            let full = v.held.len() >= v.max_active;
            let space = match j.kind {
                JobKind::Haul { units, .. } => units <= v.room,
                _ => true,
            };
            let docked = v.station == Some(j.station);
            let why = if !docked { "이 정거장에 도킹해야 수락할 수 있습니다" } else if full { "맡을 수 있는 의뢰가 가득 찼습니다" } else { "화물칸이 부족합니다" };
            let text = RichText::new(format!("의뢰 수락\n{}", j.reward.describe())).size(15.0).strong().color(Color32::WHITE);
            if ui.add_enabled(docked && !full && space, egui::Button::new(text).fill(pj::ACCEPT).min_size(Vec2::new(240.0, 50.0))).on_disabled_hover_text(why).clicked() {
                req = Some(JobRequest::Accept(j.id));
            }
            if docked && (full || !space) {
                ui.label(RichText::new(why).size(12.0).color(pj::WARN));
            }
        }
    }
    req
}

/// One job's card: its emblem, kind and errand, the pay, a footer line; `time` (the share of the
/// deadline left) draws a bar for a held job. True when clicked.
fn card(ui: &mut egui::Ui, j: &Job, foot: &str, time: Option<f32>, taken: bool, picked: bool) -> bool {
    let tag = j.kind.tag();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(366.0, 78.0), Sense::click());
    let p = ui.painter();
    let bg = if picked { pj::CARD_PICKED } else if resp.hovered() { pj::CARD_HOVER } else { pj::CARD };
    p.rect_filled(rect, 8.0, bg);
    p.rect_filled(Rect::from_min_size(rect.min, Vec2::new(4.0, rect.height())), 2.0, pj::tag(tag));
    if picked {
        p.rect_stroke(rect, 8.0, Stroke::new(1.5, pj::tag(tag)), StrokeKind::Inside);
    }
    emblem(p, rect.left_top() + Vec2::new(30.0, 30.0), 18.0, tag);
    p.text(rect.left_top() + Vec2::new(58.0, 7.0), Align2::LEFT_TOP, tag.name(), FontId::proportional(14.0), pj::tag(tag));
    p.text(rect.right_top() + Vec2::new(-10.0, 7.0), Align2::RIGHT_TOP, format!("{} CR", j.reward.credits), FontId::proportional(15.0), GOLD);
    let galley = p.layout(j.describe(), FontId::proportional(12.0), pj::TEXT, rect.width() - 70.0);
    p.galley(rect.left_top() + Vec2::new(58.0, 26.0), galley, pj::TEXT);
    p.text(rect.left_bottom() + Vec2::new(58.0, -6.0), Align2::LEFT_BOTTOM, foot, FontId::proportional(11.0), DIM);
    if taken {
        let r = p.text(rect.right_bottom() + Vec2::new(-12.0, -6.0), Align2::RIGHT_BOTTOM, "수락함", FontId::proportional(11.0), pj::ACCEPT_TEXT);
        p.rect_stroke(r.expand2(Vec2::new(5.0, 1.0)), 5.0, Stroke::new(1.0, pj::ACCEPT_TEXT), StrokeKind::Outside);
    }
    if let Some(f) = time {
        let bar = Rect::from_min_size(rect.left_bottom() + Vec2::new(58.0, -3.0), Vec2::new(rect.width() - 70.0, 2.0));
        p.rect_filled(bar, 1.0, pj::TRACK);
        let col = if f < 0.2 { pj::WARN } else { pj::tag(tag) };
        p.rect_filled(Rect::from_min_size(bar.min, Vec2::new(bar.width() * f.clamp(0.0, 1.0), bar.height())), 1.0, col);
    }
    ui.add_space(4.0);
    resp.clicked()
}

/// The slots: one diamond per job a captain may carry, lit for the ones held.
fn slots(ui: &mut egui::Ui, held: &[ActiveJob], max: usize) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(820.0), 18.0), Sense::hover());
    let p = ui.painter();
    p.text(rect.left_center(), Align2::LEFT_CENTER, "의뢰 슬롯", FontId::proportional(12.0), DIM);
    for k in 0..max {
        let c = rect.left_center() + Vec2::new(72.0 + 20.0 * k as f32, 0.0);
        let d = 6.5;
        let pts = vec![c + Vec2::new(0.0, -d), c + Vec2::new(d, 0.0), c + Vec2::new(0.0, d), c + Vec2::new(-d, 0.0)];
        match held.get(k) {
            Some(a) => p.add(Shape::convex_polygon(pts, pj::tag(a.job.kind.tag()), Stroke::NONE)),
            None => p.add(Shape::closed_line(pts, Stroke::new(1.2, DIM))),
        };
    }
}

/// Where the job goes, top down: the ship, the posting station, each destination (ticked once
/// visited), the systems around them faint.
fn chart(ui: &mut egui::Ui, here: SystemId, j: &Job, visited: &[SystemId]) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(450.0, 190.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 8.0, pj::CHART);
    let dests = j.kind.destinations(j.station);
    // A supply run: the nearest systems that make the good, where to buy it.
    let sources: Vec<SystemId> = match &j.kind {
        JobKind::Supply { good, .. } => {
            let t = shared::tuning::get();
            let mut near: Vec<SystemId> = galaxy::systems().iter().map(|s| s.id).filter(|s| *s != j.station && market::economy(&t.trade.market, *s).role(*good) == Role::Produce).collect();
            near.sort_by(|a, b| distance(j.station, *a).total_cmp(&distance(j.station, *b)));
            near.truncate(3);
            near
        }
        _ => vec![],
    };
    let mut pts: Vec<SystemId> = vec![here, j.station];
    pts.extend(dests.iter().copied());
    pts.extend(sources.iter().copied());
    let xz = |s: SystemId| {
        let m = galaxy::system(s).map_pos;
        Vec2::new(m[0], m[2])
    };
    let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
    for s in &pts {
        lo = lo.min(xz(*s));
        hi = hi.max(xz(*s));
    }
    // At least a few light years across, so a job at home still shows its neighbourhood.
    let grow = (Vec2::splat(4.0) - (hi - lo)).max(Vec2::ZERO) * 0.5;
    lo -= grow;
    hi += grow;
    let span = hi - lo;
    let inner = rect.shrink2(Vec2::new(46.0, 26.0));
    let scale = (inner.width() / span.x).min(inner.height() / span.y);
    let mid = (lo + hi) * 0.5;
    let to = |s: SystemId| inner.center() + (xz(s) - mid) * scale;
    // The neighbourhood, faint.
    for s in galaxy::systems() {
        let q = to(s.id);
        if rect.shrink(4.0).contains(q) {
            p.circle_filled(q, 2.0, pj::FAINT);
        }
    }
    let tag = j.kind.tag();
    let col = pj::tag(tag);
    // The way: from the ship to the station (when elsewhere), then out to each destination.
    let mut route = vec![here];
    if j.station != here && !matches!(j.kind, JobKind::Bounty { .. } | JobKind::Armada { .. } | JobKind::Survey { .. }) {
        route.push(j.station);
    }
    route.extend(dests.iter().copied());
    for w in route.windows(2) {
        dashed(p, to(w[0]), to(w[1]), Stroke::new(1.5, col.gamma_multiply(0.7)));
    }
    for s in &sources {
        let q = to(*s);
        dashed(p, q, to(j.station), Stroke::new(1.0, pj::FAINT));
        p.circle_stroke(q, 5.0, Stroke::new(1.5, col));
        p.text(q + Vec2::new(8.0, 4.0), Align2::LEFT_TOP, format!("{} · 생산", galaxy::system(*s).name), FontId::proportional(11.0), col);
    }
    // The posting station.
    let sp = to(j.station);
    p.circle_stroke(sp, 7.0, Stroke::new(1.5, GOLD));
    p.text(sp + Vec2::new(10.0, -10.0), Align2::LEFT_BOTTOM, format!("의뢰처 {}", galaxy::system(j.station).name), FontId::proportional(11.0), GOLD);
    // The destinations.
    for (k, d) in dests.iter().enumerate() {
        let q = to(*d);
        let done = visited.contains(d);
        let r = 7.0;
        let dia = vec![q + Vec2::new(0.0, -r), q + Vec2::new(r, 0.0), q + Vec2::new(0.0, r), q + Vec2::new(-r, 0.0)];
        p.add(Shape::convex_polygon(dia, if done { pj::FAINT } else { col }, Stroke::NONE));
        let name = galaxy::system(*d).name;
        let label = if done { format!("✓ {name}") } else if dests.len() > 1 { format!("{} {name}", k + 1) } else { name.to_string() };
        if *d != j.station {
            p.text(q + Vec2::new(10.0, 4.0), Align2::LEFT_TOP, label, FontId::proportional(11.0), tier_color(galaxy::system(*d).tier));
        }
    }
    // The ship.
    let h = to(here);
    p.add(Shape::convex_polygon(vec![h + Vec2::new(0.0, -7.0), h + Vec2::new(5.5, 5.0), h + Vec2::new(-5.5, 5.0)], pj::TEXT, Stroke::NONE));
    p.text(h + Vec2::new(0.0, 9.0), Align2::CENTER_TOP, "현재 위치", FontId::proportional(10.0), pj::TEXT);
}

fn dashed(p: &egui::Painter, a: Pos2, b: Pos2, s: Stroke) {
    let len = a.distance(b);
    if len < 1.0 {
        return;
    }
    let dir = (b - a) / len;
    let mut t = 0.0;
    while t < len {
        p.line_segment([a + dir * t, a + dir * (t + 6.0).min(len)], s);
        t += 10.0;
    }
}
