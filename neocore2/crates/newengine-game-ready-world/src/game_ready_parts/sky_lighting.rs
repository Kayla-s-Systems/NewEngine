use super::*;

#[inline]
pub(crate) fn configure_game_ready_lighting(
    world: &mut newengine_ecs::World,
    environment_parent: EntityId,
    spec: &GameReadyLightingSpec,
) {
    let ambient = AmbientLight {
        color: spec.ambient_color,
        intensity: spec.ambient_intensity,
    };
    match world.resource_mut::<AmbientLight>() {
        Some(a) => *a = ambient,
        None => world.insert_resource(ambient),
    }

    let sun_dir = Vec3::new(
        spec.sun_direction[0],
        spec.sun_direction[1],
        spec.sun_direction[2],
    )
    .normalize_or_zero();
    let sun = DirectionalLight {
        direction_ws: [sun_dir.x, sun_dir.y, sun_dir.z],
        color: spec.sun_color,
        intensity: spec.sun_intensity,
    };
    let sun_entity = newengine_engine_runtime::gameplay::scene_entity_by_role(
        world,
        newengine_engine_runtime::gameplay::SceneEntityRole::Sun,
    );
    let sun_entity = if let Some(sun_entity) = sun_entity {
        if let Some(light) = world.get_mut_tracked::<DirectionalLight>(sun_entity) {
            *light = sun;
        } else {
            let _ = world.insert(sun_entity, sun);
        }
        sun_entity
    } else {
        let sun_entity = spawn_named(world, "Scene/Environment/Sun");
        let _ = world.insert(sun_entity, sun);
        sun_entity
    };
    let _ = set_parent(world, sun_entity, Some(environment_parent));
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        sun_entity,
        newengine_engine_runtime::gameplay::SceneEntityRole::Sun,
        "Scene/Environment/Sun",
        Vec3::ZERO,
        Vec3::splat(0.5),
    );

    let sky_cycle_anchor = spawn_named(world, "Scene/Environment/SkyCycle");
    let _ = set_parent(world, sky_cycle_anchor, Some(environment_parent));
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        sky_cycle_anchor,
        newengine_engine_runtime::gameplay::SceneEntityRole::SkyCycle,
        "Scene/Environment/SkyCycle",
        Vec3::ZERO,
        Vec3::splat(0.35),
    );

    sync_game_ready_day_night_to_engine_time(&spec.day_night);

    world.insert_resource(SkyCycleRuntime {
        anchor: Some(sky_cycle_anchor),
        sun: Some(sun_entity),
        enabled: spec.day_night.enabled,
        time_of_day_hours: spec.day_night.time_of_day_hours,
        day_length_seconds: spec.day_night.day_length_seconds,
        latitude_degrees: spec.day_night.latitude_degrees,
        axial_tilt_degrees: spec.day_night.axial_tilt_degrees,
        base_sun_color: spec.sun_color,
        base_sun_intensity: spec.sun_intensity,
        base_ambient_color: spec.ambient_color,
        base_ambient_intensity: spec.ambient_intensity,
        day_index: u64::from(spec.day_night.day_of_year.saturating_sub(1)),
    });
    tick_game_ready_sky_cycle(world, 0.0);

    newengine_ulog_api::ulog::info!(
        "game-ready sky cycle: tod={:.2}h day_of_year={} day_len={:.1}s ambient={:?}/{:.3} sun_dir={:?} sun={:?}/{:.3} shadows={} strength={:.3} cascades={} resolution={} max_distance={:.1}m filter={:?} pcss=[angle={:.3}deg blocker_radius={:.2}px max_filter={:.2}px blocker_samples={} filter_samples={} min_filter={:.2}px stable_cell={:.2}px] sun_entity={:?} sky_cycle_anchor={:?} policy='scene-owned semantic anchors are entities'",
        spec.day_night.time_of_day_hours,
        spec.day_night.day_of_year,
        spec.day_night.day_length_seconds,
        ambient.color,
        ambient.intensity,
        sun.direction_ws,
        sun.color,
        sun.intensity,
        spec.shadows.enabled,
        spec.shadows.contact_strength,
        spec.shadows.cascade_count,
        spec.shadows.resolution,
        spec.shadows.max_distance,
        spec.shadows.filter,
        spec.shadows.pcss.light_angular_radius_degrees,
        spec.shadows.pcss.blocker_search_radius_texels,
        spec.shadows.pcss.max_filter_radius_texels,
        spec.shadows.pcss.blocker_samples,
        spec.shadows.pcss.filter_samples,
        spec.shadows.pcss.min_filter_radius_texels,
        spec.shadows.pcss.stable_kernel_cell_texels,
        sun_entity,
        sky_cycle_anchor,
    );

    world.insert_resource(ShadowSettings {
        enabled: spec.shadows.enabled,
        method: if spec.shadows.cascade_count > 1 {
            newengine_lighting::ShadowMethod::CascadedShadowMaps
        } else {
            newengine_lighting::ShadowMethod::DirectionalDepthMap
        },
        filter: spec.shadows.filter,
        resolution: spec.shadows.resolution,
        cascade_count: spec.shadows.cascade_count,
        max_distance: spec.shadows.max_distance,
        softness: spec.shadows.softness,
        bias: spec.shadows.bias,
        normal_bias: spec.shadows.normal_bias,
        contact_strength: spec.shadows.contact_strength,
        pcss: spec.shadows.pcss,
    });

    world.insert_resource(
        newengine_lighting::LocalShadowSettings {
            enabled: spec.shadows.enabled,
            point_enabled: true,
            spot_enabled: true,
            max_shadowed_lights: 4,
            max_resolution: spec.shadows.resolution.min(2048).max(512),
            min_resolution: 256,
            max_distance: spec.shadows.max_distance.min(96.0).max(8.0),
            bias: (spec.shadows.bias * 0.85).clamp(0.0002, 0.02),
            normal_bias: spec.shadows.normal_bias.clamp(0.0, 0.25),
            strength: 1.0,
        }
        .sanitized(),
    );
}
