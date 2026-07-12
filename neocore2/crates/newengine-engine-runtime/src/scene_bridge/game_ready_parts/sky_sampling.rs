use super::*;

pub(in crate::scene_bridge::game_ready) fn time_snapshot_for_sky_cycle(
) -> Option<newengine_core::time::TimeSnapshotV1> {
    match call_service_v1_optional(
        newengine_core::time::ENGINE_TIME_SERVICE_ID,
        newengine_core::time::time_method::SNAPSHOT_V1,
        &[],
    ) {
        Ok(Some(bytes)) => {
            match serde_json::from_slice::<newengine_core::time::TimeSnapshotV1>(&bytes) {
                Ok(snapshot) => Some(snapshot),
                Err(e) => {
                    newengine_ulog_api::ulog::warn!("game-ready sky cycle: engine.time snapshot invalid; keeping authored scene.day_night time for this tick err='{e}'");
                    None
                }
            }
        }
        Ok(None) => None,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("game-ready sky cycle: engine.time snapshot unavailable; keeping authored scene.day_night time for this tick err='{e}'");
            None
        }
    }
}

pub(in crate::scene_bridge::game_ready) fn authored_time_snapshot_for_sky_cycle(
    cycle: &SkyCycleRuntime,
) -> newengine_core::time::TimeSnapshotV1 {
    let mut snapshot = newengine_core::time::TimeSnapshotV1::default();
    snapshot.game.seconds_per_game_day = (cycle.day_length_seconds as f64).max(1.0);
    snapshot.game.normalized_day = (cycle.time_of_day_hours as f64 / 24.0).rem_euclid(1.0);
    snapshot.game.seconds_of_day =
        snapshot.game.normalized_day * snapshot.game.seconds_per_game_day;
    snapshot.game.time_scale = if cycle.enabled { 1.0 } else { 0.0 };
    snapshot
}

