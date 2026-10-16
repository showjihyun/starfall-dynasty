//! Adaptive background music (CC0 tracks, loudness-matched; see assets/music/CREDITS.md).
//! Safe systems rotate three exploration themes; dangerous space (Wild, War Zone, Deep) plays a
//! tense ambient loop; fighting crossfades to a battle theme, alternating between two per engagement.

use crate::state::{Screen, Session};
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Track {
    Explore1,
    Explore2,
    Explore3,
    Tension,
    Battle1,
    Battle2,
}

#[derive(Component)]
struct MusicTrack(Track);

#[derive(Resource)]
pub struct MusicSettings {
    pub volume: f32,
    pub muted: bool,
}

impl Default for MusicSettings {
    fn default() -> Self {
        MusicSettings { volume: 0.45, muted: false }
    }
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MusicSettings>().add_systems(Startup, start_tracks).add_systems(Update, (toggle_mute, crossfade));
    }
}

fn start_tracks(mut commands: Commands, server: Res<AssetServer>) {
    let dir = crate::ships::assets_dir();
    for (t, file) in [
        (Track::Explore1, "music/explore_observing_the_star.ogg"),
        (Track::Explore2, "music/explore_galactic_temple.ogg"),
        (Track::Explore3, "music/explore_out_there.ogg"),
        (Track::Tension, "music/tension_space_graveyard.ogg"),
        (Track::Battle1, "music/battle_determined_pursuit.ogg"),
        (Track::Battle2, "music/battle_theme_a.ogg"),
    ] {
        if !dir.join(file).exists() {
            warn!("music {file} missing");
            continue;
        }
        let h: Handle<AudioSource> = server.load(file);
        // All three loop silently from the start; the mixer only moves volumes, so switching is seamless.
        commands.spawn((MusicTrack(t), AudioPlayer(h), PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0))));
    }
}

fn toggle_mute(keys: Res<ButtonInput<KeyCode>>, mut m: ResMut<MusicSettings>, wants: Option<Res<bevy_egui::input::EguiWantsInput>>) {
    if wants.is_some_and(|w| w.wants_any_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::F2) {
        m.muted = !m.muted;
    }
    if keys.just_pressed(KeyCode::Minus) {
        m.volume = (m.volume - 0.1).max(0.0);
    }
    if keys.just_pressed(KeyCode::Equal) {
        m.volume = (m.volume + 0.1).min(1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn crossfade(time: Res<Time<Real>>, s: Res<Session>, screen: Res<State<Screen>>, m: Res<MusicSettings>, mut sinks: Query<(&MusicTrack, &mut AudioSink)>, mut last: Local<Option<Track>>, mut logged: Local<usize>, mut battles: Local<(bool, u32)>) {
    use shared::galaxy::{self, RiskTier};
    let now = time.elapsed_secs_f64();
    let playing = *screen.get() == Screen::Playing;
    // Battle music while an engagement is on, or shortly after shots were exchanged around us.
    let fighting = playing && (s.engagement.is_some() || now - s.last_combat < 10.0);
    // Each new fight switches battle theme, so long sessions don't hear the same loop every time.
    if fighting && !battles.0 {
        battles.1 += 1;
    }
    battles.0 = fighting;
    let dangerous = matches!(galaxy::system(s.system).tier, RiskTier::Wild | RiskTier::WarZone | RiskTier::Deep);
    let wanted = if fighting {
        if battles.1 % 2 == 1 { Track::Battle1 } else { Track::Battle2 }
    } else if playing && dangerous {
        Track::Tension
    } else if playing {
        [Track::Explore1, Track::Explore2, Track::Explore3][s.system as usize % 3]
    } else {
        Track::Explore1
    };
    if *last != Some(wanted) || *logged != sinks.iter().len() {
        info!("music: {:?} ({} track sinks live)", wanted, sinks.iter().len());
        *last = Some(wanted);
        *logged = sinks.iter().len();
    }
    let master = if m.muted { 0.0 } else { m.volume };
    let k = (time.delta_secs() / 2.5).min(1.0);
    for (t, mut sink) in &mut sinks {
        // Tracks are loudness-matched on disk; battle sits a little under the effects.
        let target = if t.0 == wanted { master * if matches!(t.0, Track::Battle1 | Track::Battle2) { 0.85 } else { 1.0 } } else { 0.0 };
        let cur = sink.volume().to_linear();
        let next = cur + (target - cur) * k * 3.0;
        if (next - cur).abs() > 1e-4 {
            sink.set_volume(Volume::Linear(next.max(0.0)));
        }
    }
}
