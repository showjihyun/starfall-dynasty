//! egui UI: login, character/ship creation, HUD and the game windows.

use crate::net::Net;
use crate::scene::Env;
use crate::ships::{LIVERIES, ShipVis, Visuals};
use crate::state::{Args, Screen, Session, load_token};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2 as EVec2};
use shared::chronicle::{Authenticity, Importance};
use shared::galaxy::{self, Ore, PoiKind, Species};
use shared::missions::{Objective, scenarios};
use shared::protocol::*;
use shared::ships::{HullClass, MAX_MODULE_LEVEL, Module, Ship, tier_cap, upgrade_cost};

#[derive(Resource)]
pub struct Form {
    pub name: String,
    pub species: Species,
    pub profession: Profession,
    pub hull: HullClass,
    pub ship_name: String,
    pub livery: u8,
    /// The captain picked a paint by hand; until then it follows the species' default.
    pub livery_picked: bool,
    pub fonts_ready: bool,
    pub sent_hello: bool,
}

impl Default for Form {
    fn default() -> Self {
        Form { name: String::new(), species: Species::Human, profession: Profession::Explorer, hull: HullClass::Talon, ship_name: String::new(), livery: view::palette::species_livery(Species::Human), livery_picked: false, fonts_ready: false, sent_hello: false }
    }
}

use view::hud::{bar, crosshair_gauges};
use view::palette::{ACCENT, DIM, GOLD, GREEN, RED, alpha, chrome};
use view::relation::Viewer;

pub use view::palette::tier_color;

fn setup_fonts(ctx: &egui::Context) {
    ctx.set_fonts(view::fonts::with_korean().0);
    let mut style = (*ctx.global_style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.window_fill = chrome::WINDOW;
    style.visuals.panel_fill = chrome::PANEL;
    style.visuals.window_stroke = Stroke::new(1.0, chrome::BORDER);
    style.visuals.selection.bg_fill = chrome::SELECTION;
    style.spacing.item_spacing = EVec2::new(8.0, 6.0);
    ctx.set_global_style(style);
}

#[allow(clippy::too_many_arguments)]
pub fn ui_system(
    mut contexts: EguiContexts,
    mut form: ResMut<Form>,
    mut s: ResMut<Session>,
    screen: Res<State<Screen>>,
    net: Option<Res<Net>>,
    args: Res<Args>,
    env: Res<Env>,
    vis: Res<Visuals>,
    ships: Query<(&ShipVis, &Transform)>,
    cam: Query<(&Camera, &GlobalTransform), With<crate::control::MainCamera>>,
    time: Res<Time<Real>>,
) -> Result {
    let _prof = shared::prof::span("ui::ui_system");
    let ctx = contexts.ctx_mut()?;
    if !form.fonts_ready {
        setup_fonts(ctx);
        form.fonts_ready = true;
        if let Some(n) = &args.name {
            form.name = n.clone();
        }
        if let Some(h) = args.hull {
            form.hull = h;
        }
    }
    let now = time.elapsed_secs_f64();
    // Harness layer toggle: STARFALL_HIDE_UI=1 leaves only the 3D scene in screenshots.
    if *screen.get() == Screen::Playing && std::env::var_os("STARFALL_HIDE_UI").is_some() {
        return Ok(());
    }
    let Some(net) = net else { return Ok(()) };
    match screen.get() {
        Screen::Connecting => connecting(ctx, &s),
        Screen::Login => login(ctx, &mut form, &mut s, &net, &args),
        Screen::Create => create(ctx, &mut form, &s, &net, &args),
        Screen::Playing => {
            hud(ctx, &mut s, &net, &env, &vis, &ships, &cam, now);
            windows(ctx, &mut s, &net, &env);
            crate::starmap::draw(ctx, &mut s, &net, &env, now);
        }
    }
    Ok(())
}

fn full_screen(ctx: &egui::Context, fill: Color32, f: impl FnOnce(&mut egui::Ui)) {
    let rect = ctx.content_rect();
    egui::Area::new("full".into()).fixed_pos(rect.min).order(egui::Order::Background).show(ctx, |ui| {
        ui.painter().rect_filled(rect, 0.0, fill);
        ui.set_min_size(rect.size());
        ui.set_max_size(rect.size());
        egui::Frame::NONE.inner_margin(24.0).show(ui, f);
    });
}

fn title(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("STARFALL DYNASTY").size(44.0).color(ACCENT).strong());
        ui.label(RichText::new("THE GALAXY IS NOT A MAP. IT IS A HISTORY.").size(14.0).color(DIM));
        ui.add_space(12.0);
    });
}

fn connecting(ctx: &egui::Context, s: &Session) {
    full_screen(ctx, Color32::TRANSPARENT, |ui| {
        ui.add_space(ui.available_height() * 0.3);
        title(ui);
        ui.vertical_centered(|ui| match &s.disconnected {
            Some(r) => {
                ui.label(RichText::new(format!("연결 실패: {r}")).color(RED));
                ui.label(RichText::new("서버를 먼저 실행하세요:  cargo run -p server --release").color(DIM));
            }
            None => {
                ui.spinner();
                ui.label(format!("{} 연결 중…", s.server_url));
            }
        });
    });
}

fn login(ctx: &egui::Context, form: &mut Form, s: &mut Session, net: &Net, args: &Args) {
    if args.auto && !form.sent_hello && !form.name.is_empty() {
        send_hello(form, s, net, args.token.clone());
    }
    full_screen(ctx, Color32::TRANSPARENT, |ui| {
        ui.add_space(ui.available_height() * 0.25);
        title(ui);
        ui.vertical_centered(|ui| {
            ui.set_max_width(360.0);
            ui.label("함장 이름");
            let r = ui.add(egui::TextEdit::singleline(&mut form.name).hint_text("2~20자").desired_width(320.0));
            let go = ui.add_enabled(form.name.trim().chars().count() >= 2, egui::Button::new(RichText::new("접속").size(18.0)).min_size(EVec2::new(160.0, 36.0))).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
            if go {
                send_hello(form, s, net, args.token.clone());
            }
            if let Some((e, _)) = &s.last_error {
                ui.label(RichText::new(e).color(RED));
            }
            ui.add_space(8.0);
            ui.label(RichText::new(format!("서버: {}", s.server_url)).color(DIM).size(12.0));
        });
    });
}

fn send_hello(form: &mut Form, s: &mut Session, net: &Net, token: Option<String>) {
    let name = form.name.trim().to_string();
    s.name = name.clone();
    s.last_error = None;
    net.send(ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.clone(), token: token.or_else(|| load_token(&name)) });
    form.sent_hello = true;
    if form.ship_name.is_empty() {
        form.ship_name = format!("SNS {}", name);
    }
}

