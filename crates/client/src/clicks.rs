//! Injected mouse clicks for automated UI checks: `--clicks "x,y@t;x,y@t;…"` clicks the primary
//! window at pixel (x, y) at `t` seconds (left button, pressed then released on the next frame).
//! The clicks travel the same path as a real mouse (window events → egui), so buttons that only a
//! human would press — login, character creation — are exercised without anyone at the desk.

use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButton, MouseButtonInput};
use bevy::prelude::*;
use bevy::window::{CursorMoved, PrimaryWindow, WindowEvent};

#[derive(Resource, Default)]
pub struct Clicks {
    /// (time s, x, y), in order; consumed as they fire.
    pending: Vec<(f64, f32, f32)>,
    release: Option<Entity>,
}

pub fn parse(spec: &str) -> Clicks {
    let mut pending: Vec<(f64, f32, f32)> = spec
        .split(';')
        .filter_map(|c| {
            let (xy, t) = c.trim().split_once('@')?;
            let (x, y) = xy.split_once(',')?;
            Some((t.trim().parse().ok()?, x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect();
    pending.sort_by(|a, b| a.0.total_cmp(&b.0));
    Clicks { pending, release: None }
}

pub fn inject(mut clicks: ResMut<Clicks>, time: Res<Time<Real>>, window: Query<Entity, With<PrimaryWindow>>, mut events: MessageWriter<WindowEvent>) {
    let Ok(window) = window.single() else { return };
    if let Some(w) = clicks.release.take() {
        events.write(WindowEvent::MouseButtonInput(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Released, window: w }));
        return;
    }
    let now = time.elapsed_secs_f64();
    if clicks.pending.first().is_some_and(|c| c.0 <= now) {
        let (_, x, y) = clicks.pending.remove(0);
        info!("[clicks] click at ({x}, {y}) t={now:.2}");
        events.write(WindowEvent::CursorMoved(CursorMoved { window, position: Vec2::new(x, y), delta: None }));
        events.write(WindowEvent::MouseButtonInput(MouseButtonInput { button: MouseButton::Left, state: ButtonState::Pressed, window }));
        clicks.release = Some(window);
    }
}
