//! The character creation screen's ship: the hull, species style and paint being chosen, turning slowly
//! in the empty middle of the screen over a real star system. Built the way the showroom is — one
//! local snapshot — and gone when the game proper starts.

use crate::state::{Screen, Session, Snap};
use bevy::prelude::*;
use shared::galaxy::{self, Species};
use shared::protocol::{EntityKind, EntityState};
use shared::ships::{HullClass, Ship};

/// The system behind the creation screen (Sol Haven).
const SYSTEM: u16 = 0;

pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, preview);
    }
}

/// One id per hull, species and paint: another choice is another model, spawned afresh.
fn preview_id(hull: HullClass, species: Species, livery: u8) -> u32 {
    let h = HullClass::ALL.iter().position(|x| *x == hull).unwrap_or(0) as u32;
    let s = Species::ALL.iter().position(|x| *x == species).unwrap_or(0) as u32;
    1000 + h * 100 + s * 10 + livery as u32
}

fn preview(time: Res<Time<Real>>, screen: Res<State<Screen>>, form: Res<crate::ui::Form>, mut s: ResMut<Session>) {
    if *screen.get() != Screen::Create || s.me.is_some() {
        // In the game the server's snapshots have replaced it; anywhere else it must not linger.
        if s.preview.take().is_some() && *screen.get() != Screen::Playing {
            s.snaps.clear();
        }
        return;
    }
    if s.preview.is_none() {
        s.system = SYSTEM;
        s.system_epoch += 1;
    }
    let t = time.elapsed_secs_f64();
    let id = preview_id(form.hull, form.species, form.livery);
    let ship = Ship::new(String::new(), form.hull, form.livery);
    let st = ship.stats();
    // Between the star and the station: lit from the camera's side, the station far behind it.
    let pos = galaxy::system_layout(SYSTEM).station + Vec3::new(0.0, 400.0, -2600.0);
    let state = EntityState {
        id,
        kind: EntityKind::Player,
        name: String::new(),
        hull_class: Some(form.hull),
        rank: ship.rank(),
        livery: form.livery,
        pos: pos.to_array(),
        rot: Quat::from_rotation_y(t as f32 * 0.35).to_array(),
        vel: [0.0; 3],
        hull: 1.0,
        shield: 1.0,
        radius: st.radius,
        fleet: None,
        wanted: false,
        boosting: false,
        warping: false,
        mining: None,
        docked: false,
        owner: None,
        alliance: None,
        class: Some(ship.size_class()),
        species: Some(form.species),
    };
    let mut ents = view::interp::EntMap::default();
    ents.insert(id, state);
    s.preview = Some(id);
    s.snaps.clear();
    s.snaps.push_back(Snap { at: t, ents });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn app(screen: Screen) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin)).add_plugins(PreviewPlugin).insert_state(screen).init_resource::<crate::ui::Form>().insert_resource(Session::default());
        app.update();
        app
    }

    #[test]
    fn the_creation_screen_shows_the_chosen_ship_and_the_game_replaces_it() {
        let mut app = app(Screen::Create);
        let first = app.world().resource::<Session>().preview.expect("a ship on show");
        {
            let s = app.world().resource::<Session>();
            assert_eq!(s.me, None, "nobody is flying it");
            assert_eq!(s.system_epoch, 1, "the system behind it is built once");
            assert_eq!(s.snaps.back().unwrap().ents[&first].hull_class, Some(app.world().resource::<crate::ui::Form>().hull));
        }
        app.update();
        assert_eq!(app.world().resource::<Session>().system_epoch, 1);
        // Another hull is another model.
        app.world_mut().resource_mut::<crate::ui::Form>().hull = HullClass::Bastion;
        app.update();
        let second = app.world().resource::<Session>().preview.unwrap();
        assert_ne!(first, second);
        assert_eq!(app.world().resource::<Session>().snaps.back().unwrap().ents.len(), 1, "only the current choice");
        // The game begins: the server's snapshots stay.
        app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Playing);
        app.world_mut().resource_mut::<Session>().snaps.clear();
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Session>().preview, None);
    }

    #[test]
    fn other_screens_show_no_ship() {
        let app = app(Screen::Login);
        let s = app.world().resource::<Session>();
        assert!(s.preview.is_none() && s.snaps.is_empty());
    }
}
