#[inline]
fn game_data_color(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let mut out = value;
    for (channel, fallback_channel) in out.iter_mut().zip(fallback) {
        if !channel.is_finite() {
            *channel = fallback_channel;
        }
        *channel = channel.clamp(0.0, 1.0);
    }
    out
}

#[inline]
fn game_data_direction(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let candidate = Vec3::new(value[0], value[1], value[2]);
    let direction = if candidate.is_finite() && candidate.length_squared() > 1.0e-6 {
        candidate.normalize_or_zero()
    } else {
        Vec3::new(fallback[0], fallback[1], fallback[2]).normalize_or_zero()
    };
    [direction.x, direction.y, direction.z]
}

#[inline]
fn game_data_shadow_filter(value: &str) -> newengine_lighting::ShadowFilter {
    match value.trim().to_ascii_lowercase().as_str() {
        "hard" | "none" => newengine_lighting::ShadowFilter::Hard,
        "pcf" => newengine_lighting::ShadowFilter::Pcf,
        "pcss" => newengine_lighting::ShadowFilter::Pcss,
        _ => newengine_lighting::ShadowFilter::Pcf,
    }
}

/// Provider-produced GameData is authoritative for runtime lighting policy.
/// YMAP/YTYP still owns scene content/metadata, but built-in FPS authored defaults
/// must not silently replace project shadow/day-night settings.
#[inline]
fn game_data_sky_spec(data: &GameData, fallback: &AuthoredFpsSkySpec) -> AuthoredFpsSkySpec {
    let sky = &data.world.sky;
    let definition_ref = sky.definition_ref.trim().replace('\\', "/");
    let mesh = sky.mesh.trim().replace('\\', "/");
    let cloud_dictionary = sky.cloud_dictionary.trim().replace('\\', "/");
    let moon_texture = sky.moon_texture.trim().replace('\\', "/");
    AuthoredFpsSkySpec {
        definition_ref: if definition_ref.is_empty() {
            fallback.definition_ref.clone()
        } else {
            definition_ref
        },
        render_options: fallback.render_options.clone(),
        radius: sky.radius.max(0.1),
        // Asset identity belongs to the selected YTYP graph. Empty GameData fields
        // mean "use the selected sky definition", not "erase the resolved asset".
        mesh: if mesh.is_empty() {
            fallback.mesh.clone()
        } else {
            mesh
        },
        follow_camera: sky.follow_camera,
        environment_profile: sky.environment_profile.trim().to_owned(),
        environment_region: sky.environment_region.trim().to_owned(),
        environment_biome: sky.environment_biome.trim().to_owned(),
        cloud_dictionary: if cloud_dictionary.is_empty() {
            fallback.cloud_dictionary.clone()
        } else {
            cloud_dictionary
        },
        cloud_profile: sky.cloud_profile.trim().to_owned(),
        sun_radius: sky.sun_radius.max(0.1),
        moon_radius: sky.moon_radius.max(0.1),
        moon_texture: if moon_texture.is_empty() {
            fallback.moon_texture.clone()
        } else {
            moon_texture
        },
        atmosphere: AuthoredFpsSkyAtmosphereSpec {
            day_zenith: sky.atmosphere.day_zenith,
            day_horizon: sky.atmosphere.day_horizon,
            dusk_zenith: sky.atmosphere.dusk_zenith,
            dusk_horizon: sky.atmosphere.dusk_horizon,
            night_zenith: sky.atmosphere.night_zenith,
            night_horizon: sky.atmosphere.night_horizon,
            cloud_day: sky.atmosphere.cloud_day,
            cloud_night: sky.atmosphere.cloud_night,
            night_sky_strength: sky.atmosphere.night_sky_strength.max(0.0),
            cloud_coverage: sky.atmosphere.cloud_coverage.clamp(0.0, 1.0),
            cloud_softness: sky.atmosphere.cloud_softness.clamp(0.0, 1.0),
        },
    }
}

fn install_game_data_sky_definition(map: &mut AuthoredWorldProfile, data: &GameData) {
    let previous_sky_ref = map.sky.definition_ref.trim().replace('\\', "/");
    map.sky = game_data_sky_spec(data, &map.sky);
    let definition_ref = map.sky.definition_ref.trim();
    if definition_ref.is_empty() {
        return;
    }
    if !previous_sky_ref.is_empty() && previous_sky_ref != definition_ref {
        map.definitions.retain(|spec| {
            spec.apply_mode != AuthoredFpsDefinitionApplyMode::MetadataOnly
                || spec.definition_ref != previous_sky_ref
        });
    }
    if !map
        .definitions
        .iter()
        .any(|spec| spec.definition_ref == definition_ref)
    {
        map.definitions.push(AuthoredFpsDefinitionInstanceSpec {
            definition_ref: definition_ref.to_owned(),
            position: Vec3::ZERO,
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: Vec3::ONE,
            apply_mode: AuthoredFpsDefinitionApplyMode::MetadataOnly,
        });
    }
}