fn stat_bar(ui: &mut egui::Ui, label: &str, v: f32, max: f32) {
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 14.0], egui::Label::new(RichText::new(label).size(12.0).color(DIM)));
        ui.add(egui::ProgressBar::new((v / max).clamp(0.0, 1.0)).desired_width(170.0).text(RichText::new(format!("{v:.0}")).size(11.0)));
    });
}

fn create(ctx: &egui::Context, form: &mut Form, s: &Session, net: &Net, args: &Args) {
    let send = |form: &Form| {
        net.send(ClientMsg::CreateCharacter { species: form.species, profession: form.profession, hull: form.hull, ship_name: form.ship_name.trim().to_string(), livery: form.livery });
    };
    if args.auto && !args.no_create && form.sent_hello {
        form.sent_hello = false;
        if form.ship_name.is_empty() {
            form.ship_name = format!("SNS {}", form.name);
        }
        send(form);
    }
    full_screen(ctx, chrome::SCRIM, |ui| {
        title(ui);
        ui.columns(3, |cols| {
            cols[0].heading("1. 종족");
            for sp in Species::ALL {
                cols[0].radio_value(&mut form.species, sp, RichText::new(sp.name()).size(16.0));
                cols[0].label(RichText::new(sp.motto()).size(12.0).color(DIM));
            }
            cols[0].add_space(10.0);
            cols[0].heading("2. 직업");
            for p in Profession::ALL {
                cols[0].radio_value(&mut form.profession, p, p.name());
                cols[0].label(RichText::new(p.perk()).size(11.0).color(DIM));
            }

            cols[1].heading("3. 평생의 함선 — 단 한 척");
            cols[1].label(RichText::new("함선은 교체할 수 없습니다. 일일 미션·교전·시나리오로 이 한 척을 Leviathan급까지 키워야 합니다.").color(GOLD).size(12.0));
            for h in HullClass::ALL {
                let sel = form.hull == h;
                let r = cols[1].selectable_label(sel, RichText::new(h.name()).size(16.0).color(if sel { ACCENT } else { Color32::WHITE }));
                if r.clicked() {
                    form.hull = h;
                }
                cols[1].label(RichText::new(h.blurb()).size(11.0).color(DIM));
            }

            cols[2].heading("함선 제원");
            let st = Ship::new(String::new(), form.hull, form.livery).stats();
            stat_bar(&mut cols[2], "선체", st.max_hull, 700.0);
            stat_bar(&mut cols[2], "실드", st.max_shield, 400.0);
            stat_bar(&mut cols[2], "속도", st.speed, 150.0);
            stat_bar(&mut cols[2], "선회", st.turn * 50.0, 150.0);
            stat_bar(&mut cols[2], "화력/s", st.damage / st.fire_interval, 90.0);
            stat_bar(&mut cols[2], "워프 ly", st.warp_range * 40.0, 100.0);
            stat_bar(&mut cols[2], "스캐너", st.scan_range, 1500.0);
            stat_bar(&mut cols[2], "화물", st.cargo, 100.0);
            cols[2].add_space(8.0);
            cols[2].label("함선 이름");
            cols[2].add(egui::TextEdit::singleline(&mut form.ship_name).desired_width(240.0));
            if !form.livery_picked {
                form.livery = view::palette::species_livery(form.species);
            }
            let native = view::palette::species_livery(form.species);
            cols[2].label(format!("도색 — 점은 {} 기본색", form.species.name()));
            cols[2].horizontal(|ui| {
                for (i, c) in LIVERIES.iter().enumerate().take(view::palette::PLAYER_LIVERIES) {
                    let col = Color32::from_rgb((c[0] * 255.0) as u8, (c[1] * 255.0) as u8, (c[2] * 255.0) as u8);
                    let (rect, r) = ui.allocate_exact_size(EVec2::splat(24.0), egui::Sense::click());
                    ui.painter().rect_filled(rect, 4.0, col);
                    if form.livery == i as u8 {
                        ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Outside);
                    }
                    if native == i as u8 {
                        ui.painter().circle_filled(rect.center(), 3.0, chrome::DOT);
                    }
                    if r.clicked() {
                        form.livery = i as u8;
                        form.livery_picked = true;
                    }
                }
            });
            cols[2].add_space(12.0);
            let ok = form.ship_name.trim().chars().count() >= 2;
            if cols[2].add_enabled(ok, egui::Button::new(RichText::new("진수 — 역사를 시작한다").size(18.0)).min_size(EVec2::new(240.0, 40.0))).clicked() {
                send(form);
            }
            if let Some((e, _)) = &s.last_error {
                cols[2].label(RichText::new(e).color(RED));
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn hud(ctx: &egui::Context, s: &mut Session, net: &Net, env: &Env, vis: &Visuals, ships: &Query<(&ShipVis, &Transform)>, cam: &Query<(&Camera, &GlobalTransform), With<crate::control::MainCamera>>, now: f64) {
    let Some(prof) = s.profile.clone() else { return };
    let def = galaxy::system(s.system);
    let me = s.my_state().cloned();
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("hud3d")));

    // World-space labels and target brackets.
    if let Ok((camera, cgt)) = cam.single() {
        let project = |p: Vec3| camera.world_to_viewport(cgt, p).ok().map(|v| Pos2::new(v.x, v.y));
        let my_pos = me.as_ref().map(|m| Vec3::from(m.pos)).unwrap_or(cgt.translation());
        let (cam_pos, cam_rot) = (cgt.translation(), cgt.compute_transform().rotation);
        let viewer = Viewer { me: s.me, player: s.player_id, fleet: s.fleet, alliance: s.my_alliance, alliances: &s.alliances, tier: def.tier };
        if let Some(latest) = s.latest() {
            for e in latest.ents.values() {
                if Some(e.id) == s.me || e.docked || e.kind == EntityKind::Missile {
                    continue;
                }
                let Some(vt) = vis.map.get(&e.id).and_then(|x| ships.get(*x).ok()).map(|(_, t)| t.translation) else { continue };
                let d = vt.distance(my_pos);
                if d > 9000.0 {
                    continue;
                }
                let col = viewer.relation(e).color();
                let is_target = s.target == Some(e.id);
                let projected = project(vt).filter(|_| (cam_rot.inverse() * (vt - cam_pos)).z < 0.0);
                // Off screen or behind: an arrow on the screen edge for the target and nearby hostiles.
                if let Some((at, dir)) = view::hud::offscreen_marker(screen, cam_rot.inverse() * (vt - cam_pos), projected, 46.0) {
                    let hostile = matches!(viewer.relation(e), view::relation::Relation::Hostile);
                    if is_target || (hostile && d < 3000.0) {
                        let side = EVec2::new(-dir.y, dir.x);
                        let size = if is_target { 13.0 } else { 9.0 };
                        let pts = vec![at + dir * size, at - dir * size * 0.5 + side * size * 0.7, at - dir * size * 0.5 - side * size * 0.7];
                        painter.add(egui::Shape::convex_polygon(pts, col, Stroke::NONE));
                        if is_target {
                            painter.text(at - dir * 22.0, Align2::CENTER_CENTER, format!("{:.0}u  {}", d, view::hud::elevation_text(my_pos, vt)), FontId::proportional(11.0), col);
                        }
                    }
                    continue;
                }
                let Some(sp) = projected else { continue };
                let r = (e.radius * 900.0 / d.max(1.0)).clamp(10.0, 80.0);
                let st = Stroke::new(if is_target { 2.0 } else { 1.0 }, col);
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let c = sp + EVec2::new(sx * r, sy * r);
                    painter.line_segment([c, c - EVec2::new(sx * r * 0.35, 0.0)], st);
                    painter.line_segment([c, c - EVec2::new(0.0, sy * r * 0.35)], st);
                }
                let label = if e.wanted { format!("☠ {} [수배]", e.name) } else { e.name.clone() };
                painter.text(sp + EVec2::new(0.0, r + 4.0), Align2::CENTER_TOP, format!("{label}  {:.0}u", d), FontId::proportional(11.0), col);
                if is_target {
                    painter.text(sp + EVec2::new(0.0, r + 18.0), Align2::CENTER_TOP, view::hud::elevation_text(my_pos, vt), FontId::proportional(11.0), col);
                }
                if e.kind != EntityKind::Wreck && (is_target || d < 1500.0) {
                    let w = r * 1.6;
                    let base = sp + EVec2::new(-w / 2.0, -r - 9.0);
                    painter.rect_filled(egui::Rect::from_min_size(base, EVec2::new(w, 3.0)), 0.0, chrome::MINI_TRACK);
                    painter.rect_filled(egui::Rect::from_min_size(base, EVec2::new(w * e.shield, 3.0)), 0.0, view::palette::SHIELD);
                    painter.rect_filled(egui::Rect::from_min_size(base + EVec2::new(0.0, 4.0), EVec2::new(w, 3.0)), 0.0, chrome::MINI_TRACK);
                    painter.rect_filled(egui::Rect::from_min_size(base + EVec2::new(0.0, 4.0), EVec2::new(w * e.hull, 3.0)), 0.0, view::palette::hull_color(e.hull, 1.0));
                }
            }
        }
        if let Some(layout) = &env.layout {
            let mut marks: Vec<(Vec3, String, Color32)> = layout.pois.iter().map(|p| (p.pos, format!("{} {}", poi_icon(p.kind), p.name), if s.scanned.contains(&p.id) { DIM } else { view::palette::POI })).collect();
            marks.push((layout.station, "■ 정거장 (G: 도킹)".into(), GREEN));
            marks.push((layout.jump_point, "◎ 워프 지점".into(), ACCENT));
            for (pos, name, col) in marks {
                if let Some(sp) = project(pos) {
                    let d = pos.distance(my_pos);
                    painter.circle_stroke(sp, 6.0, Stroke::new(1.0, col));
                    painter.text(sp + EVec2::new(9.0, -6.0), Align2::LEFT_TOP, format!("{name}  {:.0}u", d), FontId::proportional(11.0), col);
                }
            }
        }
    }

    // Gauges: lagging "just lost" trail, red edge flash on hull damage.
    if let Some(m) = &me {
        s.gauge.update(m.shield, m.hull, now);
        if s.gauge.flash > 0.0 {
            let a = (s.gauge.flash * 110.0) as u8;
            for k in 0..4 {
                let w = 26.0 - k as f32 * 6.0;
                let col = alpha(chrome::WOUND, a / (k + 1));
                painter.rect_stroke(screen.shrink(w / 2.0 + k as f32 * 6.0), 0.0, Stroke::new(w, col), egui::StrokeKind::Middle);
            }
        }
        crosshair_gauges(&painter, screen.center(), m.shield, m.hull, s.gauge.lag, now);
    }

    // Crosshair.
    let c = screen.center();
    let ch = chrome::CROSSHAIR;
    painter.circle_stroke(c, 14.0, Stroke::new(1.0, ch));
    for d in [EVec2::new(1.0, 0.0), EVec2::new(-1.0, 0.0), EVec2::new(0.0, 1.0), EVec2::new(0.0, -1.0)] {
        painter.line_segment([c + d * 18.0, c + d * 28.0], Stroke::new(1.5, ch));
    }

    // Top-left: location and wallet.
    egui::Area::new("loc".into()).anchor(Align2::LEFT_TOP, EVec2::new(14.0, 12.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(def.name).size(22.0).strong());
                ui.label(RichText::new(format!("{:.0} fps · {:.0} ms", s.fps, s.ping_ms)).size(10.0).color(DIM));
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new(def.tier.label()).color(tier_color(def.tier)));
                if let Some(tag) = s.controller_tag(s.system) {
                    let mine = s.alliances.iter().any(|a| Some(a.id) == s.my_alliance && a.tag == tag);
                    ui.label(RichText::new(format!("· 지배 [{tag}]{}", if mine { " (우리 동맹 +10%)" } else { "" })).color(if mine { GREEN } else { GOLD }));
                }
            });
            ui.label(RichText::new(def.lore).size(11.0).color(DIM));
            ui.separator();
            ui.label(RichText::new(format!("{}  ·  {}", prof.name, prof.species.name())).color(GOLD));
            let w = &prof.wallet;
            ui.label(format!("◆ {} CR", w.credits));
            ui.label(RichText::new(format!("합금 {} · 결정 {} · 데이터코어 {} · 유물 {}", w.alloy, w.crystal, w.datacore, w.relic)).size(12.0));
            ui.label(RichText::new(format!("평판 {} · 격파 {} · 피격침 {}{}", prof.reputation, prof.kills, prof.deaths, if prof.bounty > 0 { format!(" · 현상금 {} CR", prof.bounty) } else { String::new() })).size(11.0).color(if prof.bounty > 0 { RED } else { DIM }));
        });
    });

    // Bottom-center: ship status.
    egui::Area::new("ship".into()).anchor(Align2::CENTER_BOTTOM, EVec2::new(0.0, -36.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| view::hud::ship_status(ui, &prof.ship, prof.cargo_used(), me.as_ref(), s.gauge.lag));
    });

    // Right: target and objective tracker.
    egui::Area::new("track".into()).anchor(Align2::RIGHT_TOP, EVec2::new(-14.0, 12.0)).show(ctx, |ui| {
        ui.set_max_width(320.0);
        if let Some(t) = s.target.and_then(|t| s.latest()?.ents.get(&t).cloned()) {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new("목표").color(DIM).size(11.0));
                ui.label(RichText::new(&t.name).color(RED).strong());
                bar(ui, t.shield, view::palette::SHIELD, "실드".into(), 280.0);
                bar(ui, t.hull, view::palette::hull_color(t.hull, 1.0), "선체".into(), 280.0);
                if let Some(m) = &me {
                    ui.label(format!("거리 {:.0}u", Vec3::from(t.pos).distance(Vec3::from(m.pos))));
                    let total = shared::ships::turret_batteries(&t);
                    if let Some(line) = view::hud::battery_text(shared::ships::batteries_reaching(&t, Vec3::from(m.pos)), total) {
                        ui.label(RichText::new(line).size(12.0).color(DIM));
                    }
                    if let Some(line) = view::hud::roll_hint(shared::ships::batteries_reaching(m, Vec3::from(t.pos)), shared::ships::turret_batteries(m)) {
                        ui.label(RichText::new(line).size(12.0).color(view::palette::CAUTION));
                    }
                    let viewer = Viewer { me: s.me, player: s.player_id, fleet: s.fleet, alliance: s.my_alliance, alliances: &s.alliances, tier: galaxy::system(s.system).tier };
                    let ents = s.latest().map(|l| l.ents.values()).into_iter().flatten();
                    if let Some(line) = view::hud::stray_fire_warning(&viewer, m, &t, shared::ships::turret_batteries(m), prof.ship.stats().turret_range, ents) {
                        ui.label(RichText::new(line).size(12.0).color(view::palette::CAUTION));
                    }
                }
            });
        }
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            if let Some(sp) = &prof.scenario {
                let sc = &scenarios()[sp.id as usize];
                ui.label(RichText::new(sc.title).color(GOLD).strong());
                if let Some(step) = sc.steps.get(sp.step) {
                    ui.label(format!("▶ {}", step.describe()));
                }
                ui.separator();
            }
            ui.label(RichText::new("일일 미션 (J)").color(DIM).size(11.0));
            for m in &prof.dailies {
                let done = m.complete();
                let col = if m.claimed { DIM } else if done { GREEN } else { Color32::WHITE };
                ui.label(RichText::new(format!("{} {} ({}/{})", if m.claimed { "✔" } else if done { "★" } else { "·" }, m.objective.describe(), m.progress, m.objective.target())).size(12.0).color(col));
            }
        });
    });

    // Top-center: toasts and progress bars.
    egui::Area::new("toasts".into()).anchor(Align2::CENTER_TOP, EVec2::new(0.0, 14.0)).show(ctx, |ui| {
        ui.set_max_width(560.0);
        if let Some(sides) = &s.engagement {
            egui::Frame::popup(ui.style()).fill(chrome::DANGER).show(ui, |ui| {
                ui.label(RichText::new(format!("⚔ 함대 교전 중 — {}", sides.join("  vs  "))).color(Color32::WHITE).strong());
            });
        }
        if let Some((sys, end, total)) = s.warp {
            let left = (end - now).max(0.0) as f32;
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(format!("워프 드라이브 충전 → {}", galaxy::system(sys).name)).color(ACCENT));
                ui.add(egui::ProgressBar::new(1.0 - left / total).desired_width(400.0));
            });
        }
        if let Some((_, f)) = s.scan {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label("스캔 중…");
                ui.add(egui::ProgressBar::new(f).desired_width(400.0));
            });
        }
        for (t, at) in s.toasts.iter().rev() {
            let age = now - at;
            if age < 6.0 {
                let a = ((6.0 - age).min(1.0) * 255.0) as u8;
                ui.label(RichText::new(t).color(alpha(chrome::PAPER, a)).background_color(alpha(chrome::INK, a / 2)));
            }
        }
    });

    // Bottom-left: chat.
    egui::Area::new("chat".into()).anchor(Align2::LEFT_BOTTOM, EVec2::new(14.0, -14.0)).show(ctx, |ui| {
        ui.set_max_width(380.0);
        for (from, text) in s.chat.iter().rev().take(6).collect::<Vec<_>>().into_iter().rev() {
            ui.label(RichText::new(format!("{from}: {text}")).size(12.0).background_color(alpha(chrome::INK, 140)));
        }
        let r = ui.add(egui::TextEdit::singleline(&mut s.ui.chat_input).hint_text("채팅 (Enter)").desired_width(360.0));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !s.ui.chat_input.trim().is_empty() {
            net.send(ClientMsg::Chat { text: std::mem::take(&mut s.ui.chat_input) });
        }
    });

    // Key hints.
    egui::Area::new("keys".into()).anchor(Align2::CENTER_BOTTOM, EVec2::new(0.0, -6.0)).show(ctx, |ui| {
        ui.label(RichText::new("W/S 추진 · A/D 선회 · R/F 상하 · Q/E 롤 · Z/Ctrl 상승·하강 · Alt+A/D 좌우 평행이동 · 우클릭 조종 · Shift 부스트 · Space 사격/주포 · T 목표 · X 미사일 · B 호위공격 · Y 호위복귀 · C 1/3인칭 · I 함교 화면 · G 도킹 · N 채굴 · V 스캔 · M 지도 · U 정거장 · J 미션 · O 계약 · H 연대기 · K 함대 · L 동맹 · F1").size(11.0).color(DIM));
    });

    if let Some(killer) = s.dead.clone() {
        egui::Area::new("dead".into()).anchor(Align2::CENTER_CENTER, EVec2::ZERO).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(chrome::DEATH).show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("함선 격침").size(30.0).color(RED).strong());
                    ui.label(format!("{killer}에게 격침당했습니다. 이 사건은 은하 연대기에 기록됩니다."));
                    ui.label(RichText::new("잔해는 며칠간 남아, 누구든 조사해 진실을 밝힐 수 있습니다.").color(DIM));
                    if ui.button(RichText::new("모항에서 재건 (Enter)").size(18.0)).clicked() {
                        net.send(ClientMsg::Respawn);
                    }
                });
            });
        });
    }
    if let Some(r) = &s.disconnected {
        egui::Area::new("dc".into()).anchor(Align2::CENTER_TOP, EVec2::new(0.0, 120.0)).show(ctx, |ui| {
            ui.label(RichText::new(format!("⚠ 서버 연결 끊김: {r}")).color(RED).size(18.0));
        });
    }
}

