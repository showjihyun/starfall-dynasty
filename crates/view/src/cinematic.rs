//! In-engine cinematics (docs/STORY.md §5–7): a cutscene is a list of shots in a data file
//! (`crates/client/assets/cinematics/*.ron`), and what the screen shows at time `t` — which star
//! system, where the camera is, what it looks at, which subtitles are up, how dark the cut is — is a
//! pure function of the file, the captain's species and `t`. Same command, same frame (HARNESS), so
//! artcheck can diff any shot and these tests check timing and layout without the engine.
//!
//! Positions are anchors in the shared system layout (`shared::galaxy::system_layout`) plus an offset
//! in metres, so a shot keeps pointing at the station or the gate if the layout generator changes.

use serde::Deserialize;
use shared::galaxy::{self, PoiKind, Species, SystemId};
use shared::glam::Vec3;

/// A point in a system's layout.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Anchor {
    Star,
    Station,
    JumpPoint,
    /// The first point of interest of that kind in the system.
    Poi(PoiKind),
}

/// Where the captain's own ship floats during a shot (hull, species kit and livery from the profile).
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct ShipMark {
    pub anchor: Anchor,
    pub offset: [f32; 3],
}

#[derive(Clone, Debug, Deserialize)]
pub struct Caption {
    /// Seconds into the shot.
    pub at: f32,
    /// Seconds on screen.
    pub secs: f32,
    pub text: String,
    /// Who says it ("인류 제국 기록보관소"), shown under the line.
    #[serde(default)]
    pub source: Option<String>,
    /// Voice preset (`assets/voice/voices.json`); the narrator when absent.
    #[serde(default)]
    pub voice: Option<String>,
}

/// The voice of captions that name none.
pub const NARRATOR: &str = "narrator";

