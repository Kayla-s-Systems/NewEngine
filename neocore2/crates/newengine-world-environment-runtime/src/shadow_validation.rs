use crate::authored_foliage::terrain_height;
use crate::authored_materials::AuthoredEnvironmentMaterials;
use crate::authored_sky::SkyCycleRuntime;
use newengine_ecs::EntityId;
use newengine_engine_runtime::world_authoring::{
    spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
};
use newengine_lighting::{
    DirectionalLight, LocalShadowSettings, PointLight, ShadowFilter, ShadowMethod, ShadowSettings,
    SpotLight,
};
use newengine_materials::{MaterialDescriptor, MaterialFlags};
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_model_domain_api::MeshRenderOptions;
use newengine_primitives::{builtins, PrimitiveId, PrimitiveRegistry};
use newengine_scene::spawn_named;
use newengine_sim::{AngularVelocity, Velocity};
use newengine_transform::{set_parent, Transform};

const TORTURE_LAUNCH_ID: &str = "shadow_torture";
const TORTURE_CYCLE_SECONDS: f32 = 12.0;
const TORTURE_ANIMATED_SECONDS: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
struct ShadowValidationRuntime {
    sun: EntityId,
    player: EntityId,
    moving_caster: EntityId,
    moving_base: Vec3,
    elapsed: f32,
    frozen: bool,
    frozen_player_transform: Option<Transform>,
    player_controller_was_enabled: bool,
    interactive: bool,
}

#[inline]
fn env_enabled(name: &str) -> bool {
    matches!(
        newengine_runtime_env::var(name).as_deref(),
        Some("1")
            | Some("true")
            | Some("TRUE")
            | Some("yes")
            | Some("YES")
            | Some("on")
            | Some("ON")
    )
}

#[inline]
pub fn requested() -> bool {
    newengine_runtime_env::var(newengine_project_api::PROJECT_LAUNCH_PRESET_ENV)
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(TORTURE_LAUNCH_ID))
        || env_enabled("NEWENGINE_SHADOW_TORTURE_TEST")
}

#[inline]
fn receiver_material(mats: &MaterialRegistry, receive_shadows: bool) -> MaterialId {
    let flags = if receive_shadows {
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS)
    } else {
        MaterialFlags::CAST_SHADOWS
    };
    mats.upsert_named(
        if receive_shadows {
            "ShadowValidation/Receiver"
        } else {
            "ShadowValidation/NoReceive"
        },
        MaterialDescriptor {
            base_color: if receive_shadows {
                [0.72, 0.76, 0.82, 1.0]
            } else {
                [0.86, 0.28, 0.78, 1.0]
            },
            roughness: 0.78,
            metallic: 0.0,
            flags,
            ..MaterialDescriptor::default()
        },
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn spawn_fixture(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    material_id: MaterialId,
    primitive_id: PrimitiveId,
    name: &str,
    position: Vec3,
    scale: Vec3,
    color: [f32; 4],
) -> EntityId {
    spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent,
            primitive_id,
            material_id,
            name,
            position,
            scale,
            color,
            render_options: MeshRenderOptions::world_opaque(),
        },
    )
}

