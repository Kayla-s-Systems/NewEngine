#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, LocalShadowSettings, ShadowSettings};
use newengine_math::Vec3;
use newengine_scene::components::{ActiveCamera, SceneRoot};
use newengine_scene::{spawn_named, Scene, SceneState};
use newengine_transform::set_parent;

use newengine_camera::RuntimeNavController;
use newengine_camera_runtime::CameraManagerResource;
use newengine_sim::CameraRigComp;

use newengine_gameplay_world_runtime::gameplay::{attach_scene_element_core, SceneEntityRole};

/// Runtime bootstrap for a fresh scene.
///
/// `newengine-scene` remains foundation-first; app/runtime defaults live here.
#[inline]
pub fn bootstrap_runtime_scene(scene: &mut Scene) {
    let root_hint = scene.root();
    let cam_hint = scene.active_camera();

    {
        let world = scene.world_mut();

        // Lighting defaults (runtime-side): deterministic and minimal.
        // - Ambient is a world resource.
        // - Directional light is an entity (future: shadows, cascades, time-of-day).
        if world.resource::<AmbientLight>().is_none() {
            world.insert_resource(AmbientLight::default());
        }
        if world.resource::<ShadowSettings>().is_none() {
            world.insert_resource(ShadowSettings::default());
        }
        if world.resource::<LocalShadowSettings>().is_none() {
            world.insert_resource(LocalShadowSettings::default());
        }

        // Root: reuse when present.
        let root = root_hint.or_else(|| world.query::<SceneRoot>().next().map(|(id, _)| id));

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

        // Default sun light is a scene entity, not a scene side-channel.
        if newengine_gameplay_world_runtime::gameplay::scene_entity_by_role(
            world,
            SceneEntityRole::Sun,
        )
        .is_none()
        {
            let sun = spawn_named(world, "Sun");
            let _ = world.insert(sun, DirectionalLight::default());
            let _ = set_parent(world, sun, Some(root));
            attach_scene_element_core(
                world,
                sun,
                SceneEntityRole::Sun,
                "Scene/Environment/Sun",
                Vec3::ZERO,
                Vec3::splat(0.5),
            );
        }

        // Active camera: reuse when present.
        let cam = cam_hint.or_else(|| world.query::<ActiveCamera>().next().map(|(id, _)| id));

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
        attach_scene_element_core(
            world,
            cam,
            SceneEntityRole::ActiveCamera,
            "Scene/Cameras/ActiveCamera",
            Vec3::ZERO,
            Vec3::splat(0.35),
        );

        // Runtime camera controller composition (no renderer coupling).
        if world.get::<CameraRigComp>(cam).is_none() {
            let _ = world.insert(cam, CameraRigComp::default());
        }
        if world.get::<RuntimeNavController>(cam).is_none() {
            let _ = world.insert(cam, RuntimeNavController::default());
        }
        if world.resource::<CameraManagerResource>().is_none() {
            world.insert_resource(CameraManagerResource::default());
        }

        // Ensure camera is rooted for deterministic runtime navigation.
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

/// Runtime scene foundation without manufacturing an active camera.
///
/// Product-owned worlds use this when camera identity is authored by project data. The engine
/// installs only generic scene resources/root/light infrastructure; the product must explicitly
/// create and select its camera before rendering begins.
pub fn bootstrap_runtime_scene_foundation(scene: &mut Scene) {
    let root_hint = scene.root();
    {
        let world = scene.world_mut();
        if world.resource::<AmbientLight>().is_none() {
            world.insert_resource(AmbientLight::default());
        }
        if world.resource::<ShadowSettings>().is_none() {
            world.insert_resource(ShadowSettings::default());
        }
        if world.resource::<LocalShadowSettings>().is_none() {
            world.insert_resource(LocalShadowSettings::default());
        }

        let root = root_hint.or_else(|| world.query::<SceneRoot>().next().map(|(id, _)| id));
        let root = match root {
            Some(id) if world.exists(id) => id,
            _ => {
                let root = spawn_named(world, "Root");
                let _ = world.insert(root, SceneRoot);
                root
            }
        };
        if world.get::<SceneRoot>(root).is_none() {
            let _ = world.insert(root, SceneRoot);
        }

        if newengine_gameplay_world_runtime::gameplay::scene_entity_by_role(
            world,
            SceneEntityRole::Sun,
        )
        .is_none()
        {
            let sun = spawn_named(world, "Sun");
            let _ = world.insert(sun, DirectionalLight::default());
            let _ = set_parent(world, sun, Some(root));
            attach_scene_element_core(
                world,
                sun,
                SceneEntityRole::Sun,
                "Scene/Environment/Sun",
                Vec3::ZERO,
                Vec3::splat(0.5),
            );
        }
        if world.resource::<CameraManagerResource>().is_none() {
            world.insert_resource(CameraManagerResource::default());
        }
        if let Some(state) = world.resource_mut::<SceneState>() {
            state.root = Some(root);
            state.active_camera = None;
        } else {
            world.insert_resource(SceneState::new(Some(root), None));
        }
    }
    let _ = scene.validate_invariants();
}
