//! The generated voice lines (crates/client/assets/voice, `python tools/voice.py`) against the
//! subtitles they speak: every line has its file, no voice outlasts its caption, and the cutscene
//! starts each line exactly once as its clock runs.

use shared::galaxy::Species;
use view::cinematic::{Cinematic, caption_cues};
use view::comms::Comms;
use view::voice::{Manifest, Voices, check, wanted};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../client/assets");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{ASSETS}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn all() -> (Cinematic, Comms, Voices, Manifest) {
    (
        Cinematic::parse(&read("cinematics/opening.ron")).unwrap(),
        Comms::parse(&read("cinematics/comms.ron")).unwrap(),
        Voices::parse(&read("voice/voices.json")).unwrap(),
        Manifest::parse(&read("voice/manifest.json")).unwrap(),
    )
}

#[test]
fn every_line_is_voiced_and_fits_its_subtitle() {
    let (o, c, v, m) = all();
    assert_eq!(check(&o, &c, &v, &m), Vec::<String>::new());
    assert_eq!(m.lines.len(), wanted(&o, &c).len(), "no stale lines in the manifest");
    let mut bytes = 0;
    for e in &m.lines {
        let len = std::fs::metadata(format!("{ASSETS}/voice/{}", e.file)).unwrap_or_else(|_| panic!("{} missing", e.file)).len();
        assert!(len > 2000 && e.secs > 0.5, "{} looks empty", e.file);
        bytes += len;
    }
    // Small next to the music (15 MB): voice must not become the heavy part of the repository.
    assert!(bytes < 4_000_000, "voice files are {:.1} MB", bytes as f64 / 1e6);
}

#[test]
fn check_catches_an_edited_subtitle_and_a_voice_too_long() {
    let (mut o, c, v, mut m) = all();
    o.shots[1].captions[0].text.push('!');
    let entry = m.lines.iter_mut().find(|e| e.text.starts_with("게이트 연쇄 고장")).unwrap();
    entry.secs = 9.0;
    let bad = check(&o, &c, &v, &m);
    assert!(bad.iter().any(|b| b.contains("not voiced yet")), "{bad:?}");
    assert!(bad.iter().any(|b| b.contains("the caption is up")), "{bad:?}");
}

#[test]
fn the_cutscene_starts_each_caption_once_as_the_clock_runs() {
    let (o, ..) = all();
    let total: usize = o.shots.iter().map(|s| s.captions.len()).sum();
    for species in Species::ALL {
        let mut said = vec![];
        let mut t = 0.0f32;
        while t < o.duration() + 1.0 {
            let next = t + 1.0 / 60.0;
            said.extend(caption_cues(&o, species, t, next).into_iter().map(|c| c.text.clone()));
            t = next;
        }
        assert_eq!(said.len(), total, "{species:?}");
        let first = &said[1];
        // After the gate line, the captain's own history speaks first.
        let own = match species {
            Species::Human => "게이트 연쇄 고장",
            Species::Veyr => "우리는 기억한다",
            Species::Kharz => "계약은 피로",
        };
        assert!(first.starts_with(own), "{species:?}: {first}");
    }
}
