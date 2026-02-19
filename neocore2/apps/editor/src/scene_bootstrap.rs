#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight};
use newengine_scene::components::{ActiveCamera, SceneRoot};
use newengine_scene::{spawn_named, Scene, SceneState};
use newengine_transform::set_parent;

/// Editor bootstrap for a fresh scene.
///
/// `newengine-scene` must remain foundation-first; editor defaults live here.
#[inline]
pub fn bootstrap_editor_scene(scene: &mut Scene) {
    if scene.root().is_some() && scene.active_camera().is_some() {
        return;
    }

    let world = scene.world_mut();

    // Lighting defaults (editor-side): deterministic and minimal.
    // - Ambient is a world resource.
    // - Directional light is an entity (future: shadows, cascades, time-of-day).
    if world.resource::<AmbientLight>().is_none() {
        world.insert_resource(AmbientLight::default());
    }

    let root = spawn_named(world, "Root");
    world.insert(root, SceneRoot);

    // Default sun light.
    let sun = spawn_named(world, "Sun");
    world.insert(sun, DirectionalLight::default());
    set_parent(world, sun, Some(root));

    let cam = spawn_named(world, "Camera");
    world.insert(cam, ActiveCamera);

    set_parent(world, cam, Some(root));

    world.insert_resource(SceneState::new(Some(root), Some(cam)));
}