impl Caption {
    pub fn voice(&self) -> &str {
        self.voice.as_deref().unwrap_or(NARRATOR)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Shot {
    pub id: String,
    pub secs: f32,
    pub system: SystemId,
    /// What the camera looks at: anchor + offset.
    pub anchor: Anchor,
    #[serde(default)]
    pub offset: [f32; 3],
    /// Camera position relative to the look point at the start and end of the shot (smoothstep).
    pub from: [f32; 3],
    pub to: [f32; 3],
    #[serde(default)]
    pub captions: Vec<Caption>,
    #[serde(default)]
    pub ship: Option<ShipMark>,
    /// Fleet battle backdrop (the client's bench fleets).
    #[serde(default)]
    pub battle: bool,
    /// Title card text over the shot.
    #[serde(default)]
    pub title: Option<String>,
    /// Shots with a group are reordered per species (`Cinematic::order`); the others keep their place.
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Cinematic {
    pub name: String,
    /// Group order per species, e.g. Veyr: ["memory", "record", "contract"].
    #[serde(default)]
    pub order: Vec<(Species, Vec<String>)>,
    pub shots: Vec<Shot>,
}

/// Seconds of black at each cut (half out, half in).
pub const CUT: f32 = 0.6;
/// Subtitle fade in/out.
pub const CAPTION_FADE: f32 = 0.4;
/// Longest subtitle line, in characters: a quick rule for authors. The real limit is the pixel
/// width at SUBTITLE_PX inside SUBTITLE_BAND_W (tests/cinematic.rs measures it with the Korean font).
pub const MAX_LINE_CHARS: usize = 34;
/// Subtitle and source sizes in points, and the widest subtitle band (80% of a 1280 px screen).
pub const SUBTITLE_PX: f32 = 26.0;
pub const SOURCE_PX: f32 = 16.0;
pub const TITLE_PX: f32 = 46.0;
pub const SUBTITLE_BAND_W: f32 = 1024.0;

impl Cinematic {
    pub fn parse(ron_text: &str) -> Result<Self, String> {
        ron::from_str(ron_text).map_err(|e| e.to_string())
    }

    /// The shots in playing order for this captain's species.
    pub fn sequence(&self, species: Species) -> Vec<&Shot> {
        let mut out: Vec<&Shot> = self.shots.iter().collect();
        let Some((_, groups)) = self.order.iter().find(|(s, _)| *s == species) else { return out };
        let slots: Vec<usize> = (0..out.len()).filter(|&i| out[i].group.is_some()).collect();
        let mut grouped: Vec<&Shot> = slots.iter().map(|&i| out[i]).collect();
        grouped.sort_by_key(|s| groups.iter().position(|g| Some(g) == s.group.as_ref()).unwrap_or(usize::MAX));
        for (slot, shot) in slots.into_iter().zip(grouped) {
            out[slot] = shot;
        }
        out
    }

    pub fn duration(&self) -> f32 {
        self.shots.iter().map(|s| s.secs).sum()
    }
}

/// Everything the client needs to draw time `t`.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame<'a> {
    pub index: usize,
    pub shot: &'a str,
    pub system: SystemId,
    pub eye: Vec3,
    pub look_at: Vec3,
    /// 0 = clear, 1 = black (cuts, start and end).
    pub black: f32,
    /// (line, source, opacity 0..1) currently on screen.
    pub captions: Vec<(&'a str, Option<&'a str>, f32)>,
    pub title: Option<&'a str>,
    pub ship: Option<Vec3>,
    pub battle: bool,
    pub done: bool,
}

pub fn anchor_pos(system: SystemId, anchor: Anchor) -> Option<Vec3> {
    let lay = galaxy::system_layout(system);
    match anchor {
        Anchor::Star => Some(galaxy::STAR_POS),
        Anchor::Station => Some(lay.station),
        Anchor::JumpPoint => Some(lay.jump_point),
        Anchor::Poi(kind) => lay.pois.iter().find(|p| p.kind == kind).map(|p| p.pos),
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The frame at `t` seconds for a captain of `species`. Past the end it holds the last frame, black.
pub fn frame_at(cin: &Cinematic, species: Species, t: f32) -> Frame<'_> {
    let seq = cin.sequence(species);
    let total = cin.duration();
    let t = t.clamp(0.0, total);
    let mut start = 0.0;
    let mut index = seq.len() - 1;
    for (i, s) in seq.iter().enumerate() {
        if t < start + s.secs || i == seq.len() - 1 {
            index = i;
            break;
        }
        start += s.secs;
    }
    let shot = seq[index];
    let local = t - start;
    let look_at = anchor_pos(shot.system, shot.anchor).unwrap_or(Vec3::ZERO) + Vec3::from(shot.offset);
    let k = smoothstep(local / shot.secs);
    let eye = look_at + Vec3::from(shot.from).lerp(Vec3::from(shot.to), k);
    // Black: half a cut at each shot edge, ramping linearly.
    let half = CUT * 0.5;
    let black = (1.0 - local / half).max(1.0 - (shot.secs - local) / half).clamp(0.0, 1.0);
    let captions = shot
        .captions
        .iter()
        .filter_map(|c| {
            let u = local - c.at;
            if !(0.0..c.secs).contains(&u) {
                return None;
            }
            let alpha = (u / CAPTION_FADE).min((c.secs - u) / CAPTION_FADE).clamp(0.0, 1.0);
            Some((c.text.as_str(), c.source.as_deref(), alpha))
        })
        .collect();
    let ship = shot.ship.and_then(|m| anchor_pos(shot.system, m.anchor).map(|p| p + Vec3::from(m.offset)));
    Frame { index, shot: &shot.id, system: shot.system, eye, look_at, black, captions, title: shot.title.as_deref(), ship, battle: shot.battle, done: t >= total }
}

/// Captions that come up after `t0` and by `t1` (cinematic clock), in order: the voice lines to start
/// on this frame. Nothing once the cinematic is over.
pub fn caption_cues(cin: &Cinematic, species: Species, t0: f32, t1: f32) -> Vec<&Caption> {
    let mut start = 0.0;
    let mut out = vec![];
    for shot in cin.sequence(species) {
        for c in &shot.captions {
            let at = start + c.at;
            if at > t0 && at <= t1 {
                out.push(c);
            }
        }
        start += shot.secs;
    }
    out
}

/// Problems with a cinematic file, in words (empty = fine): anchors that do not exist in their
/// system, captions that run past their shot or overlap, lines too long for the subtitle band,
/// cameras inside what they look at.
pub fn check(cin: &Cinematic) -> Vec<String> {
    let mut bad = vec![];
    for s in &cin.shots {
        if s.secs <= CUT {
            bad.push(format!("{}: {} s is shorter than a cut", s.id, s.secs));
        }
        if anchor_pos(s.system, s.anchor).is_none() {
            bad.push(format!("{}: no {:?} in system {}", s.id, s.anchor, s.system));
        }
        if let Some(m) = s.ship
            && anchor_pos(s.system, m.anchor).is_none()
        {
            bad.push(format!("{}: ship anchor {:?} missing in system {}", s.id, m.anchor, s.system));
        }
        if Vec3::from(s.from).length() < 1.0 || Vec3::from(s.to).length() < 1.0 {
            bad.push(format!("{}: camera sits on its look point", s.id));
        }
        let mut last_end = 0.0;
        for c in &s.captions {
            if c.at < CUT * 0.5 || c.at + c.secs > s.secs - CUT * 0.5 {
                bad.push(format!("{}: caption \"{}\" runs into a cut", s.id, c.text));
            }
            if c.at < last_end {
                bad.push(format!("{}: caption \"{}\" overlaps the one before", s.id, c.text));
            }
            last_end = c.at + c.secs;
            for line in c.text.lines().chain(c.source.as_deref()) {
                if line.chars().count() > MAX_LINE_CHARS {
                    bad.push(format!("{}: line longer than {MAX_LINE_CHARS} characters: \"{line}\"", s.id));
                }
            }
            // Reading time: about 8 Korean characters a second, plus the fades.
            let chars = c.text.chars().filter(|ch| !ch.is_whitespace()).count() as f32;
            if c.secs < chars / 8.0 + 2.0 * CAPTION_FADE {
                bad.push(format!("{}: \"{}\" is on screen {:.1} s, needs {:.1} s to read", s.id, c.text, c.secs, chars / 8.0 + 2.0 * CAPTION_FADE));
            }
        }
    }
    for (species, groups) in &cin.order {
        for g in cin.shots.iter().filter_map(|s| s.group.as_ref()) {
            if !groups.contains(g) {
                bad.push(format!("order for {species:?} does not place group {g}"));
            }
        }
    }
    bad
}

/// Letterbox bar height, as a fraction of the screen height (top and bottom).
pub const LETTERBOX: f32 = 0.09;

/// Draws the cinematic layer for `f` over the whole screen: letterbox bars, subtitles with their
/// source (centred in the lower part of the picture), the title card, the cut fade and the skip hint.
/// Plain egui shapes on a foreground layer, so the HUD tests' approach checks it headless.
pub fn draw(ctx: &egui::Context, f: &Frame, skip_hint: bool) {
    use crate::palette::{alpha, cinema};
    use egui::{Align2, FontId, LayerId, Order, Pos2, Rect, Vec2};
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(LayerId::new(Order::Foreground, egui::Id::new("cinematic")));
    let (w, h) = (screen.width(), screen.height());
    let bar = h * LETTERBOX;
    painter.rect_filled(Rect::from_min_size(screen.min, Vec2::new(w, bar)), 0.0, cinema::BLACK);
    painter.rect_filled(Rect::from_min_size(Pos2::new(screen.min.x, screen.max.y - bar), Vec2::new(w, bar)), 0.0, cinema::BLACK);
    let a = |c, k: f32| alpha(c, (k.clamp(0.0, 1.0) * 255.0) as u8);
    let shadowed = |pos: Pos2, align: Align2, text: &str, font: FontId, color, k: f32| {
        painter.text(pos + Vec2::new(1.5, 1.5), align, text, font.clone(), a(cinema::SHADOW, k * 0.8));
        painter.text(pos, align, text, font, a(color, k))
    };
    // Subtitles: the line block's bottom sits just above the lower bar; the source goes under it.
    let mut y = screen.max.y - bar - h * 0.03;
    for (text, source, k) in f.captions.iter().rev() {
        if let Some(src) = source {
            let r = shadowed(Pos2::new(screen.center().x, y), Align2::CENTER_BOTTOM, &format!("— {src}"), FontId::proportional(SOURCE_PX), cinema::SOURCE, *k);
            y = r.min.y - 4.0;
        }
        // One centred text per line (a multi-line galley would left-align its lines).
        for line in text.lines().rev() {
            let r = shadowed(Pos2::new(screen.center().x, y), Align2::CENTER_BOTTOM, line, FontId::proportional(SUBTITLE_PX), cinema::LINE, *k);
            y = r.min.y - 4.0;
        }
        y -= 6.0;
    }
    if let Some(title) = f.title {
        let mut lines = title.lines();
        let main = lines.next().unwrap_or("");
        // Upper third: the shot puts the captain's ship in the middle of the frame.
        let r = shadowed(Pos2::new(screen.center().x, screen.min.y + h * 0.30), Align2::CENTER_BOTTOM, main, FontId::proportional(TITLE_PX), cinema::TITLE, 1.0 - f.black);
        for (i, line) in lines.enumerate() {
            shadowed(Pos2::new(screen.center().x, r.max.y + 14.0 + i as f32 * (SUBTITLE_PX + 8.0)), Align2::CENTER_TOP, line, FontId::proportional(SUBTITLE_PX), cinema::CHAPTER, 1.0 - f.black);
        }
    }
    if skip_hint {
        painter.text(Pos2::new(screen.max.x - 24.0, screen.min.y + bar * 0.5), Align2::RIGHT_CENTER, "Esc  건너뛰기", FontId::proportional(SOURCE_PX), cinema::SOURCE);
    }
    if f.black > 0.0 {
        painter.rect_filled(screen, 0.0, a(cinema::BLACK, f.black));
    }
}

/// Ship radius the shot offsets are written for. The camera distance scales with the real hull
/// (a Talon is 11 m, a capital ship over 200 m), so every ship fills the frame the same way.
pub const SHIP_SHOT_RADIUS: f32 = 40.0;

/// The camera for `f` when its shot frames the captain's ship of `radius` metres.
pub fn eye_for_ship(f: &Frame, radius: f32) -> Vec3 {
    if f.ship.is_none() {
        return f.eye;
    }
    f.look_at + (f.eye - f.look_at) * (radius / SHIP_SHOT_RADIUS)
}
