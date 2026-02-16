#![forbid(unsafe_op_in_unsafe_fn)]

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

    let root = spawn_named(world, "Root");
    world.insert(root, SceneRoot);

    let cam = spawn_named(world, "Camera");
    world.insert(cam, ActiveCamera);

    set_parent(world, cam, Some(root));

    world.insert_resource(SceneState::new(Some(root), Some(cam)));
}