fn poi_icon(k: PoiKind) -> &'static str {
    match k {
        PoiKind::Anomaly => "◇",
        PoiKind::Ruin => "△",
        PoiKind::Derelict => "□",
        PoiKind::Signal => "◎",
        PoiKind::AncientGate => "○",
    }
}

fn windows(ctx: &egui::Context, s: &mut Session, net: &Net, env: &Env) {
    let Some(prof) = s.profile.clone() else { return };
    let mut open = s.ui.station;
    egui::Window::new("정거장 · 조선소 (U)").open(&mut open).default_size([560.0, 600.0]).show(ctx, |ui| station(ui, s, net, &prof, env));
    s.ui.station = open;

    let mut open = s.ui.missions;
    egui::Window::new("임무 (J)").open(&mut open).default_size([520.0, 560.0]).show(ctx, |ui| missions(ui, net, &prof));
    s.ui.missions = open;

    let mut open = s.ui.contracts;
    egui::Window::new("계약 (O)").open(&mut open).default_pos([360.0, 120.0]).default_size([560.0, 520.0]).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if ui.small_button("새로고침").clicked() {
                net.send(ClientMsg::RequestContracts);
            }
            use view::panels::ContractRequest;
            match view::panels::contracts(ui, &s.contracts, prof.player_id, prof.wallet.credits, &s.charted, s.system, &mut s.ui.contract_form) {
                Some(ContractRequest::Post { kind, reward, hours }) => net.send(ClientMsg::PostContract { kind, reward, hours }),
                Some(ContractRequest::Accept(id)) => net.send(ClientMsg::AcceptContract { id }),
                Some(ContractRequest::Cancel(id)) => net.send(ClientMsg::CancelContract { id }),
                Some(ContractRequest::Deliver(id)) => net.send(ClientMsg::DeliverContract { id }),
                None => {}
            }
        });
    });
    s.ui.contracts = open;

    let mut open = s.ui.chronicle;
    egui::Window::new("은하 연대기 · Galactic Herald (H)").open(&mut open).default_pos([300.0, 60.0]).default_size([720.0, 640.0]).show(ctx, |ui| chronicle(ui, s, net, &prof));
    s.ui.chronicle = open;

    let mut open = s.ui.fleet;
    egui::Window::new("함대 (K)").open(&mut open).default_size([360.0, 320.0]).show(ctx, |ui| fleet(ui, s, net));
    s.ui.fleet = open;

    let mut open = s.ui.alliance;
    egui::Window::new("동맹 (L)").open(&mut open).default_pos([360.0, 100.0]).default_size([520.0, 520.0]).show(ctx, |ui| alliance(ui, s, net));
    s.ui.alliance = open;

    let mut open = s.ui.help;
    egui::Window::new("도움말 (F1)").open(&mut open).show(ctx, |ui| {
        ui.label("당신은 평생 단 한 척의 함선만 가집니다. 그 함선을 키우는 길:");
        ui.label("• 일일 미션(J) — 매일 바뀌는 4개의 임무. 완료 후 보상 수령.");
        ui.label("• 교전 — 해적 격파, War Zone 함대전 승리, 현상금 사냥.");
        ui.label("• 시나리오(J) — 5장 캠페인. 한 장을 돌파할 때마다 모듈 상한 +1 (Lv5 → Lv10).");
        ui.label("• 채굴(N)·판매(U) — 광석을 캐서 팔거나 정제해 합금/결정/데이터코어로.");
        ui.label("• 탐험(V) — 이상현상, 유적, 잔해를 스캔. 최초 발견은 연대기에 남습니다.");
        ui.separator();
        ui.label("위험 등급: CORE(안전) → FRONTIER(해적) → WILD(PvP, 범죄 기록) → WAR ZONE(함대전) → DEEP(최고 위험).");
        ui.label("함대(K)를 꾸려 함께 싸우면 아군 사격이 없고 보상을 공유합니다.");
        ui.label("성장: Rank 0–2 소형함 → Rank 3–6 순양함(포탑·함수포·호위함) → Rank 7–10 주력함(주포 일제사·포탑 포대·호위 함대).");
        ui.label("동맹(L): 창설·초대·선전포고·강화. 전쟁 중인 동맹끼리는 CORE를 제외한 모든 성계에서 교전할 수 있습니다.");
        ui.label("격침된 함선의 잔해는 남습니다. 잔해를 조사(V)해 증거를 얻고, 연대기(H)에서 연구를 발표하거나 반박하세요.");
        ui.separator();
        for t in view::panels::TACTICS_3D {
            ui.label(t);
        }
    });
    s.ui.help = open;
}

