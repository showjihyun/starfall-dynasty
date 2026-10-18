//! Full-screen star map (M). Mouse wheel zooms, drag pans, and zoom is continuous across two scales:
//! - Galaxy chart: systems in light-years on the sector grid, warp range and fleet members.
//! - System chart: zooming in past the galaxy limit dives into the current system. It shows the
//!   star, planets, station, warp point, asteroid fields, points of interest and every ship, all live
//!   (interpolated each frame like the 3D view) with headings, ranges and names.
//!
//! Readouts use the Starfall coordinates of [`shared::galaxy::coords`].

use crate::net::Net;
use crate::scene::Env;
use crate::state::{INTERP_DELAY, Session};
use bevy::prelude::Vec3;
use bevy_egui::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, Vec2 as EVec2};
use shared::galaxy::{self, PoiKind, SystemId, coords};
use shared::protocol::*;
pub use view::mapview::Level;
use view::mapview::{nice_step, system_extent};
use view::palette::{ACCENT, DIM, GOLD, GREEN, RED};
use view::relation::Viewer;

use view::palette::map::{BG, GRID, GRID_TEXT};
use view::palette::{alpha, map};

use view::palette::tier_color;

fn rgb(c: [f32; 3]) -> Color32 {
    Color32::from_rgb((c[0].clamp(0.0, 1.0) * 255.0) as u8, (c[1].clamp(0.0, 1.0) * 255.0) as u8, (c[2].clamp(0.0, 1.0) * 255.0) as u8)
}

pub fn draw(ctx: &egui::Context, s: &mut Session, net: &Net, env: &Env, now: f64) {
    let open = s.ui.map;
    s.map.set_open(open, ctx.content_rect());
    if !open {
        return;
    }
    let screen = ctx.content_rect();
    egui::Area::new("starmap".into()).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let (rect, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click_and_drag());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 0.0, BG);
        // Map area leaves room for the side panel on the right.
        let side_w = 330.0;
        let map = Rect::from_min_max(rect.min, Pos2::new(rect.max.x - side_w, rect.max.y));
        let scroll = if resp.hovered() { ui.input(|i| i.smooth_scroll_delta.y) } else { 0.0 };
        let hover = resp.hover_pos().filter(|h| map.contains(*h));
        match s.map.level {
            Level::Galaxy => galaxy_chart(ui, &p, map, &resp, scroll, hover, s, env, now),
            Level::System => system_chart(ui, &p, map, &resp, scroll, hover, s, net, env, now),
        }
        side_panel(ui, Rect::from_min_max(Pos2::new(map.max.x, rect.min.y), rect.max), s, net, now);
        top_bar(ui, map, s, env);
    });
}

