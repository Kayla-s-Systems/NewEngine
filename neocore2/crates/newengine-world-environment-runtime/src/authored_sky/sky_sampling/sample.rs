use super::*;

pub(crate) fn sample_sky_frame(
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
    let defaults = newengine_game_data::default_game_data()
        .world
        .sky
        .atmosphere;
    let day_zenith = profile.map(|p| p.day_zenith).unwrap_or(defaults.day_zenith);
    let day_horizon = profile
        .map(|p| p.day_horizon)
        .unwrap_or(defaults.day_horizon);
    let dusk_zenith = profile
        .map(|p| p.dusk_zenith)
        .unwrap_or(defaults.dusk_zenith);
    let dusk_horizon = profile
        .map(|p| p.dusk_horizon)
        .unwrap_or(defaults.dusk_horizon);
    let night_zenith = profile
        .map(|p| p.night_zenith)
        .unwrap_or(defaults.night_zenith);
    let night_horizon = profile
        .map(|p| p.night_horizon)
        .unwrap_or(defaults.night_horizon);
    let cloud_day = profile.map(|p| p.cloud_day).unwrap_or(defaults.cloud_day);
    let cloud_night = profile
        .map(|p| p.cloud_night)
        .unwrap_or(defaults.cloud_night);
    let night_sky_strength = profile
        .map(|p| p.night_sky_strength)
        .unwrap_or(defaults.night_sky_strength)
        .clamp(0.0, 1.0);
    let cloud_coverage = profile
        .map(|p| p.cloud_coverage)
        .unwrap_or(defaults.cloud_coverage)
        .clamp(0.0, 1.0);
    let cloud_softness = profile
        .map(|p| p.cloud_softness)
        .unwrap_or(defaults.cloud_softness)
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
        cloud_field_seed: cycle.day_index.wrapping_mul(0x9E37_79B9_7F4A_7C15),
        cloud_world_time_seconds: cycle.day_index as f64 * 86_400.0
            + cycle.time_of_day_hours as f64 * 3_600.0,
        rayleigh_strength: 1.0,
        mie_strength: (0.56 + horizon_glow * 0.34).clamp(0.35, 1.2),
        star_intensity: (night * night_sky_strength * 0.9).clamp(0.0, 1.0),
        cloud_gust_strength: 0.18,
        cloud_overcast: cloud_coverage * 0.28,
        cloud_light_absorption: cloud_coverage * 0.18,
        cloud_base_altitude_m: 1350.0,
        cloud_thickness_m: 980.0 + cloud_coverage * 520.0,
        cloud_layer_density: cloud_coverage * 0.34,
        high_cloud_coverage: cloud_coverage * 0.22,
        high_cloud_density: cloud_coverage * 0.08,
        humidity: (0.38 + cloud_coverage * 0.28).clamp(0.0, 1.0),
        aerosol_density: (0.10 + horizon_glow * 0.12).clamp(0.0, 1.0),
        precipitation_intensity: 0.0,
    }
}

pub(crate) fn env_vec_to_vec3(v: newengine_world_environment_api::Vec3Dto, fallback: Vec3) -> Vec3 {
    sky_safe_dir(Vec3::new(v.x, v.y, v.z), fallback)
}

#[inline]
pub(crate) fn env_color_to_rgb(c: newengine_world_environment_api::Color3Dto) -> [f32; 3] {
    [
        c.r.clamp(0.0, 1.0),
        c.g.clamp(0.0, 1.0),
        c.b.clamp(0.0, 1.0),
    ]
}