fn station(ui: &mut egui::Ui, s: &mut Session, net: &Net, prof: &Profile, env: &Env) {
    let docked = s.my_state().is_some_and(|m| m.docked);
    if !docked {
        let d = s.my_state().zip(env.layout.as_ref()).map(|(m, l)| Vec3::from(m.pos).distance(l.station)).unwrap_or(0.0);
        ui.label(RichText::new(format!("정거장에 도킹해야 이용할 수 있습니다 (G). 거리 {d:.0}u")).color(GOLD));
    }
    let ship = &prof.ship;
    let cap = tier_cap(prof.scenarios_completed);
    ui.label(RichText::new(format!("「{}」 {} — {}급 (총 레벨 {}/80)", ship.name, ship.hull.name(), ship.designation(), ship.total_levels())).strong());
    ui.label(RichText::new(format!("모듈 상한 Lv{cap} — 시나리오를 돌파할 때마다 +1 (최대 Lv{MAX_MODULE_LEVEL})")).color(DIM).size(12.0));
    egui::Grid::new("mods").striped(true).num_columns(4).show(ui, |ui| {
        for m in Module::ALL {
            let lvl = ship.level(m);
            ui.label(RichText::new(m.name()).strong());
            ui.label(format!("Lv{lvl}"));
            if lvl >= MAX_MODULE_LEVEL {
                ui.label(RichText::new("최대").color(GOLD));
                ui.label("");
            } else {
                let cost = upgrade_cost(m, lvl);
                let afford = prof.wallet.covers(&cost);
                ui.label(RichText::new(cost.describe()).size(11.0).color(if afford { Color32::WHITE } else { DIM }));
                let ok = docked && afford && lvl < cap;
                let txt = if lvl >= cap { "잠김" } else { "업그레이드" };
                if ui.add_enabled(ok, egui::Button::new(txt)).on_hover_text(m.effect()).clicked() {
                    net.send(ClientMsg::Upgrade { module: m });
                }
            }
            ui.end_row();
        }
    });
    ui.separator();
    ui.label(RichText::new(format!("화물칸 {}/{:.0}", prof.cargo_used(), ship.stats().cargo)).strong());
    for ore in Ore::ALL {
        let have = *prof.cargo.get(ore.name()).unwrap_or(&0);
        if have == 0 {
            continue;
        }
        ui.horizontal(|ui| {
            ui.label(format!("{} × {}", ore.name(), have));
            if ui.add_enabled(docked, egui::Button::new("전량 판매")).clicked() {
                net.send(ClientMsg::Sell { ore, units: have });
            }
            let what = match ore {
                Ore::Ferrite => "합금으로 정제 (5:1)",
                Ore::Lumen => "결정으로 정제 (4:1)",
                Ore::Xenite => "데이터코어로 정제 (2:1)",
            };
            if ui.add_enabled(docked, egui::Button::new(what)).clicked() {
                net.send(ClientMsg::Refine { ore, units: have });
            }
        });
    }
    if prof.cargo.is_empty() {
        ui.label(RichText::new("화물 없음 — 소행성대에서 채굴(N)하세요. 위험한 성계일수록 비싸게 팔립니다.").color(DIM).size(12.0));
    }
    ui.separator();
    if view::panels::specimens(ui, &prof.specimens, docked) {
        net.send(ClientMsg::SellSpecimens);
    }
    ui.separator();
    let t = shared::tuning::get();
    let prices = view::panels::ArtifactPrices {
        appraisal: if prof.profession == Profession::Archaeologist { 0 } else { t.artifacts.appraisal_cost },
        unappraised_sale: t.artifacts.unappraised_sale,
        donation_share: t.artifacts.donation_share,
        donation_reputation: t.artifacts.donation_reputation,
    };
    use view::panels::ArtifactRequest;
    match view::panels::artifacts(ui, &s.artifacts, docked, &prices) {
        Some(ArtifactRequest::Appraise(id)) => net.send(ClientMsg::AppraiseArtifact { id }),
        Some(ArtifactRequest::Sell(id)) => net.send(ClientMsg::SellArtifact { id }),
        Some(ArtifactRequest::Donate(id)) => net.send(ClientMsg::DonateArtifact { id }),
        None => {}
    }    ui.separator();
    let cost = t.social.institute_cost;
    use view::panels::InstituteRequest;
    let mut name = s.ui.institute_name.clone();
    match view::panels::institutes(ui, &s.institutes, s.system, &s.artifacts, prof.player_id, prof.wallet.credits, docked, cost, &mut name) {
        Some(InstituteRequest::Found(n)) => net.send(ClientMsg::FoundInstitute { name: n }),
        Some(InstituteRequest::Donate { artifact, institute }) => net.send(ClientMsg::DonateToInstitute { artifact, institute }),
        None => {}
    }
    s.ui.institute_name = name;
}

