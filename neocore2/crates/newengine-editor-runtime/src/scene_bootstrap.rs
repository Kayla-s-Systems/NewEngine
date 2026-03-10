#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight};
use newengine_scene::components::{ActiveCamera, SceneRoot};
use newengine_scene::{spawn_named, Scene, SceneState};
use newengine_transform::set_parent;

use crate::editor_camera::EditorCameraController;
use newengine_sim::CameraRigComp;

/// Editor bootstrap for a fresh scene.
///
/// `newengine-scene` must remain foundation-first; editor defaults live here.
#[inline]
pub fn bootstrap_editor_scene(scene: &mut Scene) {
    let root_hint = scene.root();
    let cam_hint = scene.active_camera();

    {
        let world = scene.world_mut();

        // Lighting defaults (editor-side): deterministic and minimal.
        // - Ambient is a world resource.
        // - Directional light is an entity (future: shadows, cascades, time-of-day).
        if world.resource::<AmbientLight>().is_none() {
            world.insert_resource(AmbientLight::default());
        }

        // Root: reuse when present.
        let root = root_hint.or_else(|| {
            world
                .query::<SceneRoot>()
                .next()
                .map(|(id, _)| id)
        });

        let root = match root {
            Some(id) if world.exists(id) => id,
            _ => {
                let r = spawn_named(world, "Root");
                let _ = world.insert(r, SceneRoot);
                r
            }
        };

        // Ensure root marker exists on the selected root.
        if world.get::<SceneRoot>(root).is_none() {
            let _ = world.insert(root, SceneRoot);
        }

        // Default sun light (only if no directional lights exist).
        if world.query::<DirectionalLight>().next().is_none() {
            let sun = spawn_named(world, "Sun");
            let _ = world.insert(sun, DirectionalLight::default());
            let _ = set_parent(world, sun, Some(root));
        }

        // Active camera: reuse when present.
        let cam = cam_hint.or_else(|| {
            world
                .query::<ActiveCamera>()
                .next()
                .map(|(id, _)| id)
        });

        let cam = match cam {
            Some(id) if world.exists(id) => id,
            _ => {
                let c = spawn_named(world, "Camera");
                let _ = world.insert(c, ActiveCamera);
                let _ = set_parent(world, c, Some(root));
                c
            }
        };

        // Ensure camera marker exists.
        if world.get::<ActiveCamera>(cam).is_none() {
            let _ = world.insert(cam, ActiveCamera);
        }

        // Editor camera controller composition (no renderer coupling).
        if world.get::<CameraRigComp>(cam).is_none() {
            let _ = world.insert(cam, CameraRigComp::default());
        }
        if world.get::<EditorCameraController>(cam).is_none() {
            let _ = world.insert(cam, EditorCameraController::default());
        }

        // Ensure camera is rooted for deterministic editor navigation.
        let _ = set_parent(world, cam, Some(root));

        // Keep SceneState consistent with markers.
        if let Some(st) = world.resource_mut::<SceneState>() {
            st.root = Some(root);
            st.active_camera = Some(cam);
        } else {
            world.insert_resource(SceneState::new(Some(root), Some(cam)));
        }
    }

    // Let scene reconcile duplicates deterministically (if user data created any).
    let _ = scene.validate_invariants();
}