fn spawn_local_lights(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    origin: Vec3,
) -> (u32, u32) {
    let points = [
        (
            "ShadowValidation/Point/Warm",
            origin + Vec3::new(-4.0, 4.5, 8.0),
            [1.0, 0.52, 0.28],
            18.0,
            13.0,
        ),
        (
            "ShadowValidation/Point/Cool",
            origin + Vec3::new(4.5, 3.6, 14.0),
            [0.32, 0.58, 1.0],
            16.0,
            12.0,
        ),
    ];
    for (name, position, color, intensity, range) in points {
        let entity = spawn_named(world, name);
        let _ = set_parent(world, entity, Some(parent));
        let _ = world.insert(
            entity,
            PointLight {
                color,
                intensity,
                range,
            },
        );
        newengine_engine_runtime::gameplay::attach_scene_object_core(
            world,
            entity,
            position,
            Vec3::splat(0.25),
        );
        if let Some(transform) = world.get_mut_tracked::<Transform>(entity) {
            transform.position = position;
        }
    }

    let spots = [
        (
            "ShadowValidation/Spot/Left",
            origin + Vec3::new(-7.0, 7.5, 20.0),
            origin + Vec3::new(-1.0, 0.7, 25.0),
            [1.0, 0.88, 0.64],
        ),
        (
            "ShadowValidation/Spot/Right",
            origin + Vec3::new(7.0, 6.5, 30.0),
            origin + Vec3::new(1.0, 0.6, 34.0),
            [0.56, 0.72, 1.0],
        ),
    ];
    for (name, position, target, color) in spots {
        let direction = (target - position).normalize_or_zero();
        let entity = spawn_named(world, name);
        let _ = set_parent(world, entity, Some(parent));
        let _ = world.insert(
            entity,
            SpotLight {
                direction_ws: [direction.x, direction.y, direction.z],
                color,
                intensity: 22.0,
                range: 20.0,
                outer_angle_rad: 0.62,
                inner_angle_rad: 0.40,
            },
        );
        newengine_engine_runtime::gameplay::attach_scene_object_core(
            world,
            entity,
            position,
            Vec3::splat(0.25),
        );
        if let Some(transform) = world.get_mut_tracked::<Transform>(entity) {
            transform.position = position;
        }
    }
    (points.len() as u32, spots.len() as u32)
}