fn missions(ui: &mut egui::Ui, net: &Net, prof: &Profile) {
    ui.heading("일일 미션");
    ui.label(RichText::new("매일 UTC 자정에 새로 생성됩니다.").color(DIM).size(11.0));
    for (i, m) in prof.dailies.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::ProgressBar::new(m.progress as f32 / m.objective.target() as f32).desired_width(90.0).text(format!("{}/{}", m.progress, m.objective.target())));
            ui.label(m.objective.describe());
            if m.claimed {
                ui.label(RichText::new("수령 완료").color(DIM));
            } else if ui.add_enabled(m.complete(), egui::Button::new(format!("보상: {}", m.reward.describe()))).clicked() {
                net.send(ClientMsg::ClaimDaily { index: i });
            }
        });
        if matches!(m.objective, Objective::KillPlayers { .. } | Objective::WinEngagement) {
            ui.label(RichText::new("   ↳ WILD / WAR ZONE 성계에서만 가능").color(DIM).size(11.0));
        }
    }
    ui.separator();
    ui.heading("시나리오 캠페인");
    for sc in scenarios() {
        let done = sc.id < prof.scenarios_completed;
        let active = prof.scenario.as_ref().is_some_and(|p| p.id == sc.id);
        let available = sc.id == prof.scenarios_completed && prof.scenario.is_none();
        let col = if done { GREEN } else if active { GOLD } else if available { Color32::WHITE } else { DIM };
        egui::CollapsingHeader::new(RichText::new(format!("{}{}", sc.title, if done { "  ✔" } else { "" })).color(col)).default_open(active || available).show(ui, |ui| {
            ui.label(RichText::new(sc.briefing).size(12.0));
            for (k, st) in sc.steps.iter().enumerate() {
                let mark = match &prof.scenario {
                    Some(p) if p.id == sc.id && k < p.step => "✔",
                    Some(p) if p.id == sc.id && k == p.step => "▶",
                    _ if done => "✔",
                    _ => "·",
                };
                ui.label(format!("{mark} {}", st.describe()));
            }
            ui.label(RichText::new(format!("보상: {} · 모듈 상한 +1", sc.reward.describe())).color(GOLD).size(12.0));
            if available && ui.button("시나리오 시작").clicked() {
                net.send(ClientMsg::StartScenario { id: sc.id });
            }
            if active && ui.button("포기").clicked() {
                net.send(ClientMsg::AbandonScenario);
            }
        });
    }
}

