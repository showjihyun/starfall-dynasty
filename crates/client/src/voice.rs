//! Plays the generated voice lines (`view::voice`, assets/voice) that the cutscene and the comms
//! window ask for through `Session::voice`. One voice at a time: a new line or a Stop silences the
//! last. Runs in PostUpdate, outside the Update graph (memory: new Update edges move the art scenes).

use crate::state::Session;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use std::sync::OnceLock;
use view::voice::{Cue, Manifest};

/// `assets/voice/manifest.json`, read once (missing → no voices, subtitles only).
pub fn manifest() -> &'static Manifest {
    static FILE: OnceLock<Manifest> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = crate::ships::assets_dir().join("voice/manifest.json");
        std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Manifest::parse(&t)).unwrap_or_else(|e| {
            eprintln!("{}: {e} (no voices)", path.display());
            Manifest::default()
        })
    })
}

/// A voice line playing (music ducks under it).
#[derive(Component)]
pub struct VoiceLine;

pub struct VoicePlugin;

impl Plugin for VoicePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, speak);
    }
}

fn speak(mut commands: Commands, mut s: ResMut<Session>, server: Res<AssetServer>, playing: Query<Entity, With<VoiceLine>>) {
    if s.voice.is_empty() {
        return;
    }
    for cue in std::mem::take(&mut s.voice) {
        for e in &playing {
            commands.entity(e).try_despawn();
        }
        if let Cue::Play { voice, text } = cue
            && let Some(line) = manifest().get(&voice, &text)
        {
            info!("voice: {} ({:.1} s) {}", line.file, line.secs, line.text);
            commands.spawn((VoiceLine, AudioPlayer::<AudioSource>(server.load(format!("voice/{}", line.file))), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(1.0))));
        }
    }
}