pub fn bootstrap_shadow_validation_if_requested(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    terrain: EntityId,
    materials: AuthoredEnvironmentMaterials,
    player_start: Vec3,
) {
    if !requested() || world.resource::<ShadowValidationRuntime>().is_some() {
        return;
    }

    let Some(sun) = newengine_engine_runtime::gameplay::scene_entity_by_role(
        world,
        newengine_engine_runtime::gameplay::SceneEntityRole::Sun,
    ) else {
        newengine_ulog_api::ulog::warn!(
            "shadow validation: requested but scene has no semantic Sun entity; fixture disabled"
        );
        return;
    };

    let Some(player) = newengine_engine_runtime::gameplay::scene_entity_by_role(
        world,
        newengine_engine_runtime::gameplay::SceneEntityRole::Player,
    ) else {
        newengine_ulog_api::ulog::warn!(
            "shadow validation: requested but scene has no semantic Player entity; fixture disabled"
        );
        return;
    };
    let player_controller_was_enabled = world
        .get::<newengine_engine_runtime::gameplay::PlayerController>(player)
        .map(|controller| controller.enabled)
        .unwrap_or(true);
    let interactive = env_enabled("NEWENGINE_SHADOW_TORTURE_INTERACTIVE");
    if !interactive {
        if let Some(controller) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerController>(player)
        {
            controller.enabled = false;
        }
        newengine_engine_runtime::gameplay::clear_player_input(world, player);
    }

    if let Some(cycle) = world.resource_mut::<SkyCycleRuntime>() {
        // The torture controller owns directional motion. Normal day/night would otherwise
        // rewrite the same DirectionalLight after the fixture updated it.
        cycle.enabled = false;
    }

    world.insert_resource(ShadowSettings {
        enabled: true,
        method: ShadowMethod::CascadedShadowMaps,
        filter: ShadowFilter::Pcss,
        resolution: 2048,
        cascade_count: 4,
        max_distance: 96.0,
        softness: 1.0,
        bias: 0.0016,
        normal_bias: 0.010,
        contact_strength: 0.35,
        pcss: newengine_lighting::ShadowPcssSettings {
            light_angular_radius_degrees: 0.27,
            blocker_search_radius_texels: 5.0,
            max_filter_radius_texels: 12.0,
            blocker_samples: 12,
            filter_samples: 16,
            min_filter_radius_texels: 0.75,
            stable_kernel_cell_texels: 4.0,
        }
        .sanitized(),
    });
    world.insert_resource(
        LocalShadowSettings {
            enabled: true,
            point_enabled: true,
            spot_enabled: true,
            max_shadowed_lights: 4,
            max_resolution: 1024,
            min_resolution: 256,
            max_distance: 72.0,
            bias: 0.0015,
            normal_bias: 0.010,
            strength: 1.0,
        }
        .sanitized(),
    );

    let receiver = receiver_material(mats, true);
    let no_receive = receiver_material(mats, false);
    let base_y = terrain_height(world, terrain, player_start.x, player_start.z) + 0.08;
    let origin = Vec3::new(player_start.x, base_y, player_start.z);

    // A flat diagnostic runway guarantees stable receivers even where authored terrain becomes
    // irregular. Distinct segments keep the CSM distance bands visually readable.
    for (index, z) in [4.0_f32, 14.0, 32.0, 64.0].into_iter().enumerate() {
        spawn_fixture(
            world,
            prims,
            mats,
            parent,
            receiver,
            builtins::ID_CUBE,
            &format!("ShadowValidation/ReceiverBand/{index}"),
            origin + Vec3::new(0.0, -0.16, z),
            Vec3::new(12.0, 0.18, if index == 3 { 22.0 } else { 14.0 }),
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    // Thin vertical geometry is deliberately hostile to both depth bias and distant caster LOD.
    for (index, z) in [5.0_f32, 13.0, 28.0, 58.0].into_iter().enumerate() {
        spawn_fixture(
            world,
            prims,
            mats,
            parent,
            receiver,
            builtins::ID_CUBE,
            &format!("ShadowValidation/ThinCaster/{index}"),
            origin + Vec3::new(-2.8 + index as f32 * 1.8, 1.55, z),
            Vec3::new(0.08, 3.0 + index as f32 * 0.45, 0.30),
            [0.96, 0.76, 0.30, 1.0],
        );
    }

    // Dedicated receiver-policy probe: casts a shadow, but must not sample either directional
    // or local shadow visibility itself.
    spawn_fixture(
        world,
        prims,
        mats,
        parent,
        no_receive,
        builtins::ID_CUBE,
        "ShadowValidation/NoReceiveProbe",
        origin + Vec3::new(3.4, 0.85, 8.0),
        Vec3::new(1.25, 1.7, 1.25),
        [1.0, 1.0, 1.0, 1.0],
    );

    // Reuse the authored maple leaf material so alpha-cutout shadow depth exercises the real
    // project texture/material path instead of a synthetic alpha mask.
    let alpha_card = spawn_fixture(
        world,
        prims,
        mats,
        parent,
        materials.tree_leaf,
        builtins::ID_PLANE,
        "ShadowValidation/AlphaCutoutCard",
        origin + Vec3::new(0.0, 1.8, 10.5),
        Vec3::new(4.5, 1.0, 4.5),
        [1.0, 1.0, 1.0, 1.0],
    );
    if let Some(transform) = world.get_mut_tracked::<Transform>(alpha_card) {
        transform.rotation =
            Quat::from_euler(EulerRot::XYZ, core::f32::consts::FRAC_PI_2, 0.0, 0.0);
    }

    let moving_base = origin + Vec3::new(0.0, 1.4, 18.0);
    let moving_caster = spawn_fixture(
        world,
        prims,
        mats,
        parent,
        receiver,
        builtins::ID_CUBE,
        "ShadowValidation/DynamicCaster",
        moving_base,
        Vec3::new(1.1, 2.4, 1.1),
        [0.28, 0.88, 0.58, 1.0],
    );

    let (point_count, spot_count) = spawn_local_lights(world, parent, origin);
    world.insert_resource(ShadowValidationRuntime {
        sun,
        player,
        moving_caster,
        moving_base,
        elapsed: 0.0,
        frozen: false,
        frozen_player_transform: None,
        player_controller_was_enabled,
        interactive,
    });

    newengine_ulog_api::ulog::info!(
        "shadow validation: enabled launch='{}' csm(cascades=4 resolution=2048 max_distance=96m filter=PCSS) local(points={} spots={} max_shadowed=4 max_resolution=1024) fixtures='receiver bands + thin casters + alpha-cutout + no-receive + dynamic caster' cycle='8s animated + 4s frozen cache window' input_mode='{}'",
        TORTURE_LAUNCH_ID,
        point_count,
        spot_count,
        if interactive { "interactive" } else { "deterministic" },
    );
}

pub fn tick_shadow_validation(world: &mut newengine_ecs::World, dt: f32) {
    let Some(runtime) = world.resource::<ShadowValidationRuntime>().copied() else {
        return;
    };
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    };
    let elapsed = (runtime.elapsed + dt).rem_euclid(TORTURE_CYCLE_SECONDS);
    let frozen = elapsed >= TORTURE_ANIMATED_SECONDS;
    let phase_changed = frozen != runtime.frozen || elapsed < runtime.elapsed;

    let entering_frozen = phase_changed && frozen;
    let leaving_frozen = phase_changed && !frozen;
    let captured_player_transform = entering_frozen
        .then(|| world.get::<Transform>(runtime.player).copied())
        .flatten();

    if let Some(state) = world.resource_mut::<ShadowValidationRuntime>() {
        state.elapsed = elapsed;
        state.frozen = frozen;
        if entering_frozen {
            state.frozen_player_transform = captured_player_transform;
        } else if leaving_frozen {
            state.frozen_player_transform = None;
        }
    }

    if entering_frozen {
        if let Some(controller) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerController>(runtime.player)
        {
            controller.enabled = false;
        }
        newengine_engine_runtime::gameplay::clear_player_input(world, runtime.player);
    } else if leaving_frozen && runtime.interactive {
        if let Some(controller) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerController>(runtime.player)
        {
            controller.enabled = runtime.player_controller_was_enabled;
        }
    } else if !runtime.interactive {
        if let Some(controller) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerController>(runtime.player)
        {
            controller.enabled = false;
        }
        newengine_engine_runtime::gameplay::clear_player_input(world, runtime.player);
    }

    if phase_changed {
        newengine_ulog_api::ulog::info!(
            "shadow validation: phase={} elapsed={:.2}s expectation='{}'",
            if frozen {
                "cache-window"
            } else {
                "animated-stress"
            },
            elapsed,
            if frozen {
                "frozen player/camera should reuse CSM/local atlas and omit both shadow graph passes"
            } else {
                "sun/dynamic caster movement must invalidate the affected shadow caches"
            },
        );
    }

    if frozen {
        if let Some(transform) = runtime
            .frozen_player_transform
            .or(captured_player_transform)
        {
            if let Some(current) = world.get_mut::<Transform>(runtime.player) {
                *current = transform;
            }
        }
        if let Some(velocity) = world.get_mut::<Velocity>(runtime.player) {
            velocity.0 = Vec3::ZERO;
        }
        if let Some(angular_velocity) = world.get_mut::<AngularVelocity>(runtime.player) {
            angular_velocity.0 = Vec3::ZERO;
        }
        newengine_engine_runtime::gameplay::clear_player_input(world, runtime.player);
        return;
    }

    let angle = elapsed * 0.16;
    let (sin_a, cos_a) = angle.sin_cos();
    let direction = Vec3::new(cos_a * 0.52, -0.82, sin_a * 0.52).normalize_or_zero();
    if let Some(light) = world.get_mut_tracked::<DirectionalLight>(runtime.sun) {
        light.direction_ws = [direction.x, direction.y, direction.z];
    }

    if let Some(transform) = world.get_mut_tracked::<Transform>(runtime.moving_caster) {
        transform.position = runtime.moving_base
            + Vec3::new(
                (elapsed * 1.35).sin() * 2.25,
                (elapsed * 2.1).sin() * 0.45,
                (elapsed * 0.72).cos() * 0.65,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torture_cycle_has_deterministic_cache_window() {
        const {
            assert!(TORTURE_ANIMATED_SECONDS > 0.0);
            assert!(TORTURE_CYCLE_SECONDS > TORTURE_ANIMATED_SECONDS);
        }
        assert_eq!(TORTURE_CYCLE_SECONDS - TORTURE_ANIMATED_SECONDS, 4.0);
    }
}
