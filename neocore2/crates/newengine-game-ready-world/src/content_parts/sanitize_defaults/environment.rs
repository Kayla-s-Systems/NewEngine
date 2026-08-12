use super::*;

#[inline]
pub(in super::super) fn sanitize_color3(mut v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    for i in 0..3 {
        if !v[i].is_finite() {
            v[i] = fallback[i];
        }
        v[i] = v[i].clamp(0.0, 1.0);
    }
    v
}

#[inline]
pub(in super::super) fn sanitize_direction3(v: ColorRgb, fallback: ColorRgb) -> ColorRgb {
    let d = Vec3::new(v[0], v[1], v[2]);
    let d = if d.length_squared() > 1.0e-6 && d.is_finite() {
        d.normalize_or_zero()
    } else {
        Vec3::new(fallback[0], fallback[1], fallback[2]).normalize_or_zero()
    };
    [d.x, d.y, d.z]
}

#[inline]
pub(in super::super) fn sanitize_sky_atmosphere_spec(
    raw: RawSkyAtmosphereSpec,
) -> GameReadySkyAtmosphereSpec {
    GameReadySkyAtmosphereSpec {
        day_zenith: sanitize_color3(raw.day_zenith, default_sky_day_zenith()),
        day_horizon: sanitize_color3(raw.day_horizon, default_sky_day_horizon()),
        dusk_zenith: sanitize_color3(raw.dusk_zenith, default_sky_dusk_zenith()),
        dusk_horizon: sanitize_color3(raw.dusk_horizon, default_sky_dusk_horizon()),
        night_zenith: sanitize_color3(raw.night_zenith, default_sky_night_zenith()),
        night_horizon: sanitize_color3(raw.night_horizon, default_sky_night_horizon()),
        cloud_day: sanitize_color3(raw.cloud_day, default_sky_cloud_day()),
        cloud_night: sanitize_color3(raw.cloud_night, default_sky_cloud_night()),
        night_sky_strength: raw.night_sky_strength.clamp(0.0, 1.0),
        cloud_coverage: raw.cloud_coverage.clamp(0.0, 1.0),
        cloud_softness: raw.cloud_softness.clamp(0.01, 1.0),
    }
}

#[inline]
fn sanitize_shadow_filter(value: &str) -> newengine_lighting::ShadowFilter {
    match value.trim().to_ascii_lowercase().as_str() {
        "hard" | "none" => newengine_lighting::ShadowFilter::Hard,
        "pcf" => newengine_lighting::ShadowFilter::Pcf,
        "pcss" => newengine_lighting::ShadowFilter::Pcss,
        _ => newengine_lighting::ShadowFilter::Pcss,
    }
}

#[inline]
pub(in super::super) fn sanitize_lighting_spec(raw: RawLightingSpec) -> GameReadyLightingSpec {
    GameReadyLightingSpec {
        ambient_color: sanitize_color3(raw.ambient_color, default_ambient_color()),
        ambient_intensity: raw.ambient_intensity.clamp(0.0, 8.0),
        sun_direction: sanitize_direction3(raw.sun_direction, default_sun_direction()),
        sun_color: sanitize_color3(raw.sun_color, default_sun_color()),
        sun_intensity: raw.sun_intensity.clamp(0.0, 32.0),
        day_night: GameReadyDayNightSpec {
            enabled: raw.day_night.enabled,
            time_of_day_hours: raw.day_night.time_of_day_hours.rem_euclid(24.0),
            day_length_seconds: raw.day_night.day_length_seconds.clamp(30.0, 86_400.0),
            day_of_year: raw.day_night.day_of_year.clamp(1, 366),
            latitude_degrees: raw.day_night.latitude_degrees.clamp(-89.0, 89.0),
            axial_tilt_degrees: raw.day_night.axial_tilt_degrees.clamp(-45.0, 45.0),
        },
        shadows: GameReadyShadowSpec {
            enabled: raw.shadows.enabled,
            resolution: raw.shadows.resolution.clamp(256, 8192),
            cascade_count: raw.shadows.cascade_count.clamp(1, 4),
            max_distance: raw.shadows.max_distance.clamp(1.0, 1000.0),
            softness: raw.shadows.softness.clamp(0.0, 16.0),
            bias: raw.shadows.bias.clamp(0.0, 0.1),
            normal_bias: raw.shadows.normal_bias.clamp(0.0, 0.5),
            contact_strength: raw.shadows.contact_strength.clamp(0.0, 1.0),
            filter: sanitize_shadow_filter(&raw.shadows.filter),
            pcss: newengine_lighting::ShadowPcssSettings {
                light_angular_radius_degrees: raw.shadows.pcss_light_angular_radius_degrees,
                blocker_search_radius_texels: raw.shadows.pcss_blocker_search_radius_texels,
                max_filter_radius_texels: raw.shadows.pcss_max_filter_radius_texels,
                blocker_samples: raw.shadows.pcss_blocker_samples,
                filter_samples: raw.shadows.pcss_filter_samples,
                min_filter_radius_texels: raw.shadows.pcss_min_filter_radius_texels,
                stable_kernel_cell_texels: raw.shadows.pcss_stable_kernel_cell_texels,
            }
            .sanitized(),
        },
    }
}

#[inline]
pub(in super::super) fn sanitize_foliage_spec(raw: RawFoliageSpec) -> GameReadyFoliageSpec {
    let min_scale = raw.min_scale.clamp(0.05, 32.0);
    let max_scale = raw.max_scale.clamp(min_scale, 32.0);
    let (grid_min, grid_max) = if raw.grid_min <= raw.grid_max {
        (raw.grid_min, raw.grid_max)
    } else {
        (raw.grid_max, raw.grid_min)
    };

    GameReadyFoliageSpec {
        enabled: raw.enabled && raw.max_count > 0,
        prefab: non_empty_or(raw.prefab, default_foliage_prefab()),
        seed: raw.seed,
        grid_min: grid_min.clamp(-512, 512),
        grid_max: grid_max.clamp(-512, 512),
        spacing: raw.spacing.clamp(0.5, 128.0),
        jitter: raw.jitter.clamp(0.0, 0.95),
        gate_threshold: raw.gate_threshold.clamp(0.0, 1.0),
        max_count: raw.max_count.min(8192),
        min_scale,
        max_scale,
        min_player_distance: raw.min_player_distance.clamp(0.0, 256.0),
        edge_margin: raw.edge_margin.clamp(0.0, 512.0),
        surface_offset: raw.surface_offset.clamp(-4.0, 8.0),
        render_options: newengine_model_domain_api::MeshRenderOptions::foliage_instanced(),
    }
}