fn top_bar(ui: &mut egui::Ui, map: Rect, s: &mut Session, env: &Env) {
    let bar = Rect::from_min_size(map.min + EVec2::new(14.0, 12.0), EVec2::new(map.width() - 28.0, 34.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
        ui.horizontal(|ui| {
            let sys = galaxy::system(s.system);
            let crumb = match s.map.level {
                Level::Galaxy => "은하 전도".to_string(),
                Level::System => format!("은하 전도 › {} 성계", sys.name),
            };
            ui.label(RichText::new(crumb).size(18.0).strong().color(GOLD));
            ui.add_space(16.0);
            if ui.button("은하").clicked() {
                s.map.show_galaxy(s.system);
            }
            if ui.button("현재 성계").clicked() {
                s.map.frame_system(map, system_extent(env.layout.as_ref()));
            }
            if ui.button("내 위치").clicked() {
                s.map.follow_me();
            }
            if ui.button(" + ").clicked() {
                s.map.zoom_by(1.5);
            }
            if ui.button(" − ").clicked() {
                s.map.zoom_by(1.0 / 1.5);
            }
            if ui.button(if s.map.is_3d() { "평면" } else { "입체" }).clicked() {
                s.map.toggle_3d();
            }
            if ui.button("닫기 (M)").clicked() {
                s.ui.map = false;
            }
            let hint = if s.map.level == Level::System { "휠: 확대/축소 · 드래그: 이동 · 우클릭 드래그: 회전·기울이기 · 함선 클릭: 목표 지정" } else { "휠: 확대/축소 · 드래그: 이동 · 우클릭 드래그: 회전·기울이기 · 성계 클릭: 선택" };
            ui.label(RichText::new(hint).size(11.0).color(DIM));
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn galaxy_chart(_ui: &mut egui::Ui, p: &egui::Painter, map: Rect, resp: &egui::Response, scroll: f32, hover: Option<Pos2>, s: &mut Session, env: &Env, now: f64) {
    // Zooming in past the limit dives into the current system.
    if resp.dragged_by(egui::PointerButton::Secondary) {
        s.map.rotate(resp.drag_delta());
    }
    let pan = resp.dragged_by(egui::PointerButton::Primary).then(|| resp.drag_delta());
    if s.map.galaxy_input(map, pan, scroll, hover, system_extent(env.layout.as_ref())) {
        return;
    }
    let v = s.map.clone();
    let sc = v.g_scale;
    let to = |mp: [f32; 3]| v.galaxy_to_screen(map, mp);
    // Sector grid on the galactic plane (y = 0).
    let (x0, x1, z0, z1) = v.galaxy_bounds(map);
    let (z0, z1) = (z0.floor(), z1.ceil());
    for xi in (x0 - coords::SECTOR_ORIGIN_X).floor() as i32..=(x1 - coords::SECTOR_ORIGIN_X).ceil() as i32 {
        let x = coords::SECTOR_ORIGIN_X + xi as f32;
        p.line_segment([to([x, 0.0, z0]), to([x, 0.0, z1])], Stroke::new(1.0, GRID));
    }
    for zi in z0 as i32..=z1 as i32 {
        let (xa, xb) = (coords::SECTOR_ORIGIN_X + ((x0 - coords::SECTOR_ORIGIN_X).floor()), coords::SECTOR_ORIGIN_X + ((x1 - coords::SECTOR_ORIGIN_X).ceil()));
        p.line_segment([to([xa, 0.0, zi as f32]), to([xb, 0.0, zi as f32])], Stroke::new(1.0, GRID));
    }
    for xi in (x0 - coords::SECTOR_ORIGIN_X).floor() as i32..(x1 - coords::SECTOR_ORIGIN_X).ceil() as i32 {
        // Only charted sectors are named (columns A–Z from the origin, rows from 1).
        if !(0..26).contains(&xi) {
            continue;
        }
        for zi in z0.floor().max(0.0) as i32..z1.ceil() as i32 {
            let (x, z) = (coords::SECTOR_ORIGIN_X + xi as f32, zi as f32);
            let name = coords::sector_of([x + 0.5, 0.0, z + 0.5]);
            p.text(to([x, 0.0, z + 1.0]) + EVec2::new(4.0, 3.0), Align2::LEFT_TOP, name, FontId::monospace(11.0), GRID_TEXT);
        }
    }
    // Lanes, warp range, systems.
    let cur = galaxy::system(s.system);
    let range = s.profile.as_ref().map(|p| p.ship.stats().warp_range).unwrap_or(0.0);
    for a in galaxy::systems() {
        for b in galaxy::systems() {
            // Lanes join neighbours one short hop apart (every system but The Deep has one).
            if a.id < b.id && galaxy::distance_ly(a.id, b.id) <= 1.5 {
                p.line_segment([to(a.map_pos), to(b.map_pos)], Stroke::new(1.0, map::LANE));
            }
        }
    }
    let cp = to(cur.map_pos);
    p.circle_stroke(cp, range * sc, Stroke::new(1.0, alpha(ACCENT, 70)));
    p.text(cp + EVec2::new(0.0, -range * sc - 4.0), Align2::CENTER_BOTTOM, format!("워프 사거리 {range:.2} ly"), FontId::proportional(11.0), alpha(ACCENT, 140));
    let mut hovered: Option<SystemId> = None;
    for sys in galaxy::systems() {
        let sp = to(sys.map_pos);
        let r = (sc * 0.05).clamp(5.0, 14.0);
        // Height stem down (or up) to the galactic plane.
        if v.is_3d() {
            let foot = to([sys.map_pos[0], 0.0, sys.map_pos[2]]);
            if foot.distance(sp) > 2.0 {
                p.line_segment([sp, foot], Stroke::new(1.0, DIM.gamma_multiply(0.45)));
                p.line_segment([foot - EVec2::new(3.0, 0.0), foot + EVec2::new(3.0, 0.0)], Stroke::new(1.0, DIM.gamma_multiply(0.6)));
            }
        }
        if !view::mapview::known(sys.id, &s.charted, s.system) {
            // Uncharted: a faint point, no name, tier or star colour until someone gets there.
            p.circle_stroke(sp, r * 0.7, Stroke::new(1.0, DIM));
            if hover.is_some_and(|h| h.distance(sp) < r + 8.0) {
                hovered = Some(sys.id);
            }
            if s.map.selected == Some(sys.id) {
                p.circle_stroke(sp, r + 7.0, Stroke::new(1.5, GOLD));
            }
            p.text(sp + EVec2::new(r + 6.0, -8.0), Align2::LEFT_TOP, view::mapview::UNKNOWN_SYSTEM, FontId::proportional(12.0), DIM);
            p.text(sp + EVec2::new(r + 6.0, 7.0), Align2::LEFT_TOP, coords::sector_of(sys.map_pos), FontId::proportional(10.0), DIM);
            continue;
        }
        let star = rgb(sys.star.color());
        p.circle_filled(sp, r * 2.2, star.gamma_multiply(0.12));
        p.circle_filled(sp, r, star);
        if hover.is_some_and(|h| h.distance(sp) < r + 8.0) {
            hovered = Some(sys.id);
        }
        if s.map.selected == Some(sys.id) {
            p.circle_stroke(sp, r + 7.0, Stroke::new(1.5, GOLD));
        }
        p.text(sp + EVec2::new(r + 6.0, -8.0), Align2::LEFT_TOP, sys.name, FontId::proportional(13.0), tier_color(sys.tier));
        let held = s.controller_tag(sys.id).map(|t| format!(" · [{t}] 지배")).unwrap_or_default();
        p.text(sp + EVec2::new(r + 6.0, 8.0), Align2::LEFT_TOP, format!("{} · {}{held}", coords::sector_of(sys.map_pos), sys.tier.label()), FontId::proportional(10.0), DIM);
        let others: Vec<String> = s.fleet_members.iter().filter(|m| m.system == sys.id && Some(&m.name) != s.profile.as_ref().map(|p| &p.name)).map(|m| m.name.clone()).collect();
        if !others.is_empty() {
            p.text(sp + EVec2::new(r + 6.0, 21.0), Align2::LEFT_TOP, format!("⚑ {}", others.join(", ")), FontId::proportional(10.0), GREEN);
        }
    }
    // "You are here": pulsing ring and marker.
    let pulse = ((now * 2.0).sin() * 0.5 + 0.5) as f32;
    p.circle_stroke(cp, 18.0 + pulse * 8.0, Stroke::new(2.0, alpha(GOLD, (220.0 - pulse * 150.0) as u8)));
    p.text(cp + EVec2::new(0.0, 26.0), Align2::CENTER_TOP, "▲ 현재 위치", FontId::proportional(12.0), GOLD);
    if let Some(id) = hovered {
        let sys = galaxy::system(id);
        let d = galaxy::distance_ly(s.system, id);
        let place = format!("섹터 {} · G({:+.2}, {:+.2}, {:+.2}) ly", coords::sector_of(sys.map_pos), sys.map_pos[0], sys.map_pos[1], sys.map_pos[2]);
        let dist = format!("거리 {d:.2} ly{}", if d <= range { " · 워프 가능" } else { "" });
        if view::mapview::known(id, &s.charted, s.system) {
            tooltip(p, hover.unwrap(), &[format!("{} ({})", sys.name, sys.tier.label()), place, dist, sys.lore.to_string()]);
        } else {
            tooltip(p, hover.unwrap(), &[view::mapview::UNKNOWN_SYSTEM.to_string(), place, dist, "항로국 기록 없음 — 처음 도달한 함장이 이름을 올린다.".into()]);
        }
        if resp.clicked() {
            s.map.selected = Some(id);
        }
        if resp.double_clicked() && id == s.system {
            s.map.follow_me();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn system_chart(_ui: &mut egui::Ui, p: &egui::Painter, map: Rect, resp: &egui::Response, scroll: f32, hover: Option<Pos2>, s: &mut Session, net: &Net, env: &Env, now: f64) {
    let t = now - INTERP_DELAY;
    let me_pose = s.me.and_then(|m| s.sample(m, t)).map(|(pos, rot, st)| (pos, rot, st.clone()));
    // Zooming out of the fully framed system pulls back to the galaxy chart.
    if resp.dragged_by(egui::PointerButton::Secondary) {
        s.map.rotate(resp.drag_delta());
    }
    let pan = resp.dragged_by(egui::PointerButton::Primary).then(|| resp.drag_delta());
    if s.map.system_input(map, pan, scroll, hover, system_extent(env.layout.as_ref()), s.system, me_pose.as_ref().map(|m| m.0)) {
        return;
    }
    let v = s.map.clone();
    let sc = v.s_scale;
    let to = |w: Vec3| v.system_to_screen(map, w);
    let from = |sp: Pos2| v.system_from_screen(map, sp);
    // Height stem from an object down (or up) to the reference plane, with a tick at its foot.
    let stem = |p: &egui::Painter, w: Vec3, col: Color32| {
        if !v.is_3d() {
            return;
        }
        let (top, foot) = (to(w), to(v.foot(w)));
        if top.distance(foot) < 2.0 {
            return;
        }
        p.line_segment([top, foot], Stroke::new(1.0, col.gamma_multiply(0.45)));
        p.line_segment([foot - EVec2::new(3.0, 0.0), foot + EVec2::new(3.0, 0.0)], Stroke::new(1.0, col.gamma_multiply(0.6)));
    };
    // A horizontal circle on the plane through `c` (orbits, rings), projected.
    let ring = |p: &egui::Painter, c: Vec3, r: f32, stroke: Stroke| {
        let pts: Vec<Pos2> = (0..=64).map(|i| {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            to(c + Vec3::new(a.cos() * r, 0.0, a.sin() * r))
        }).collect();
        p.add(egui::Shape::line(pts, stroke));
    };

    // Coordinate grid in km on the reference plane (labels on the flat chart only).
    let step = nice_step(sc, 90.0);
    let corners = [from(map.min), from(map.max), from(Pos2::new(map.min.x, map.max.y)), from(Pos2::new(map.max.x, map.min.y))];
    let (mut x0, mut x1, mut z0, mut z1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for c in corners {
        (x0, x1, z0, z1) = (x0.min(c.x), x1.max(c.x), z0.min(c.z), z1.max(c.z));
    }
    let h = v.s_height;
    let grid_stroke = |k: f32| Stroke::new(1.0, if (k / step).round() as i64 % 5 == 0 { GRID.gamma_multiply(1.6) } else { GRID });
    let mut x = (x0 / step).floor() * step;
    while x <= x1 {
        p.line_segment([to(Vec3::new(x, h, z0)), to(Vec3::new(x, h, z1))], grid_stroke(x));
        if !v.is_3d() {
            p.text(Pos2::new(to(Vec3::new(x, h, 0.0)).x + 3.0, map.bottom() - 16.0), Align2::LEFT_TOP, format!("{:+.1}", x / 1000.0), FontId::monospace(10.0), GRID_TEXT);
        }
        x += step;
    }
    let mut z = (z0 / step).floor() * step;
    while z <= z1 {
        p.line_segment([to(Vec3::new(x0, h, z)), to(Vec3::new(x1, h, z))], grid_stroke(z));
        if !v.is_3d() {
            p.text(Pos2::new(map.left() + 4.0, to(Vec3::new(0.0, h, z)).y + 2.0), Align2::LEFT_TOP, format!("{:+.1}", z / 1000.0), FontId::monospace(10.0), GRID_TEXT);
        }
        z += step;
    }
    if v.is_3d() {
        p.text(Pos2::new(map.left() + 24.0, map.bottom() - 84.0), Align2::LEFT_TOP, format!("기준면 고도 {:+.2} km · 격자 {}", h / 1000.0, if step >= 1000.0 { format!("{:.0} km", step / 1000.0) } else { format!("{step:.0} m") }), FontId::proportional(11.0), DIM);
    }
    // Scale bar and north arrow.
    let bar_px = step * sc;
    let b0 = Pos2::new(map.left() + 24.0, map.bottom() - 40.0);
    p.line_segment([b0, b0 + EVec2::new(bar_px, 0.0)], Stroke::new(2.0, DIM));
    p.text(b0 + EVec2::new(bar_px / 2.0, -4.0), Align2::CENTER_BOTTOM, if step >= 1000.0 { format!("{:.0} km", step / 1000.0) } else { format!("{step:.0} m") }, FontId::proportional(11.0), DIM);
    let n0 = Pos2::new(map.right() - 34.0, map.top() + 80.0);
    let nd = (to(Vec3::new(v.s_center.x, h, v.s_center.y - 1000.0)) - to(Vec3::new(v.s_center.x, h, v.s_center.y))).normalized();
    p.line_segment([n0 - nd * 16.0, n0 + nd * 16.0], Stroke::new(2.0, DIM));
    p.text(n0 + nd * 18.0, Align2::CENTER_CENTER, "N", FontId::proportional(11.0), DIM);

    let mut tips: Vec<(f32, Vec<String>, Option<EntityId>)> = vec![];
    let mut placed: Vec<Rect> = vec![];
    let mut label = |p: &egui::Painter, at: Pos2, text: &str, col: Color32, force: bool| {
        let r = Rect::from_min_size(at, EVec2::new(text.chars().count() as f32 * 7.0, 14.0));
        if force || !placed.iter().any(|q| q.intersects(r)) {
            p.text(at, Align2::LEFT_TOP, text, FontId::proportional(11.0), col);
            placed.push(r);
        }
    };
    let mut consider = |sp: Pos2, r: f32, lines: Vec<String>, id: Option<EntityId>| {
        if let Some(h) = hover {
            let d = h.distance(sp);
            if d < r.max(9.0) {
                tips.push((d, lines, id));
            }
        }
    };
    let my_pos = me_pose.as_ref().map(|m| m.0);
    let dist_txt = |w: Vec3| my_pos.map(|m| format!(" · {:.2} km", m.distance(w) / 1000.0)).unwrap_or_default();

    // Static bodies.
    if let Some(lay) = &env.layout {
        let def = galaxy::system(s.system);
        let sp = to(Vec3::ZERO);
        let r = (lay.star_radius * sc).max(7.0);
        let star = rgb(def.star.color());
        p.circle_filled(sp, r * 2.0, star.gamma_multiply(0.1));
        p.circle_filled(sp, r, star);
        consider(sp, r, vec![format!("항성 {}", def.name), "L(0, 0, 0) — 성계 기준점".into()], None);
        for (i, pl) in lay.planets.iter().enumerate() {
            let pp = to(pl.pos);
            // Orbit ring around the star (in the star's plane).
            ring(p, Vec3::ZERO, EVec2::new(pl.pos.x, pl.pos.z).length(), Stroke::new(1.0, map::ORBIT));
            stem(p, pl.pos, map::PLANET_RIM);
            let pr = (pl.radius * sc).max(4.0);
            p.circle_filled(pp, pr, map::PLANET);
            p.circle_stroke(pp, pr, Stroke::new(1.0, map::PLANET_RIM));
            consider(pp, pr, vec![format!("행성 {}-{}", def.name, (b'b' + i as u8) as char), loc(pl.pos) + &dist_txt(pl.pos)], None);
        }
        for a in &lay.asteroids {
            let ap = to(a.pos);
            if map.contains(ap) {
                let ar = (a.radius * sc).clamp(1.5, 18.0);
                p.circle_filled(ap, ar, map::ASTEROID);
                // Rare deposits stand out; every rock names its mineral on hover.
                if let Some(m) = shared::minerals::mineral(a.mineral) {
                    if m.rarity >= shared::minerals::Rarity::Rare {
                        let c = view::palette::rarity_color(m.rarity);
                        stem(p, a.pos, c);
                        p.circle_stroke(ap, ar + 3.0, Stroke::new(1.2, c));
                    }
                    consider(ap, ar.max(5.0), vec![m.label(), loc(a.pos) + &dist_txt(a.pos)], None);
                }
            }
        }
        stem(p, lay.station, GREEN);
        stem(p, lay.jump_point, ACCENT);
        let st = to(lay.station);
        let half = (90.0 * sc).max(6.0);
        p.rect_stroke(Rect::from_center_size(st, EVec2::splat(half * 2.0)), 2.0, Stroke::new(2.0, GREEN), egui::StrokeKind::Middle);
        label(p, st + EVec2::new(half + 5.0, -6.0), "정거장", GREEN, true);
        consider(st, half, vec!["정거장 (G: 도킹)".into(), loc(lay.station) + &dist_txt(lay.station)], None);
        let jp = to(lay.jump_point);
        p.circle_stroke(jp, 10.0, Stroke::new(2.0, ACCENT));
        p.circle_stroke(jp, 5.0, Stroke::new(1.0, ACCENT));
        label(p, jp + EVec2::new(13.0, -6.0), "워프 지점", ACCENT, true);
        consider(jp, 10.0, vec!["워프 지점".into(), loc(lay.jump_point) + &dist_txt(lay.jump_point)], None);
        for poi in &lay.pois {
            let pp = to(poi.pos);
            let scanned = s.scanned.contains(&poi.id);
            let col = if scanned { DIM } else { view::palette::POI };
            let icon = match poi.kind {
                PoiKind::Anomaly => "◇",
                PoiKind::Derelict => "▣",
                PoiKind::Ruin => "♜",
                PoiKind::Signal => "◉",
                PoiKind::AncientGate => "○",
            };
            stem(p, poi.pos, col);
            p.text(pp, Align2::CENTER_CENTER, icon, FontId::proportional(15.0), col);
            if sc > 0.02 {
                label(p, pp + EVec2::new(10.0, -6.0), &poi.name, col, false);
            }
            consider(pp, 10.0, vec![format!("{}{}", poi.name, if scanned { " (스캔 완료)" } else { "" }), loc(poi.pos) + &dist_txt(poi.pos)], None);
        }
    }

    // Ships, live.
    let prof = s.profile.clone();
    let def = galaxy::system(s.system);
    let viewer = Viewer { me: s.me, player: s.player_id, fleet: s.fleet, alliance: s.my_alliance, alliances: &s.alliances, tier: def.tier };
    let ids: Vec<EntityId> = s.latest().map(|l| l.ents.keys().copied().collect()).unwrap_or_default();
    let mut drawn = 0usize;
    for id in ids {
        let Some((pos, rot, st)) = s.sample(id, t).map(|(p, r, st)| (p, r, st.clone())) else { continue };
        if st.docked || st.kind == EntityKind::Missile {
            continue;
        }
        let sp = to(pos);
        if !map.expand(20.0).contains(sp) {
            continue;
        }
        drawn += 1;
        let mine = Some(id) == s.me;
        let col = viewer.relation(&st).color();
        let fwd = rot * Vec3::NEG_Z;
        stem(p, pos, col);
        // Heading as seen by the chart camera (straight at the camera: point up the screen).
        let dir = (to(pos + fwd * 100.0) - sp).normalized();
        let dir = if dir.length() > 0.5 { dir } else { EVec2::new(0.0, -1.0) };
        let size = if mine { 11.0 } else { (st.radius * sc).clamp(4.0, 9.0) + if matches!(st.class, Some(shared::ships::SizeClass::Capital)) { 3.0 } else { 0.0 } };
        if st.kind == EntityKind::Wreck {
            p.text(sp, Align2::CENTER_CENTER, "✕", FontId::proportional(12.0), col);
        } else {
            // Heading chevron.
            let side = EVec2::new(-dir.y, dir.x);
            let tip = sp + dir * size;
            let pts = vec![tip, sp - dir * size * 0.6 + side * size * 0.6, sp - dir * size * 0.25, sp - dir * size * 0.6 - side * size * 0.6];
            p.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
            if s.target == Some(id) {
                p.circle_stroke(sp, size + 6.0, Stroke::new(1.5, RED));
            }
        }
        if mine {
            // Velocity vector (5 s ahead), weapon reach and a pulsing "you are here".
            let v = Vec3::from(st.vel);
            p.line_segment([sp, to(pos + v * 5.0)], Stroke::new(1.5, GOLD));
            if let Some(pr) = &prof {
                let stats = pr.ship.stats();
                p.circle_stroke(sp, stats.weapon_range * sc, Stroke::new(1.0, alpha(map::WEAPON_REACH, 70)));
                if stats.turret_range > 0.0 && stats.turrets > 0 {
                    p.circle_stroke(sp, stats.turret_range * sc, Stroke::new(1.0, alpha(map::TURRET_REACH, 50)));
                }
                p.circle_stroke(sp, stats.scan_range * sc, Stroke::new(1.0, alpha(ACCENT, 40)));
            }
            let pulse = ((now * 2.5).sin() * 0.5 + 0.5) as f32;
            p.circle_stroke(sp, 16.0 + pulse * 6.0, Stroke::new(1.5, alpha(GOLD, (200.0 - pulse * 140.0) as u8)));
        }
        let near = my_pos.is_some_and(|m| m.distance(pos) < 2500.0);
        if mine || s.target == Some(id) || (sc > 0.05 && st.kind != EntityKind::Wreck) || (near && drawn < 60) {
            label(p, sp + EVec2::new(size + 4.0, -6.0), &st.name, col, mine);
        }
        let (b, el) = coords::bearing(fwd);
        let above = my_pos.filter(|_| !mine).map(|m| {
            let dy = pos.y - m.y;
            format!(" · {} {:.2} km", if dy >= 0.0 { "▲" } else { "▼" }, dy.abs() / 1000.0)
        });
        let mut lines = vec![
            format!("{}{}", st.name, if st.wanted { " [수배]" } else { "" }),
            loc(pos) + &dist_txt(pos) + &above.unwrap_or_default(),
            format!("방위 {b:03.0}° · 고도각 {el:+.0}° · 속도 {:.0} m/s", Vec3::from(st.vel).length()),
        ];
        if st.kind != EntityKind::Wreck {
            lines.push(format!("실드 {:.0}% · 선체 {:.0}%", st.shield * 100.0, st.hull * 100.0));
        }
        consider(sp, size + 4.0, lines, (st.kind != EntityKind::Wreck && !mine).then_some(id));
    }

    // Cursor readout + nearest-object tooltip; click a ship to target it.
    if let Some(h) = hover {
        let w = from(h);
        p.text(h + EVec2::new(14.0, 14.0), Align2::LEFT_TOP, format!("({:+.2}, {:+.2}) km", w.x / 1000.0, w.z / 1000.0), FontId::monospace(10.0), DIM);
    }
    tips.sort_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, lines, id)) = tips.into_iter().next() {
        tooltip(p, hover.unwrap(), &lines);
        if resp.clicked()
            && let Some(id) = id
        {
            s.target = Some(id);
            net.send(ClientMsg::SetTarget(Some(id)));
        }
    }
}

fn loc(w: Vec3) -> String {
    format!("L({:+.2}, {:+.2}, {:+.2}) km", w.x / 1000.0, w.y / 1000.0, w.z / 1000.0)
}

fn tooltip(p: &egui::Painter, at: Pos2, lines: &[String]) {
    let font = FontId::proportional(12.0);
    let galleys: Vec<_> = lines.iter().enumerate().map(|(i, l)| p.layout_no_wrap(l.clone(), font.clone(), if i == 0 { Color32::WHITE } else { DIM })).collect();
    let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max) + 16.0;
    let h = galleys.iter().map(|g| g.size().y + 2.0).sum::<f32>() + 12.0;
    let mut o = at + EVec2::new(16.0, -h - 8.0);
    if o.y < 0.0 {
        o.y = at.y + 20.0;
    }
    let r = Rect::from_min_size(o, EVec2::new(w, h));
    p.rect_filled(r, 4.0, map::TOOLTIP);
    p.rect_stroke(r, 4.0, Stroke::new(1.0, map::TOOLTIP_BORDER), egui::StrokeKind::Inside);
    let mut y = o.y + 6.0;
    for g in galleys {
        let gh = g.size().y;
        p.galley(Pos2::new(o.x + 8.0, y), g, Color32::WHITE);
        y += gh + 2.0;
    }
}

fn side_panel(ui: &mut egui::Ui, rect: Rect, s: &mut Session, net: &Net, now: f64) {
    ui.painter().rect_filled(rect, 0.0, map::SIDEBAR);
    ui.painter().line_segment([rect.left_top(), rect.left_bottom()], Stroke::new(1.0, map::DIVIDER));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink(14.0)), |ui| {
        ui.label(RichText::new("현재 위치").size(16.0).strong().color(GOLD));
        let sys = galaxy::system(s.system);
        ui.label(RichText::new(format!("{} 성계 · {}", sys.name, sys.tier.label())).color(tier_color(sys.tier)));
        ui.label(RichText::new(format!("섹터 {}", coords::sector_of(sys.map_pos))).monospace());
        ui.label(RichText::new(format!("G({:+.2}, {:+.2}, {:+.2}) ly", sys.map_pos[0], sys.map_pos[1], sys.map_pos[2])).monospace());
        let me = s.me.and_then(|m| s.sample(m, now - INTERP_DELAY)).map(|(p, r, st)| (p, r, st.clone()));
        if let Some((pos, rot, st)) = &me {
            ui.label(RichText::new(loc(*pos)).monospace().color(Color32::WHITE));
            let (b, el) = coords::bearing(*rot * Vec3::NEG_Z);
            ui.label(RichText::new(format!("방위 {b:03.0}° · 고도각 {el:+.0}°")).monospace());
            ui.label(RichText::new(format!("속도 {:.0} m/s · 항성까지 {:.2} km", Vec3::from(st.vel).length(), pos.length() / 1000.0)).size(12.0).color(DIM));
            if let Some(t) = s.target.and_then(|t| s.sample(t, now - INTERP_DELAY)).map(|(tp, _, ts)| (tp, ts.name.clone())) {
                let (tb, tel) = coords::bearing(t.0 - *pos);
                ui.label(RichText::new(format!("목표 {} · {:.2} km · 방위 {tb:03.0}° {tel:+.0}°", t.1, t.0.distance(*pos) / 1000.0)).size(12.0).color(RED));
            }
        } else {
            ui.label(RichText::new("함선 없음 (도킹/재건 대기)").color(DIM));
        }
        ui.separator();
        ui.label(RichText::new("범례").strong());
        for (c, t) in [(GOLD, "▲ 내 함선 (속도 벡터·사거리 원)"), (GREEN, "▲ 아군 (함대·동맹)"), (RED, "▲ 적대 (해적·교전 대상)"), (ACCENT, "▲ 중립 함장"), (GOLD, "✕ 잔해")] {
            ui.label(RichText::new(t).size(12.0).color(c));
        }
        ui.separator();
        let range = s.profile.as_ref().map(|p| p.ship.stats().warp_range).unwrap_or(0.0);
        ui.label(RichText::new(format!("워프 (사거리 {range:.2} ly)")).strong());
        egui::ScrollArea::vertical().max_height(rect.height() * 0.45).show(ui, |ui| {
            // Nearest first: with fifty systems the ones in range are what matters.
            let mut near: Vec<(&galaxy::SystemDef, f32)> = galaxy::systems().iter().filter(|o| o.id != s.system).map(|o| (o, galaxy::distance_ly(s.system, o.id))).collect();
            near.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.id.cmp(&b.0.id)));
            for (sys, d) in near {
                ui.horizontal(|ui| {
                    let sel = s.map.selected == Some(sys.id);
                    let name = if view::mapview::known(sys.id, &s.charted, s.system) {
                        RichText::new(format!("{}{}", if sel { "▶ " } else { "" }, sys.name)).color(tier_color(sys.tier))
                    } else {
                        RichText::new(format!("{}{}", if sel { "▶ " } else { "" }, view::mapview::UNKNOWN_SYSTEM)).color(DIM)
                    };
                    if ui.add(egui::Label::new(name).sense(egui::Sense::click())).clicked() {
                        s.map.show_system(sys.id);
                    }
                    ui.label(RichText::new(format!("{} · {d:.2} ly", coords::sector_of(sys.map_pos))).size(11.0).color(DIM));
                    if ui.add_enabled(d <= range && s.warp.is_none(), egui::Button::new("워프")).clicked() {
                        net.send(ClientMsg::Undock);
                        net.send(ClientMsg::Warp { system: sys.id });
                        s.ui.map = false;
                    }
                });
            }
        });
    });
}