fn chronicle(ui: &mut egui::Ui, s: &mut Session, net: &Net, prof: &Profile) {
    ui.horizontal(|ui| {
        if ui.button("새로고침").clicked() {
            net.send(ClientMsg::RequestChronicle);
        }
        ui.label("전기 조회:");
        ui.add(egui::TextEdit::singleline(&mut s.ui.bio_query).desired_width(140.0));
        if ui.button("조회").clicked() {
            let n = if s.ui.bio_query.trim().is_empty() { prof.name.clone() } else { s.ui.bio_query.trim().to_string() };
            net.send(ClientMsg::RequestBiography { name: n });
        }
    });
    if let Some((name, ch, sh)) = &s.biography {
        egui::CollapsingHeader::new(RichText::new(format!("📜 {name}의 역사적 전기")).color(GOLD)).default_open(true).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("bio").max_height(140.0).show(ui, |ui| {
                for b in ch {
                    ui.label(RichText::new(format!("{}  {}", b.date, b.text)).size(12.0));
                }
                if !sh.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("함선 기록").color(DIM));
                    for b in sh {
                        ui.label(RichText::new(format!("{}  {}", b.date, b.text)).size(12.0));
                    }
                }
            });
        });
    }
    let Some((events, evidence, herald)) = s.chronicle.clone() else {
        ui.label("불러오는 중…");
        return;
    };
    egui::CollapsingHeader::new(RichText::new("📰 GALACTIC HERALD").color(ACCENT).strong()).default_open(true).show(ui, |ui| {
        for h in herald.iter().take(5) {
            let correction = h.headline.starts_with("[정정]");
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&h.headline).strong().color(if correction { RED } else { Color32::WHITE }));
                if let Some(by) = &h.by {
                    ui.label(RichText::new(format!("— {by} 기자")).size(11.0).color(GOLD));
                }
            });
            ui.label(RichText::new(format!("[{}] {}", h.date, h.body)).size(11.0).color(DIM));
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("events").show(ui, |ui| {
        for e in &events {
            let col = match e.importance {
                Importance::Historic => GOLD,
                Importance::Major => view::palette::MAJOR,
                _ => Color32::WHITE,
            };
            egui::CollapsingHeader::new(RichText::new(format!("[{}] {} — {} ({})", e.date, e.title, galaxy::system(e.system).name, e.importance.label())).color(col)).id_salt(e.id).show(ui, |ui| {
                ui.label(&e.summary);
                if !e.causes.is_empty() {
                    let names: Vec<String> = e.causes.iter().filter_map(|c| events.iter().find(|x| x.id == *c).map(|x| x.title.clone())).collect();
                    ui.label(RichText::new(format!("원인: {}", if names.is_empty() { format!("{:?}", e.causes) } else { names.join(", ") })).size(11.0).color(DIM));
                }
                ui.label(RichText::new("증거").color(DIM));
                let t = shared::tuning::get();
                let check_cost = if prof.profession == Profession::Historian { 0 } else { t.history.authenticate_cost };
                for ev in evidence.iter().filter(|x| x.event == e.id) {
                    let mine = prof.archive.contains(&ev.id);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{} #{} {} — {} (신뢰 {:.0}%){}: {}", if mine { "📁" } else { "·" }, ev.id, ev.kind.name(), ev.source, ev.reliability * 100.0, ev.discovered_by.as_ref().map(|d| format!(" 발견: {d}")).unwrap_or_default(), ev.text)).size(11.0));
                        if ev.leaked {
                            ui.label(RichText::new("[공개됨]").color(GOLD).size(11.0));
                        } else if mine && ui.small_button("공개").on_hover_text("이 기록을 모두가 읽을 수 있게 공개합니다(봉인된 블랙박스 포함). 연대기·Herald에 폭로로 남습니다.").clicked() {
                            net.send(ClientMsg::LeakEvidence { evidence: ev.id });
                        }
                        match ev.authenticity {
                            Authenticity::Forged => {
                                ui.label(RichText::new("[위조 판명]").color(RED).size(11.0).strong());
                            }
                            Authenticity::Authentic => {
                                ui.label(RichText::new("[진품 확인]").color(GREEN).size(11.0));
                            }
                            Authenticity::Unverified => {
                                let txt = if check_cost == 0 { "감정 (무료)".to_string() } else { format!("감정 ({check_cost} CR)") };
                                if ui.small_button(txt).on_hover_text("진위를 감정합니다. 위조품이면 위조자와 그것을 인용한 연구가 드러납니다.").clicked() {
                                    net.send(ClientMsg::AuthenticateEvidence { evidence: ev.id });
                                }
                                if mine && !s.ui.forge_drafts.contains_key(&ev.id) && ui.small_button("위조…").on_hover_text("내용을 고친 사본을 만듭니다. 감정되기 전까지는 원본과 구별되지 않습니다.").clicked() {
                                    s.ui.forge_drafts.insert(ev.id, ev.text.clone());
                                }
                            }
                        }
                    });
                    if let Some(draft) = s.ui.forge_drafts.get_mut(&ev.id) {
                        let mut send = false;
                        let mut cancel = false;
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(draft).desired_width(360.0));
                            send = ui.button(format!("위조 사본 만들기 ({} CR)", t.history.forge_cost)).clicked();
                            cancel = ui.small_button("취소").clicked();
                        });
                        if send {
                            net.send(ClientMsg::ForgeEvidence { evidence: ev.id, text: draft.clone() });
                        }
                        if send || cancel {
                            s.ui.forge_drafts.remove(&ev.id);
                        }
                    }
                }
                for r in &e.research {
                    let note = if r.discredited { " [위조 증거 인용 — 신뢰 상실]" } else { "" };
                    ui.label(RichText::new(format!("🎓 {}: \"{}\" — 신뢰도 {:.0}%, 인용 {}건{}{note}", r.author, r.thesis, r.confidence * 100.0, r.cited.len(), if r.disputed_by.is_empty() { String::new() } else { format!(", 반박: {}", r.disputed_by.join(", ")) })).color(if r.discredited { DIM } else { ACCENT }).size(12.0));
                    if r.author != prof.name && ui.small_button(format!("{}의 연구 반박", r.author)).clicked() {
                        net.send(ClientMsg::DisputeResearch { event: e.id, author: r.author.clone() });
                    }
                }
                let held: Vec<u64> = evidence.iter().filter(|x| x.event == e.id && prof.archive.contains(&x.id)).map(|x| x.id).collect();
                if !held.is_empty() {
                    ui.horizontal(|ui| {
                        let draft = s.ui.research_drafts.entry(e.id).or_default();
                        ui.add(egui::TextEdit::singleline(draft).hint_text("당신의 해석 (연구 주장)").desired_width(320.0));
                        if ui.button(format!("증거 {}건 인용하여 발표", held.len())).clicked() {
                            net.send(ClientMsg::PublishResearch { event: e.id, thesis: draft.clone(), cite: held.clone() });
                        }
                    });
                } else if e.kind == "SHIP_DESTROYED" {
                    ui.label(RichText::new("현장의 잔해를 조사(V)하면 증거를 확보해 연구를 발표할 수 있습니다.").size(11.0).color(DIM));
                }
                if prof.profession == Profession::Journalist {
                    // Report it: from the scene (this system) or holding its evidence; the server checks.
                    let filed = s.ui.stories_filed.contains(&e.id);
                    if filed {
                        ui.label(RichText::new("이 사건의 기사를 송고했습니다.").size(11.0).color(DIM));
                    } else {
                        let (head, body) = s.ui.story_drafts.entry(e.id).or_default();
                        ui.add(egui::TextEdit::singleline(head).hint_text("기사 제목").desired_width(420.0));
                        ui.add(egui::TextEdit::multiline(body).hint_text("기사 본문 — 현장에서 보고 들은 것").desired_rows(2).desired_width(420.0));
                        let t = shared::tuning::get();
                        if ui.button(format!("Herald에 송고 (+{} CR, 독자당 +{} CR)", t.history.story_fee, t.history.story_per_reader)).clicked() {
                            net.send(ClientMsg::FileStory { event: e.id, headline: head.clone(), body: body.clone(), cite: held.clone() });
                            s.ui.stories_filed.insert(e.id);
                        }
                    }
                }
            });
        }
        if events.is_empty() {
            ui.label("아직 기록된 역사가 없습니다. 당신이 첫 페이지를 쓰세요.");
        }
    });
}

