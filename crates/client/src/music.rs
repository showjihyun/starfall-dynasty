//! Adaptive background music (CC0 tracks, see assets/music/CREDITS.md).
//! Exploration themes alternate by star system; the battle theme crossfades in while fighting.

use crate::state::{Screen, Session};
use bevy::audio::{AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Track {
    Explore1,
    Explore2,
    Battle,
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
    for (t, file) in [(Track::Explore1, "music/explore_observing_the_star.ogg"), (Track::Explore2, "music/explore_galactic_temple.ogg"), (Track::Battle, "music/battle_determined_pursuit.ogg")] {
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

fn crossfade(time: Res<Time<Real>>, s: Res<Session>, screen: Res<State<Screen>>, m: Res<MusicSettings>, mut sinks: Query<(&MusicTrack, &mut AudioSink)>, mut last: Local<Option<Track>>, mut logged: Local<usize>) {
    let now = time.elapsed_secs_f64();
    // Battle music while an engagement is on, or shortly after shots were exchanged around us.
    let fighting = *screen.get() == Screen::Playing && (s.engagement.is_some() || now - s.last_combat < 10.0);
    let wanted = if fighting {
        Track::Battle
    } else if *screen.get() == Screen::Playing && s.system % 2 == 1 {
        Track::Explore2
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
        let target = if t.0 == wanted { master * if t.0 == Track::Battle { 0.8 } else { 1.0 } } else { 0.0 };
        let cur = sink.volume().to_linear();
        let next = cur + (target - cur) * k * 3.0;
        if (next - cur).abs() > 1e-4 {
            sink.set_volume(Volume::Linear(next.max(0.0)));
        }
    }
}