fn install_game_data_player_definition(map: &mut AuthoredWorldProfile, data: &GameData) {
    let map_owns_avatar = map.player.model.enabled && !map.player.model.source.trim().is_empty();
    if map_owns_avatar {
        return;
    }

    let definition_ref = data.player.character_ref.trim().replace('\\', "/");
    if definition_ref.is_empty() {
        return;
    }
    if !map
        .definitions
        .iter()
        .any(|spec| spec.definition_ref == definition_ref)
    {
        map.definitions.push(AuthoredFpsDefinitionInstanceSpec {
            definition_ref: definition_ref.clone(),
            position: Vec3::ZERO,
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: Vec3::ONE,
            apply_mode: AuthoredFpsDefinitionApplyMode::MetadataOnly,
        });
    }
    newengine_ulog_api::ulog::info!(
        "fps-authored player character selection: definition_ref='{}' policy='every game has a playable visual character; Shared preset hydrates model and character tuning'",
        definition_ref
    );
}

fn install_game_data_player_input_policy(profile: &mut AuthoredWorldProfile, data: &GameData) {
    // GameData V2 owns only project-level player input policy here. Character model, body,
    // movement speeds and locomotion tuning are definition-owned and are hydrated from the
    // selected character YTYP below. Never clamp the V2 runtime-resolved sentinel fields into
    // synthetic character defaults.
    profile.player.look_sens = data.player.look_sensitivity;
    newengine_ulog_api::ulog::info!(
        "fps-authored game-data player input policy: look_sensitivity={:.6} character_ref='{}' policy='GameData selects character/input policy; YMAP owns spawn; YTYP owns model/body/locomotion'",
        profile.player.look_sens,
        data.player.character_ref,
    );
}

fn game_data_lighting_spec(data: &GameData) -> AuthoredFpsLightingSpec {
    let lighting = &data.world.lighting;
    let shadows = &data.world.shadows;
    let day_night = data.world.day_night;
    AuthoredFpsLightingSpec {
        ambient_color: game_data_color(lighting.ambient_color, [0.42, 0.47, 0.56]),
        ambient_intensity: lighting.ambient_intensity.clamp(0.0, 8.0),
        sun_direction: game_data_direction(lighting.sun_direction, [-0.55, -0.82, -0.28]),
        sun_color: game_data_color(lighting.sun_color, [1.0, 0.955, 0.86]),
        sun_intensity: lighting.sun_intensity.clamp(0.0, 32.0),
        shadows: AuthoredFpsShadowSpec {
            enabled: shadows.enabled,
            resolution: shadows.resolution.clamp(256, 8192),
            cascade_count: shadows.cascade_count.clamp(1, 4),
            max_distance: shadows.max_distance.clamp(1.0, 1000.0),
            softness: shadows.softness.clamp(0.0, 16.0),
            bias: shadows.bias.clamp(0.0, 0.1),
            normal_bias: shadows.normal_bias.clamp(0.0, 0.5),
            contact_strength: shadows.contact_strength.clamp(0.0, 1.0),
            filter: game_data_shadow_filter(&shadows.filter),
            pcss: newengine_lighting::ShadowPcssSettings {
                light_angular_radius_degrees: shadows.pcss_light_angular_radius_degrees,
                blocker_search_radius_texels: shadows.pcss_blocker_search_radius_texels,
                max_filter_radius_texels: shadows.pcss_max_filter_radius_texels,
                blocker_samples: shadows.pcss_blocker_samples,
                filter_samples: shadows.pcss_filter_samples,
                min_filter_radius_texels: shadows.pcss_min_filter_radius_texels,
                stable_kernel_cell_texels: shadows.pcss_stable_kernel_cell_texels,
            }
            .sanitized(),
        },
        day_night: AuthoredFpsDayNightSpec {
            enabled: day_night.enabled,
            time_of_day_hours: day_night.time_of_day_hours.rem_euclid(24.0),
            day_length_seconds: day_night.day_length_seconds.clamp(30.0, 86_400.0),
            day_of_year: day_night.day_of_year.clamp(1, 366),
            latitude_degrees: day_night.latitude_degrees.clamp(-89.0, 89.0),
            axial_tilt_degrees: day_night.axial_tilt_degrees.clamp(-45.0, 45.0),
        },
    }
}