fn fleet(ui: &mut egui::Ui, s: &mut Session, net: &Net) {
    if let Some((from, fid)) = s.invite.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{from}의 함대 초대")).color(GOLD));
            if ui.button("수락").clicked() {
                net.send(ClientMsg::FleetAccept { fleet: fid });
                s.invite = None;
            }
            if ui.button("거절").clicked() {
                s.invite = None;
            }
        });
        ui.separator();
    }
    match s.fleet {
        Some(id) => {
            ui.label(RichText::new(format!("함대 #{id}")).strong());
            for m in &s.fleet_members {
                ui.label(format!("{} {} — {} · {}", if m.online { "●" } else { "○" }, m.name, m.hull.name(), galaxy::system(m.system).name));
            }
            if ui.button("함대 탈퇴").clicked() {
                net.send(ClientMsg::FleetLeave);
            }
        }
        None => {
            ui.label("소속 함대 없음. 함장을 초대하면 함대가 결성됩니다 (최대 5척).");
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut s.ui.invite_name).hint_text("함장 이름").desired_width(160.0));
        if ui.button("초대").clicked() && !s.ui.invite_name.trim().is_empty() {
            net.send(ClientMsg::FleetInvite { name: s.ui.invite_name.trim().to_string() });
        }
    });
    view::panels::fleet_tips(ui);
}

