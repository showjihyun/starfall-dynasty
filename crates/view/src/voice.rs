//! Voice lines for the cutscenes and comms calls (docs/STORY.md §8: MeloTTS generated speech, each
//! speaker told apart by its preset's speed, pitch and effect). Files live in
//! `crates/client/assets/voice/`: `voices.json` (presets and the pronunciation table, written by hand),
//! `manifest.json` and the `.ogg` lines (written by `python tools/voice.py`, which asks
//! `cargo run -p view --example voice_lines` what to say). A line is found by (preset, on-screen text),
//! so editing a subtitle simply leaves it unvoiced until the tool is run again — `check` says so.

use crate::cinematic::Cinematic;
use crate::comms::Comms;
use serde::{Deserialize, Serialize};
use shared::galaxy::Species;

#[derive(Clone, Debug, Deserialize)]
pub struct Preset {
    pub id: String,
    /// Speaking rate for the TTS (1 = normal).
    pub speed: f32,
    /// Pitch shift in semitones (post-processing).
    pub pitch: f32,
    /// Post-processing: "none", "radio", "computer", "reverb".
    pub effect: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Voices {
    pub voices: Vec<Preset>,
    /// Latin names as the Korean voice should say them ("Vesta" → "베스타").
    #[serde(default)]
    pub say: Vec<(String, String)>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub voice: String,
    pub text: String,
    /// File name under `assets/voice/`.
    pub file: String,
    pub secs: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub lines: Vec<Entry>,
}

/// A line that should have a voice: preset and the text as shown on screen.
#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Wanted {
    pub voice: String,
    pub text: String,
}

/// What to do with the voice channel this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Cue {
    Play { voice: String, text: String },
    /// Silence whatever is speaking (a skipped cutscene, a closed call, the next comms line).
    Stop,
}

impl Voices {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    pub fn get(&self, voice: &str, text: &str) -> Option<&Entry> {
        self.lines.iter().find(|e| e.voice == voice && e.text == text)
    }

    /// Seconds of the voice line, if there is one.
    pub fn secs(&self, voice: &str, text: &str) -> Option<f32> {
        self.get(voice, text).map(|e| e.secs)
    }
}

/// Every line the opening and the comms calls speak, once each (all species' variants).
pub fn wanted(opening: &Cinematic, comms: &Comms) -> Vec<Wanted> {
    let mut out: Vec<Wanted> = opening.shots.iter().flat_map(|s| &s.captions).map(|c| Wanted { voice: c.voice().into(), text: c.text.clone() }).collect();
    out.extend(comms.chapters.iter().flat_map(|ch| &ch.lines).map(|l| Wanted { voice: l.who.clone(), text: l.text.clone() }));
    out.sort();
    out.dedup();
    out
}

/// Seconds a caption must stay up after its voice line ends.
pub const CAPTION_TAIL: f32 = 0.3;

/// Problems in words (empty = fine): lines without a preset or without a voice file, and opening
/// captions whose voice outlasts the subtitle.
pub fn check(opening: &Cinematic, comms: &Comms, voices: &Voices, manifest: &Manifest) -> Vec<String> {
    let mut bad = vec![];
    for w in wanted(opening, comms) {
        if !voices.voices.iter().any(|p| p.id == w.voice) {
            bad.push(format!("no voice preset \"{}\" for \"{}\"", w.voice, w.text));
        } else if manifest.get(&w.voice, &w.text).is_none() {
            bad.push(format!("not voiced yet ({}): \"{}\" — run python tools/voice.py", w.voice, w.text));
        }
    }
    for s in &opening.shots {
        for c in &s.captions {
            if let Some(v) = manifest.secs(c.voice(), &c.text)
                && v + CAPTION_TAIL > c.secs
            {
                bad.push(format!("{}: voice of \"{}\" is {v:.1} s, the caption is up {:.1} s", s.id, c.text, c.secs));
            }
        }
    }
    bad
}

/// The comms call after `after` for a `species` captain as (text, hold seconds), holds lengthened to
/// let each voice line finish.
pub fn comms_lines<'a>(comms: &'a Comms, manifest: &Manifest, after: u32, species: Species) -> Vec<(&'a str, f32)> {
    comms.lines(after, species).into_iter().map(|l| (l.text.as_str(), crate::comms::hold(&l.text, manifest.secs(&l.who, &l.text)))).collect()
}