pub(in crate::scene_bridge::game_ready) fn environment_frame_for_sky_cycle(
    cycle: &SkyCycleRuntime,
    snapshot: newengine_core::time::TimeSnapshotV1,
) -> Option<newengine_world_environment_api::EnvironmentFrameDto> {
    let request = newengine_world_environment_api::EnvironmentFrameRequest {
        frame_id: snapshot.frame_index,
        world_instance_id: "game-ready-fps.world".to_owned(),
        time: snapshot,
        observer_position: newengine_world_environment_api::Vec3Dto::zero(),
        observer_cell: None,
        active_region: Some("game_ready.forest_road".to_owned()),
        active_biome: Some("temperate_forest".to_owned()),
        resident_cells: Vec::new(),
        environment_profile: newengine_world_environment_api::EnvironmentProfileRefDto {
            profile_id: "environment.game_ready_forest_road".to_owned(),
        },
        seed: 0x4752_4541_4459u64 ^ u64::from(cycle.day_length_seconds.to_bits()),
    };
    let payload = match serde_json::to_vec(&request) {
        Ok(payload) => payload,
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready environment bridge: failed to encode EnvironmentFrameRequest err='{e}'"
            );
            return None;
        }
    };
    match call_service_v1_optional(
        newengine_world_environment_api::ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
        newengine_world_environment_api::WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<
            newengine_world_environment_api::EnvironmentFrameDto,
        >(&bytes)
        {
            Ok(frame) => Some(frame),
            Err(e) => {
                newengine_ulog_api::ulog::warn!("game-ready environment bridge: EnvironmentFrameDto decode failed; using explicit degraded authored sky frame err='{e}'");
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            newengine_ulog_api::ulog::warn!("game-ready environment bridge: engine.world.environment unavailable; using explicit degraded authored sky frame err='{e}'");
            None
        }
    }
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sync_game_ready_day_night_to_engine_time(
    day_night: &GameReadyDayNightSpec,
) {
    let request = newengine_core::time::TimeGameClockSetRequestV1 {
        day_index: u64::from(day_night.day_of_year.saturating_sub(1)),
        seconds_of_day: (day_night.time_of_day_hours as f64 * 3600.0).rem_euclid(86_400.0),
        seconds_per_game_day: (day_night.day_length_seconds as f64).max(1.0),
        time_scale: if day_night.enabled { 1.0 } else { 0.0 },
    };
    let Ok(payload) = serde_json::to_vec(&request) else {
        newengine_ulog_api::ulog::warn!(
            "game-ready sky cycle: failed to encode engine.time clock request"
        );
        return;
    };
    match call_service_v1_optional(
        newengine_core::time::ENGINE_TIME_SERVICE_ID,
        newengine_core::time::time_method::SET_GAME_CLOCK_V1,
        &payload,
    ) {
        Ok(Some(bytes)) => match serde_json::from_slice::<newengine_core::time::TimeSnapshotV1>(&bytes) {
            Ok(snapshot) => newengine_ulog_api::ulog::info!(
                "game-ready sky cycle: engine.time game clock set source='scene.day_night' tod={:.2}h day_of_year={} day_len={:.1}s normalized_day={:.6} time_scale={:.3}",
                day_night.time_of_day_hours,
                day_night.day_of_year,
                day_night.day_length_seconds,
                snapshot.game.normalized_day,
                snapshot.game.time_scale
            ),
            Err(e) => newengine_ulog_api::ulog::warn!(
                "game-ready sky cycle: engine.time set_game_clock_v1 returned invalid snapshot err='{}'",
                e
            ),
        },
        Ok(None) => newengine_ulog_api::ulog::debug!(
            "game-ready sky cycle: engine.time route absent; authored scene.day_night time remains fixed until a time provider is active"
        ),
        Err(e) => newengine_ulog_api::ulog::warn!(
            "game-ready sky cycle: engine.time set_game_clock_v1 failed; authored scene.day_night time remains fixed err='{}'",
            e
        ),
    }
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-5)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn solar_direction_from_cycle(
    time_hours: f32,
    latitude_degrees: f32,
    axial_tilt_degrees: f32,
    day_index: u64,
) -> Vec3 {
    // Solar declination is seasonal, not hourly. The previous implementation
    // varied declination during a single day and produced an asymmetric, visibly
    // accelerating solar arc. This uses a stable tropical-year approximation.
    let latitude = latitude_degrees.to_radians().clamp(-1.5533, 1.5533);
    let axial_tilt = axial_tilt_degrees.to_radians().clamp(0.0, 0.5236);
    let season = TAU * ((day_index as f32 - 80.0) / 365.2422);
    let declination = axial_tilt * season.sin();
    let hour_angle = TAU * (time_hours / 24.0 - 0.5);

    let sin_altitude = (latitude.sin() * declination.sin()
        + latitude.cos() * declination.cos() * hour_angle.cos())
    .clamp(-1.0, 1.0);
    let east = declination.cos() * hour_angle.sin();
    let north =
        latitude.cos() * declination.sin() - latitude.sin() * declination.cos() * hour_angle.cos();
    Vec3::new(east, sin_altitude, -north).normalize_or_zero()
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_mul3(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_mul3_components(
    a: [f32; 3],
    b: [f32; 3],
) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_clamp3(a: [f32; 3], lo: f32, hi: f32) -> [f32; 3] {
    [a[0].clamp(lo, hi), a[1].clamp(lo, hi), a[2].clamp(lo, hi)]
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_color_to_rgba(a: [f32; 3]) -> [f32; 4] {
    [a[0], a[1], a[2], 1.0]
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn sky_safe_dir(v: Vec3, fallback: Vec3) -> Vec3 {
    if v.is_finite() && v.length_squared() > 1.0e-6 {
        v.normalize_or_zero()
    } else {
        fallback
    }
}

pub(in crate::scene_bridge::game_ready) fn sample_sky_frame(
    cycle: &SkyCycleRuntime,
    atmosphere: Option<&SkyAtmosphereRuntime>,
    to_sun: Vec3,
) -> SkyFrameSample {
    let to_sun = sky_safe_dir(to_sun, Vec3::new(0.0, 1.0, 0.0));
    let elevation = to_sun.y;

    // Photometric transition bands based on astronomical twilight thresholds.
    // The values are sin(elevation degrees), because `to_sun.y` already stores
    // sin(altitude). Separating civil/nautical/astronomical twilight prevents the
    // single broad smoothstep that used to flatten dawn and dusk into one band.
    let astronomical = sky_smoothstep(-0.3090, -0.2079, elevation);
    let nautical = sky_smoothstep(-0.2079, -0.1045, elevation);
    let civil = sky_smoothstep(-0.1045, 0.0349, elevation);
    let day = sky_smoothstep(-0.0349, 0.1392, elevation);
    let night = (1.0 - astronomical).clamp(0.0, 1.0);
    let twilight = ((astronomical - day) * 0.28 + (nautical - day) * 0.34 + (civil - day) * 0.62)
        .clamp(0.0, 1.0);
    let horizon_glow = (1.0 - sky_smoothstep(0.05, 0.62, elevation.abs())).clamp(0.0, 1.0);
    let dusk_mix = (twilight * (0.62 + horizon_glow * 0.38)).clamp(0.0, 1.0);

    let profile = atmosphere.map(|a| &a.profile);
    let day_zenith = profile.map(|p| p.day_zenith).unwrap_or([0.23, 0.42, 0.82]);
    let day_horizon = profile.map(|p| p.day_horizon).unwrap_or([0.64, 0.78, 0.96]);
    let dusk_zenith = profile.map(|p| p.dusk_zenith).unwrap_or([0.16, 0.20, 0.40]);
    let dusk_horizon = profile
        .map(|p| p.dusk_horizon)
        .unwrap_or([1.00, 0.47, 0.20]);
    let night_zenith = profile
        .map(|p| p.night_zenith)
        .unwrap_or([0.006, 0.010, 0.030]);
    let night_horizon = profile
        .map(|p| p.night_horizon)
        .unwrap_or([0.020, 0.024, 0.052]);
    let cloud_day = profile.map(|p| p.cloud_day).unwrap_or([0.98, 0.96, 0.88]);
    let cloud_night = profile
        .map(|p| p.cloud_night)
        .unwrap_or([0.040, 0.050, 0.085]);
    let night_sky_strength = profile
        .map(|p| p.night_sky_strength)
        .unwrap_or(0.35)
        .clamp(0.0, 1.0);
    let cloud_coverage = profile
        .map(|p| p.cloud_coverage)
        .unwrap_or(0.42)
        .clamp(0.0, 1.0);
    let cloud_softness = profile
        .map(|p| p.cloud_softness)
        .unwrap_or(0.72)
        .clamp(0.04, 0.98);

    let zenith_base = sky_lerp3(night_zenith, day_zenith, day);
    let horizon_base = sky_lerp3(night_horizon, day_horizon, day);
    let zenith = sky_lerp3(zenith_base, dusk_zenith, dusk_mix);
    let horizon = sky_lerp3(horizon_base, dusk_horizon, dusk_mix);

    let sky_band = (0.27 + 0.42 * twilight).clamp(0.0, 1.0);
    let mut sky_rgb = sky_lerp3(zenith, horizon, sky_band);
    let night_dim = (1.0 - night * (1.0 - night_sky_strength)).clamp(0.025, 1.0);
    sky_rgb = sky_mul3(sky_rgb, night_dim);

    let cloud_visibility = (0.12 + 0.88 * day + 0.30 * twilight + 0.15 * night).clamp(0.0, 1.0);
    let cloud_shape_gain = (1.0 - cloud_coverage * 0.22) * (0.68 + cloud_softness * 0.32);
    let cloud_rgb = sky_mul3(
        sky_lerp3(
            cloud_night,
            sky_lerp3(cloud_day, dusk_horizon, twilight * 0.42),
            day.max(twilight * 0.70),
        ),
        (cloud_visibility * cloud_shape_gain).clamp(0.02, 1.35),
    );

    let warm = [1.0, 0.49, 0.20];
    let moon_light = [0.22, 0.29, 0.48];
    let noon = cycle.base_sun_color;
    let day_color = sky_lerp3(noon, warm, horizon_glow * (1.0 - day * 0.72));
    let sun_color = sky_lerp3(moon_light, day_color, day.max(civil * 0.22));
    let solar_height = elevation.max(0.0).powf(0.42);
    let sun_intensity = cycle.base_sun_intensity * solar_height
        + cycle.base_sun_intensity * 0.075 * civil * (1.0 - day)
        + 0.018 * night;

    let ambient_color = sky_lerp3(
        sky_lerp3([0.015, 0.021, 0.052], cycle.base_ambient_color, day),
        [0.39, 0.25, 0.16],
        twilight * 0.34,
    );
    let ambient_intensity = cycle.base_ambient_intensity * (0.055 + 0.945 * day.powf(0.72))
        + 0.060 * civil * (1.0 - day)
        + 0.012 * night;

    SkyFrameSample {
        to_sun,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 2.5)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 2.5)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.25),
        sun_intensity: sun_intensity.max(0.0),
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
        cloud_coverage,
        cloud_softness,
        cloud_shadow_strength: (cloud_coverage * 0.38).clamp(0.0, 0.55),
        haze_amount: (0.08 + horizon_glow * 0.12).clamp(0.0, 0.35),
        cloud_advection: Vec2::new(2.1, 0.65),
        rayleigh_strength: 1.0,
        mie_strength: (0.56 + horizon_glow * 0.34).clamp(0.35, 1.2),
        star_intensity: (night * night_sky_strength * 0.9).clamp(0.0, 1.0),
        cloud_gust_strength: 0.18,
        cloud_overcast: cloud_coverage * 0.28,
        cloud_light_absorption: cloud_coverage * 0.18,
    }
}

pub(in crate::scene_bridge::game_ready) fn env_vec_to_vec3(
    v: newengine_world_environment_api::Vec3Dto,
    fallback: Vec3,
) -> Vec3 {
    sky_safe_dir(Vec3::new(v.x, v.y, v.z), fallback)
}

#[inline]
pub(in crate::scene_bridge::game_ready) fn env_color_to_rgb(
    c: newengine_world_environment_api::Color3Dto,
) -> [f32; 3] {
    [
        c.r.clamp(0.0, 1.0),
        c.g.clamp(0.0, 1.0),
        c.b.clamp(0.0, 1.0),
    ]
}

#[derive(Clone, Copy)]
struct SkyCloudVisualPreset {
    softness: f32,
    haze_bias: f32,
    shadow_scale: f32,
    day_tint: [f32; 3],
    night_tint: [f32; 3],
    rayleigh_scale: f32,
    mie_scale: f32,
}

#[inline]
fn sky_cloud_visual_preset(
    weather: newengine_world_environment_api::WeatherKind,
) -> SkyCloudVisualPreset {
    use newengine_world_environment_api::WeatherKind;

    match weather {
        WeatherKind::Clear => SkyCloudVisualPreset {
            softness: 0.88,
            haze_bias: -0.02,
            shadow_scale: 0.45,
            day_tint: [1.03, 1.02, 1.00],
            night_tint: [0.90, 0.96, 1.08],
            rayleigh_scale: 1.06,
            mie_scale: 0.86,
        },
        WeatherKind::Cloudy => SkyCloudVisualPreset {
            softness: 0.74,
            haze_bias: 0.01,
            shadow_scale: 0.92,
            day_tint: [1.00, 0.99, 0.98],
            night_tint: [0.90, 0.96, 1.07],
            rayleigh_scale: 0.98,
            mie_scale: 1.00,
        },
        WeatherKind::Overcast => SkyCloudVisualPreset {
            softness: 0.60,
            haze_bias: 0.05,
            shadow_scale: 1.04,
            day_tint: [0.82, 0.88, 0.98],
            night_tint: [0.76, 0.84, 1.02],
            rayleigh_scale: 0.84,
            mie_scale: 1.18,
        },
        WeatherKind::Rain => SkyCloudVisualPreset {
            softness: 0.52,
            haze_bias: 0.09,
            shadow_scale: 1.12,
            day_tint: [0.70, 0.78, 0.90],
            night_tint: [0.66, 0.75, 0.92],
            rayleigh_scale: 0.74,
            mie_scale: 1.32,
        },
        WeatherKind::Storm => SkyCloudVisualPreset {
            softness: 0.42,
            haze_bias: 0.13,
            shadow_scale: 1.25,
            day_tint: [0.52, 0.60, 0.72],
            night_tint: [0.50, 0.59, 0.78],
            rayleigh_scale: 0.64,
            mie_scale: 1.48,
        },
        WeatherKind::Snow => SkyCloudVisualPreset {
            softness: 0.66,
            haze_bias: 0.07,
            shadow_scale: 0.88,
            day_tint: [1.05, 1.09, 1.17],
            night_tint: [0.82, 0.91, 1.10],
            rayleigh_scale: 0.94,
            mie_scale: 1.12,
        },
        WeatherKind::Fog => SkyCloudVisualPreset {
            softness: 0.92,
            haze_bias: 0.16,
            shadow_scale: 0.34,
            day_tint: [0.90, 0.95, 1.01],
            night_tint: [0.76, 0.85, 0.99],
            rayleigh_scale: 0.68,
            mie_scale: 1.52,
        },
        WeatherKind::DustStorm => SkyCloudVisualPreset {
            softness: 0.72,
            haze_bias: 0.22,
            shadow_scale: 0.56,
            day_tint: [1.16, 0.88, 0.64],
            night_tint: [0.88, 0.66, 0.50],
            rayleigh_scale: 0.58,
            mie_scale: 1.78,
        },
        WeatherKind::HeatHaze => SkyCloudVisualPreset {
            softness: 0.90,
            haze_bias: 0.10,
            shadow_scale: 0.22,
            day_tint: [1.10, 0.99, 0.82],
            night_tint: [0.90, 0.82, 0.72],
            rayleigh_scale: 0.82,
            mie_scale: 1.32,
        },
    }
}

pub(in crate::scene_bridge::game_ready) fn sample_sky_frame_from_environment(
    cycle: &SkyCycleRuntime,
    environment: &newengine_world_environment_api::EnvironmentFrameDto,
) -> SkyFrameSample {
    let to_sun = env_vec_to_vec3(
        environment.celestial.sun.direction_world,
        Vec3::new(0.0, 1.0, 0.0),
    );
    let render = &environment.consumer_packets.render;
    let day_strength = (render.sun_intensity_hint / 105_000.0).clamp(0.0, 1.0);
    let moon_strength = (render.moon_intensity_hint / 0.25).clamp(0.0, 1.0);
    let overcast = environment.sky.overcast_blend.clamp(0.0, 1.0);
    let weather_intensity = environment.weather.intensity.clamp(0.0, 1.0);
    let preset = sky_cloud_visual_preset(environment.weather.state);
    let preset_blend = (0.24 + weather_intensity * 0.66 + overcast * 0.10).clamp(0.0, 1.0);
    let haze =
        (environment.atmosphere.haze_amount + preset.haze_bias * preset_blend).clamp(0.0, 1.0);
    let overcast_loss = 1.0 - overcast * 0.32;
    let sky_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.zenith_color_linear),
            env_color_to_rgb(environment.sky.horizon_color_linear),
            0.36 + environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.30,
        ),
        overcast_loss,
    );
    let phase_tint = match environment.time_of_day_state.phase {
        newengine_world_environment_api::TimeOfDayPhase::Dawn => [1.06, 0.96, 0.88],
        newengine_world_environment_api::TimeOfDayPhase::Dusk => [1.08, 0.93, 0.84],
        newengine_world_environment_api::TimeOfDayPhase::Night => [0.88, 0.94, 1.10],
        newengine_world_environment_api::TimeOfDayPhase::Day => [1.0, 1.0, 1.0],
    };
    let sky_phase_weight = (environment.sky.dusk_dawn_blend * 0.20
        + environment.sky.night_blend * 0.10)
        .clamp(0.0, 0.24);
    let sky_rgb = sky_mul3_components(
        sky_rgb,
        sky_lerp3([1.0, 1.0, 1.0], phase_tint, sky_phase_weight),
    );

    let cloud_base_rgb = sky_mul3(
        sky_lerp3(
            env_color_to_rgb(environment.sky.horizon_color_linear),
            env_color_to_rgb(environment.sky.sun_horizon_color_linear),
            environment.sky.dusk_dawn_blend.clamp(0.0, 1.0) * 0.52,
        ),
        (0.76 + day_strength * 0.42 - environment.clouds.light_absorption * 0.28).clamp(0.05, 1.25),
    );
    let preset_tint = sky_lerp3(
        preset.night_tint,
        preset.day_tint,
        environment.time_of_day_state.day_blend.clamp(0.0, 1.0),
    );
    let cloud_rgb = sky_mul3_components(
        cloud_base_rgb,
        sky_lerp3([1.0, 1.0, 1.0], preset_tint, preset_blend * 0.62),
    );
    let sun_color = sky_lerp3(
        env_color_to_rgb(environment.celestial.moon.color_linear),
        env_color_to_rgb(environment.celestial.sun.color_linear),
        day_strength.max(environment.sky.dusk_dawn_blend * 0.28),
    );
    let absorption = environment.clouds.light_absorption.clamp(0.0, 1.0);
    let dusk = environment.sky.dusk_dawn_blend.clamp(0.0, 1.0);
    // Convert the provider's physically-inspired lux hints into the compact
    // renderer light range. A daylight floor is deliberate: overcast removes
    // directional contrast, not all incident energy. This prevents forest
    // materials from collapsing to black under fair/overcast transitions.
    let daylight_curve = day_strength.powf(0.72);
    let sun_intensity =
        cycle.base_sun_intensity * (0.10 + daylight_curve * 0.90) * (1.0 - absorption * 0.48)
            + cycle.base_sun_intensity * 0.020 * moon_strength
            + cycle.base_sun_intensity * 0.070 * dusk;
    let ambient_color = sky_lerp3(
        [0.020, 0.028, 0.066],
        cycle.base_ambient_color,
        (day_strength.powf(0.55) + dusk * 0.38).clamp(0.0, 1.0),
    );
    let sky_light = environment
        .lighting_intent
        .sky_light_intensity
        .clamp(0.0, 1.0);
    let storm_darkening = environment.exposure_intent.storm_darkening.clamp(0.0, 0.75);
    let ambient_intensity = cycle.base_ambient_intensity
        * (0.11 + day_strength.powf(0.58) * 0.82 + sky_light * 0.55 + overcast * 0.12)
        * (1.0 - storm_darkening * 0.55);
    let cloud_coverage = environment.clouds.coverage.clamp(0.0, 1.0);
    let baseline_softness = (0.88 - overcast * 0.30).clamp(0.36, 0.92);
    let cloud_softness = (baseline_softness + (preset.softness - baseline_softness) * preset_blend)
        .clamp(0.34, 0.94);
    let cloud_shadow_strength = (environment.clouds.shadow_strength
        * (1.0 + (preset.shadow_scale - 1.0) * preset_blend))
        .clamp(0.0, 1.0);
    let adv = environment.wind.cloud_advection;
    SkyFrameSample {
        to_sun,
        sky_tint: sky_color_to_rgba(sky_clamp3(sky_rgb, 0.0, 2.5)),
        cloud_tint: sky_color_to_rgba(sky_clamp3(cloud_rgb, 0.0, 2.5)),
        sun_color: sky_clamp3(sun_color, 0.0, 1.25),
        sun_intensity: sun_intensity.max(0.0),
        ambient_color: sky_clamp3(ambient_color, 0.0, 1.0),
        ambient_intensity: ambient_intensity.max(0.0),
        cloud_coverage,
        cloud_softness,
        cloud_shadow_strength,
        haze_amount: haze,
        cloud_advection: Vec2::new(adv.x, adv.z),
        rayleigh_strength: ((1.08 - haze * 0.22)
            * (1.0 + (preset.rayleigh_scale - 1.0) * preset_blend))
            .clamp(0.50, 1.20),
        mie_strength: ((0.50 + haze * 1.65 + overcast * 0.25)
            * (1.0 + (preset.mie_scale - 1.0) * preset_blend))
            .clamp(0.35, 2.75),
        star_intensity: (environment.sky.night_blend
            * (1.0 - environment.sky.light_pollution.clamp(0.0, 1.0))
            * (1.0 - overcast * 0.82))
            .clamp(0.0, 1.0),
        cloud_gust_strength: environment.wind.gust_strength.clamp(0.0, 1.0),
        cloud_overcast: overcast,
        cloud_light_absorption: environment.clouds.light_absorption.clamp(0.0, 1.0),
    }
}