fn alliance(ui: &mut egui::Ui, s: &mut Session, net: &Net) {
    {
        let mine = s.alliances.iter().find(|a| Some(a.id) == s.my_alliance).map(|a| a.tag.clone());
        view::panels::influence(ui, &s.influence, mine.as_deref(), s.influence_threshold);
        ui.separator();
    }
    if let Some((from, id, name)) = s.alliance_invite.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{from}의 초대: {name}")).color(GOLD));
            if ui.button("가입").clicked() {
                net.send(ClientMsg::AllianceAccept { alliance: id });
                s.alliance_invite = None;
            }
            if ui.button("거절").clicked() {
                s.alliance_invite = None;
            }
        });
        ui.separator();
    }
    let mine = s.my_alliance.and_then(|m| s.alliances.iter().find(|a| a.id == m)).cloned();
    match &mine {
        Some(a) => {
            ui.label(RichText::new(format!("[{}] {}", a.tag, a.name)).size(20.0).color(ACCENT).strong());
            ui.label(format!("맹주 {} · 함장 {}명", a.leader, a.members.len()));
            ui.label(RichText::new(a.members.join(", ")).size(12.0).color(DIM));
            let leader = s.profile.as_ref().is_some_and(|p| p.name == a.leader);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("금고 {} CR", a.treasury)).color(GOLD)).on_hover_text("지배 성계에서 비동맹 함장의 광석 판매·현상금에 붙는 세금");
                let held = s.influence.iter().filter(|x| x.controller.as_ref().is_some_and(|c| c.0 == a.id)).count();
                let upkeep = shared::tuning::get().social.upkeep(held);
                ui.label(RichText::new(format!("지배 {held}곳 · 유지비 {upkeep} CR/시간")).size(12.0).color(if upkeep > a.treasury { RED } else { DIM }))
                    .on_hover_text("첫 성계는 무료. 금고가 유지비를 못 내면 가장 약한 성계부터 철수합니다.");
                if leader && ui.add_enabled(a.treasury >= a.members.len() as u64, egui::Button::new("동맹원에게 균등 배당")).clicked() {
                    net.send(ClientMsg::AllianceDividend);
                }
            });
            if leader {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.ui.invite_name).hint_text("초대할 함장").desired_width(160.0));
                    if ui.button("초대").clicked() && !s.ui.invite_name.trim().is_empty() {
                        net.send(ClientMsg::AllianceInvite { name: s.ui.invite_name.trim().to_string() });
                    }
                });
            }
            if ui.button("동맹 탈퇴").clicked() {
                net.send(ClientMsg::AllianceLeave);
            }
            ui.separator();
            ui.label(RichText::new("외교").strong());
            let me_id = a.id;
            for other in s.alliances.clone().iter().filter(|o| o.id != me_id) {
                ui.horizontal(|ui| {
                    let at_war = a.wars.contains(&other.id);
                    ui.label(RichText::new(format!("[{}] {} ({}명)", other.tag, other.name, other.members.len())).color(if at_war { RED } else { Color32::WHITE }));
                    if at_war {
                        ui.label(RichText::new("⚔ 전쟁 중").color(RED));
                        if leader && ui.button("강화 제의").clicked() {
                            net.send(ClientMsg::AlliancePeace { target: other.id });
                        }
                    } else if leader && ui.button("선전포고").clicked() {
                        net.send(ClientMsg::AllianceDeclareWar { target: other.id });
                    }
                });
            }
        }
        None => {
            ui.label("소속 동맹 없음. 동맹을 창설하거나 초대를 받으세요.");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut s.ui.alliance_name).hint_text("동맹 이름").desired_width(180.0));
                ui.add(egui::TextEdit::singleline(&mut s.ui.alliance_tag).hint_text("태그 (예: FPA)").desired_width(90.0));
                if ui.button("창설 (1000 CR)").clicked() {
                    net.send(ClientMsg::AllianceCreate { name: s.ui.alliance_name.clone(), tag: s.ui.alliance_tag.clone() });
                }
            });
            ui.separator();
            for a in &s.alliances {
                ui.label(format!("[{}] {} — 맹주 {}, {}명", a.tag, a.name, a.leader, a.members.len()));
            }
        }
    }
    ui.separator();
    ui.label(RichText::new("동맹원끼리는 서로 공격할 수 없습니다. 전쟁 중인 동맹과는 CORE를 제외한 모든 성계에서 범죄 기록 없이 교전하며, 동맹 대 동맹 전투는 연대기에 기록됩니다.").size(11.0).color(DIM));
}
