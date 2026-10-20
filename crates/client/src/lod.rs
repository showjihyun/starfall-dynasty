//! Mesh levels of detail from the asset pipeline (`cargo run -p assetpipe -- MODEL.glb --lod R`,
//! docs/STUDIO_PIPELINE.md). A node `X__lod1` (a child of node `X`, same transform) holds the
//! simplified mesh and `extras.lod_m`: the distance from the model's box centre beyond which the
//! simplification error stays under a pixel at 1080p. When a model is spawned, `X`'s own primitives
//! get a `VisibilityRange` ending there and the LOD's primitives one starting there, with a short
//! dithered crossfade, scaled by the world scale of the node.
//! The ranges only decide what is drawn, so the simulation and its digests never see them.

use bevy::camera::visibility::VisibilityRange;
use bevy::gltf::GltfExtras;
use bevy::prelude::*;
use bevy::world_serialization::WorldInstanceReady;

pub const SUFFIX: &str = "__lod1";
/// Crossfade band past the switch distance, as a fraction of it.
const FADE: f32 = 0.1;

pub fn on_ready(ev: On<WorldInstanceReady>, mut commands: Commands) {
    let root = ev.entity;
    commands.queue(move |world: &mut World| {
        apply(world, root);
    });
}

/// Attaches the ranges under `root`; returns how many LOD nodes it wired.
pub fn apply(world: &mut World, root: Entity) -> usize {
    let mut lods = vec![];
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
        if world.get::<Name>(e).is_some_and(|n| n.as_str().ends_with(SUFFIX)) {
            let lod_m = world.get::<GltfExtras>(e).and_then(|x| serde_json::from_str::<serde_json::Value>(&x.value).ok()).and_then(|v| v["lod_m"].as_f64());
            if let (Some(lod_m), Some(parent)) = (lod_m, world.get::<ChildOf>(e).map(ChildOf::parent)) {
                lods.push((e, parent, lod_m as f32));
            }
        }
    }
    for &(lod, node, lod_m) in &lods {
        let d = lod_m * world_scale(world, node) * debug_scale();
        let fade = d..d * (1.0 + FADE);
        let detailed = VisibilityRange { start_margin: 0.0..0.0, end_margin: fade.clone(), use_aabb: true };
        let simple = VisibilityRange { start_margin: fade, end_margin: f32::MAX..f32::MAX, use_aabb: true };
        for (owner, range) in [(node, detailed), (lod, simple)] {
            let prims: Vec<Entity> = world.get::<Children>(owner).map(|c| c.iter().filter(|&c| c != lod && world.get::<Mesh3d>(c).is_some()).collect()).unwrap_or_default();
            for p in prims {
                world.entity_mut(p).insert(range.clone());
            }
        }
    }
    lods.len()
}

/// `STARFALL_LOD_SCALE=X` multiplies every switch distance (0 = always the simplified mesh), to look
/// at or diff the LODs: `STARFALL_LOD_SCALE=0 client --showroom 0 --shot lod.png`.
fn debug_scale() -> f32 {
    std::env::var("STARFALL_LOD_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0)
}

/// Product of the uniform scale of `e` and its ancestors (props are spawned under scaled parents).
fn world_scale(world: &World, mut e: Entity) -> f32 {
    let mut s = 1.0;
    loop {
        s *= world.get::<Transform>(e).map_or(1.0, |t| t.scale.max_element());
        match world.get::<ChildOf>(e) {
            Some(c) => e = c.parent(),
            None => return s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detailed_mesh_ends_where_the_lod_starts_scaled_by_the_parents() {
        let mut world = World::new();
        let mesh = || Mesh3d(Handle::default());
        let root = world.spawn(Transform::from_scale(Vec3::splat(2.0))).id();
        let node = world.spawn((Name::new("Ring"), Transform::default(), ChildOf(root))).id();
        let prim = world.spawn((mesh(), ChildOf(node))).id();
        let lod = world.spawn((Name::new("Ring__lod1"), GltfExtras { value: r#"{"lod_m":300.0}"#.into() }, ChildOf(node))).id();
        let lod_prim = world.spawn((mesh(), ChildOf(lod))).id();
        let other = world.spawn((Name::new("Hull"), ChildOf(root))).id();
        let other_prim = world.spawn((mesh(), ChildOf(other))).id();

        assert_eq!(apply(&mut world, root), 1);
        let r = world.get::<VisibilityRange>(prim).unwrap();
        assert_eq!((r.start_margin.clone(), r.end_margin.clone()), (0.0..0.0, 600.0..660.0), "300 m at scale 2");
        let r = world.get::<VisibilityRange>(lod_prim).unwrap();
        assert_eq!(r.start_margin, 600.0..660.0, "the LOD fades in exactly where the detailed mesh fades out");
        assert!(r.use_aabb && r.is_visible_at_all(1e9));
        assert!(world.get::<VisibilityRange>(other_prim).is_none(), "a mesh without a LOD is always drawn");
        assert!(world.get::<VisibilityRange>(lod).is_none(), "the LOD node itself is not a primitive");
    }
}
