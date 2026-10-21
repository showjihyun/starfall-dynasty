//! Comms window at the end of each campaign chapter (docs/STORY.md §4, §6 way 2): a client, ally or
//! enemy calls the captain and opens the next chapter. Faceless first version: a species emblem and a
//! voice waveform instead of a portrait. The lines live in `crates/client/assets/cinematics/comms.ron`;
//! which line is up and how much of it is typed out is a function of the clock and the clicks, so the
//! tests below check timing and layout without the engine.

use serde::Deserialize;
use shared::galaxy::Species;

/// Who is talking: picks the emblem and the colour.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum Voice {
    Ship,
    Human,
    Veyr,
    Kharz,
    Aster,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Speaker {
    pub id: String,
    /// "이델 · 제국 기록보관소"
    pub name: String,
    pub voice: Voice,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Line {
    pub who: String,
    pub text: String,
    /// Said only to a captain of this species (each species hears its own voice react).
    #[serde(default)]
    pub only: Option<Species>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Chapter {
    /// Scenario id the call follows (0 = after chapter 1).
    pub after: u32,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Comms {
    pub speakers: Vec<Speaker>,
    pub chapters: Vec<Chapter>,
}

/// Typing speed, characters a second.
pub const TYPE_CPS: f32 = 28.0;
/// Reading speed for the hold (as the opening's subtitles: about 8 Korean characters a second).
pub const READ_CPS: f32 = 8.0;
/// Extra seconds a line stays after it could be read.
pub const HOLD: f32 = 1.5;
/// Longest line, in characters (the panel wraps it to at most MAX_ROWS rows of TEXT_PX).
pub const MAX_LINE_CHARS: usize = 60;
pub const MAX_ROWS: usize = 3;
/// Panel size and text sizes in points.
pub const PANEL_W: f32 = 500.0;
pub const PORTRAIT: f32 = 92.0;
pub const NAME_PX: f32 = 15.0;
pub const TEXT_PX: f32 = 17.0;
/// Top-left corner of the panel: under the HUD's location panel, clear of the toasts (top centre),
/// the chapter tracker (top right) and the ship panel (bottom centre).
pub const PANEL_POS: [f32; 2] = [14.0, 214.0];

impl Comms {
    pub fn parse(ron_text: &str) -> Result<Self, String> {
        ron::from_str(ron_text).map_err(|e| e.to_string())
    }

    /// The call after scenario `after`, as a captain of `species` hears it.
    pub fn lines(&self, after: u32, species: Species) -> Vec<&Line> {
        self.chapters.iter().find(|c| c.after == after).map_or(vec![], |c| c.lines.iter().filter(|l| l.only.is_none_or(|s| s == species)).collect())
    }

    pub fn speaker(&self, id: &str) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.id == id)
    }
}

fn chars(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// Seconds `text` stays up before the next line comes on its own.
pub fn hold(text: &str) -> f32 {
    chars(text) as f32 / READ_CPS + HOLD
}

/// Seconds to type `text` out.
pub fn typing(text: &str) -> f32 {
    text.chars().count() as f32 / TYPE_CPS
}

/// The part of `text` typed out `dt` seconds after the line came up.
pub fn typed(text: &str, dt: f32) -> &str {
    let n = (dt.max(0.0) * TYPE_CPS) as usize;
    match text.char_indices().nth(n) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

/// An open call: which chapter's call, which line, and when that line came up.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub after: u32,
    pub line: usize,
    pub since: f64,
}

impl Panel {
    pub fn new(after: u32, now: f64) -> Self {
        Panel { after, line: 0, since: now }
    }

    /// One frame of the call with `lines` (the current line's text is `lines[self.line]`): a click
    /// finishes the typing, or goes to the next line once it is typed; a line also moves on by itself
    /// after its hold. Returns false once the last line is done (close the window).
    pub fn step(&mut self, lines: &[&str], now: f64, clicked: bool) -> bool {
        let Some(text) = lines.get(self.line) else { return false };
        let dt = (now - self.since) as f32;
        let next = if clicked {
            if dt < typing(text) {
                // Show the whole line now, as if the typing had just finished.
                self.since = now - typing(text) as f64;
                false
            } else {
                true
            }
        } else {
            dt >= hold(text)
        };
        if next {
            self.line += 1;
            self.since = now;
        }
        self.line < lines.len()
    }
}

/// Problems with the comms file, in words (empty = fine).
pub fn check(c: &Comms) -> Vec<String> {
    let mut bad = vec![];
    for ch in &c.chapters {
        for s in Species::ALL {
            let n = ch.lines.iter().filter(|l| l.only.is_none_or(|x| x == s)).count();
            if n == 0 {
                bad.push(format!("call after {}: no lines for a {s:?} captain", ch.after));
            }
        }
        for l in &ch.lines {
            if c.speaker(&l.who).is_none() {
                bad.push(format!("call after {}: unknown speaker {}", ch.after, l.who));
            }
            if l.text.chars().count() > MAX_LINE_CHARS {
                bad.push(format!("call after {}: line longer than {MAX_LINE_CHARS} characters: \"{}\"", ch.after, l.text));
            }
        }
    }
    bad
}

fn color(v: Voice) -> egui::Color32 {
    use crate::palette::comms;
    match v {
        Voice::Ship => comms::SHIP,
        Voice::Human => comms::HUMAN,
        Voice::Veyr => comms::VEYR,
        Voice::Kharz => comms::KHARZ,
        Voice::Aster => comms::ASTER,
    }
}

/// The voice's emblem in `r` (faceless portraits): Human a chevron in a square plate, Veyr a ring with
/// three vein arcs, Kharz a hexagon with a clan bar, Aster a glyph ring of dots, the ship radar rings.
fn emblem(p: &egui::Painter, r: egui::Rect, v: Voice) {
    use egui::{Pos2, Shape, Stroke, Vec2};
    let c = r.center();
    let s = r.width() * 0.3;
    let col = color(v);
    let st = Stroke::new(2.5, col);
    let poly = |n: usize, rad: f32, phase: f32| -> Vec<Pos2> { (0..n).map(|i| c + Vec2::angled(phase + i as f32 * std::f32::consts::TAU / n as f32) * rad).collect() };
    match v {
        Voice::Human => {
            p.rect_stroke(egui::Rect::from_center_size(c, Vec2::splat(s * 2.0)), 3.0, st, egui::StrokeKind::Middle);
            p.line(vec![c + Vec2::new(-s * 0.6, s * 0.1), c + Vec2::new(0.0, -s * 0.5), c + Vec2::new(s * 0.6, s * 0.1)], st);
            p.line(vec![c + Vec2::new(-s * 0.6, s * 0.6), c + Vec2::new(0.0, 0.0), c + Vec2::new(s * 0.6, s * 0.6)], st);
        }
        Voice::Veyr => {
            p.circle_stroke(c, s, st);
            for k in 0..3 {
                let a = k as f32 * std::f32::consts::TAU / 3.0 - std::f32::consts::FRAC_PI_2;
                p.line_segment([c, c + Vec2::angled(a) * s * 0.85], Stroke::new(1.5, col));
            }
            p.circle_filled(c, s * 0.18, col);
        }
        Voice::Kharz => {
            p.add(Shape::closed_line(poly(6, s, 0.0), st));
            p.line_segment([c + Vec2::new(-s * 0.55, 0.0), c + Vec2::new(s * 0.55, 0.0)], Stroke::new(4.0, col));
        }
        Voice::Aster => {
            p.circle_stroke(c, s, Stroke::new(1.5, col));
            for q in poly(9, s, 0.0) {
                p.circle_filled(q, 2.5, col);
            }
            p.add(Shape::closed_line(poly(3, s * 0.5, -std::f32::consts::FRAC_PI_2), Stroke::new(1.5, col)));
        }
        Voice::Ship => {
            for k in 1..=3 {
                p.circle_stroke(c, s * k as f32 / 3.0, Stroke::new(1.2, col));
            }
            p.line_segment([c, c + Vec2::new(s * 0.7, -s * 0.7)], st);
        }
    }
}

/// Bar heights (0..1) of the voice waveform at time `t` while talking; flat when quiet.
pub fn waveform(t: f64, talking: bool) -> [f32; 12] {
    let mut out = [0.08; 12];
    if talking {
        for (i, h) in out.iter_mut().enumerate() {
            let x = t as f32 * 9.0 + i as f32 * 0.9;
            *h = 0.2 + 0.8 * (0.5 + 0.5 * x.sin()) * (0.6 + 0.4 * (x * 0.37 + 1.3).sin().abs());
        }
    }
    out
}

/// Draws the call: portrait (emblem + waveform), speaker name, the typed part of the line and the
/// "n/N · 클릭" hint. Returns true when the panel was clicked this frame.
pub fn draw(ctx: &egui::Context, who: &Speaker, shown: &str, talking: bool, (index, count): (usize, usize), t: f64) -> bool {
    use crate::palette::{DIM, chrome, comms};
    use egui::{Align2, FontId, RichText, Sense, Stroke, Vec2};
    let col = color(who.voice);
    let resp = egui::Area::new("comms".into()).anchor(Align2::LEFT_TOP, Vec2::from(PANEL_POS)).show(ctx, |ui| {
        egui::Frame::NONE.fill(chrome::PANEL).stroke(Stroke::new(1.5, col)).corner_radius(6.0).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(PANEL_W - 20.0);
            ui.horizontal_top(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(PORTRAIT), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 4.0, comms::WELL);
                let face = egui::Rect::from_min_size(rect.min, Vec2::new(PORTRAIT, PORTRAIT * 0.72));
                emblem(p, face, who.voice);
                let bars = waveform(t, talking);
                let bw = PORTRAIT / bars.len() as f32;
                let base = rect.max.y - 6.0;
                for (i, h) in bars.iter().enumerate() {
                    let x = rect.min.x + (i as f32 + 0.5) * bw;
                    p.line_segment([egui::pos2(x, base), egui::pos2(x, base - h * PORTRAIT * 0.22)], Stroke::new(bw * 0.55, col));
                }
                ui.vertical(|ui| {
                    ui.set_width(PANEL_W - 20.0 - PORTRAIT - 12.0);
                    ui.label(RichText::new(&who.name).font(FontId::proportional(NAME_PX)).color(col).strong());
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(RichText::new(shown).font(FontId::proportional(TEXT_PX)).color(comms::TEXT)).wrap());
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("{}/{} · 클릭: 다음", index + 1, count)).size(12.0).color(DIM));
            });
        })
    });
    ui_clicked(ctx, resp.response.rect)
}

fn ui_clicked(ctx: &egui::Context, rect: egui::Rect) -> bool {
    ctx.input(|i| i.pointer.primary_clicked() && i.pointer.interact_pos().is_some_and(|p| rect.contains(p)))
}
