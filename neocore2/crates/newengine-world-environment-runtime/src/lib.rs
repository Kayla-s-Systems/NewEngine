#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted provider routes for `engine.world.environment`.
//!
//! This crate deliberately exposes baseline environment providers through the
//! gateway registry. It does not mutate ECS/world storage, inspect renderer
//! state or decide GPU implementation details.

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_dynamic_best_effort,
    register_null_engine_gateway_provider_service_dynamic_best_effort,
    EngineGatewayProviderDeclDynamic, JsonServiceRouter, NullEngineGatewayProviderDeclDynamic,
};
use newengine_world_environment_api::{
    AtmosphereStateDto, CelestialBodyDto, CelestialStateDto, CloudLayerDto, CloudStateDto,
    Color3Dto, EnvironmentDiagnosticsDto, EnvironmentFrameDto, EnvironmentFrameRequest,
    EnvironmentGameplayModifiersDto, EnvironmentGlobalStateDto, EnvironmentInvokeRequest,
    EnvironmentLightingIntentDto, EnvironmentPreviewTimeRequest, EnvironmentRestoreRequest,
    EnvironmentRestoreResponse, EnvironmentSampleAtPositionRequest, EnvironmentSampleAtPositionResponse,
    EnvironmentServiceInfo, EnvironmentSnapshotRequest, EnvironmentSnapshotResponse, ExposureIntentDto,
    SkyStateDto, Vec3Dto, WeatherStateDto, WindStateDto, ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
    WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID, WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
    WORLD_ENVIRONMENT_NULL_SERVICE_ID, WORLD_ENVIRONMENT_REQUIRED_METHODS_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_INFO,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE, WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};

pub const WORLD_ENVIRONMENT_GATEWAY_OWNER: &str = "newengine-world-environment-runtime.environment-gateway";
pub const WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE: &str = "engine.world.default.environment";
pub const WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE: &str = "engine.world.null.environment";
pub const WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1: &str = "newengine.world.environment.snapshot.v1";

#[derive(Clone, Debug)]
struct EnvironmentProviderState {
    provider: &'static str,
    provider_route: &'static str,
    degraded: bool,
    last_frame: EnvironmentFrameDto,
}

impl EnvironmentProviderState {
    #[inline]
    fn new(provider: &'static str, provider_route: &'static str, degraded: bool) -> Self {
        let last_frame = if degraded {
            EnvironmentFrameDto::neutral_degraded(0, "world.runtime.default", "environment.null.initial")
        } else {
            build_default_environment_frame(provider, provider_route, EnvironmentFrameRequest::default())
        };
        Self { provider, provider_route, degraded, last_frame }
    }

    #[inline]
    fn info(&self) -> EnvironmentServiceInfo {
        if self.degraded {
            EnvironmentServiceInfo::null_provider(self.provider)
        } else {
            EnvironmentServiceInfo::default_provider(self.provider)
        }
    }

    fn frame_json_v1(&mut self, req: EnvironmentFrameRequest) -> EnvironmentFrameDto {
        let frame = if self.degraded {
            EnvironmentFrameDto::neutral_degraded(
                req.frame_id,
                req.world_instance_id.clone(),
                deterministic_key(&req, self.provider),
            )
        } else {
            build_default_environment_frame(self.provider, self.provider_route, req)
        };
        self.last_frame = frame.clone();
        frame
    }

    fn sample_at_position_json_v1(&self, req: EnvironmentSampleAtPositionRequest) -> EnvironmentSampleAtPositionResponse {
        EnvironmentSampleAtPositionResponse {
            position: req.position,
            cell: req.cell,
            visibility_multiplier: req.frame.gameplay_modifiers.visibility_multiplier,
            wind_velocity: Vec3Dto::new(
                req.frame.wind.global_direction.x * req.frame.wind.global_speed_mps,
                req.frame.wind.global_direction.y * req.frame.wind.global_speed_mps,
                req.frame.wind.global_direction.z * req.frame.wind.global_speed_mps,
            ),
            weather_tags: req.frame.weather.tags.clone(),
            diagnostics: req.frame.diagnostics,
        }
    }

    fn snapshot_json_v1(&self, req: EnvironmentSnapshotRequest) -> EnvironmentSnapshotResponse {
        let mut frame = self.last_frame.clone();
        if !req.include_objects {
            frame.environment_objects.clear();
            frame.clouds.volumes.clear();
            frame.clouds.storm_cells.clear();
        }
        EnvironmentSnapshotResponse { schema: WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1.to_owned(), frame }
    }

    fn restore_json_v1(&mut self, req: EnvironmentRestoreRequest) -> EnvironmentRestoreResponse {
        self.last_frame = req.snapshot.frame;
        EnvironmentRestoreResponse { ok: true, frame: self.last_frame.clone() }
    }

    fn preview_time_json_v1(&mut self, req: EnvironmentPreviewTimeRequest) -> EnvironmentFrameDto {
        let mut frame_req = req.base_request;
        frame_req.time.game.normalized_day = clamp01(req.normalized_time_of_day as f64);
        frame_req.time.game.seconds_of_day = frame_req.time.game.normalized_day * frame_req.time.game.seconds_per_game_day.max(1.0);
        self.frame_json_v1(frame_req)
    }

    fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload)
            .and_then(|v| serde_json::from_value::<EnvironmentInvokeRequest>(v).map_err(|e| e.to_string()))
        {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let payload = match serde_json::to_vec(&req.payload) {
            Ok(bytes) => Blob::from(bytes),
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        match req.method.as_str() {
            WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1 => {
                let req = match payload_json(&payload)
                    .and_then(|v| serde_json::from_value::<EnvironmentFrameRequest>(v).map_err(|e| e.to_string()))
                {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                ok_json(self.frame_json_v1(req))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1 => {
                let req = match payload_json(&payload)
                    .and_then(|v| serde_json::from_value::<EnvironmentSampleAtPositionRequest>(v).map_err(|e| e.to_string()))
                {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                ok_json(self.sample_at_position_json_v1(req))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1 => {
                let req = match payload_json(&payload)
                    .and_then(|v| serde_json::from_value::<EnvironmentSnapshotRequest>(v).map_err(|e| e.to_string()))
                {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                ok_json(self.snapshot_json_v1(req))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1 => {
                let req = match payload_json(&payload)
                    .and_then(|v| serde_json::from_value::<EnvironmentRestoreRequest>(v).map_err(|e| e.to_string()))
                {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                ok_json(self.restore_json_v1(req))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1 => {
                let req = match payload_json(&payload)
                    .and_then(|v| serde_json::from_value::<EnvironmentPreviewTimeRequest>(v).map_err(|e| e.to_string()))
                {
                    Ok(v) => v,
                    Err(e) => return RResult::RErr(RString::from(e)),
                };
                ok_json(self.preview_time_json_v1(req))
            }
            other => RResult::RErr(RString::from(format!(
                "engine.world.environment invoke_json unknown target method '{other}'"
            ))),
        }
    }
}

fn environment_gateway_service(
    service_id: &'static str,
    provider: &'static str,
    provider_route: &'static str,
    degraded: bool,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = if degraded {
        EnvironmentServiceInfo::null_provider(provider)
    } else {
        EnvironmentServiceInfo::default_provider(provider)
    };
    let description = engine_gateway_provider_service_description(
        service_id,
        WORLD_ENVIRONMENT_GATEWAY_OWNER,
        WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_WORLD_ENVIRONMENT_SERVICE_ID)
    .notes("Environment is world meaning; renderer consumes resolved packets only.");

    JsonServiceRouter::with_state(service_id, EnvironmentProviderState::new(provider, provider_route, degraded))
        .describe_json(&description)
        .get_json(WORLD_ENVIRONMENT_SERVICE_METHOD_INFO, |state| state.info())
        .blob(WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE, |state, payload| state.invoke_json(payload))
        .post_json(WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, |state, req: EnvironmentFrameRequest| state.frame_json_v1(req))
        .post_json(
            WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
            |state, req: EnvironmentSampleAtPositionRequest| state.sample_at_position_json_v1(req),
        )
        .post_json(WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1, |state, req: EnvironmentSnapshotRequest| state.snapshot_json_v1(req))
        .post_json(WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1, |state, req: EnvironmentRestoreRequest| state.restore_json_v1(req))
        .post_json(WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1, |state, req: EnvironmentPreviewTimeRequest| state.preview_time_json_v1(req))
        .blob(WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

/// Registers visible default and null provider routes for `engine.world.environment`.
///
/// The default route is an engine-runtime baseline provider, not a hidden fallback.
/// The null route is registered as a real NullProvider route and remains visible
/// in gateway diagnostics even when shadowed.
pub fn register_world_environment_gateway_best_effort() {
    if !newengine_plugin_host::has_service(WORLD_ENVIRONMENT_NULL_SERVICE_ID) {
        let null_service = environment_gateway_service(
            WORLD_ENVIRONMENT_NULL_SERVICE_ID,
            "environment.null",
            WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
            true,
        );
        register_null_engine_gateway_provider_service_dynamic_best_effort(NullEngineGatewayProviderDeclDynamic {
            gateway: ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
            service_kind: "world.environment",
            provider_service: WORLD_ENVIRONMENT_NULL_SERVICE_ID,
            provider_route: WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
            capability: WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
            owner: WORLD_ENVIRONMENT_GATEWAY_OWNER,
            service: null_service,
        });
    }

    if !newengine_plugin_host::has_service(WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID) {
        let default_service = environment_gateway_service(
            WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
            "environment.default",
            WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
            false,
        );
        register_engine_gateway_provider_service_dynamic_best_effort(EngineGatewayProviderDeclDynamic {
            gateway: ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
            service_kind: "world.environment",
            provider_service: WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
            provider_route: WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
            capability: WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: WORLD_ENVIRONMENT_GATEWAY_OWNER,
            service: default_service,
        });
    }

    log::info!(
        "engine.world.environment gateway baseline routes ready methods={} default_service='{}' null_service='{}'",
        WORLD_ENVIRONMENT_REQUIRED_METHODS_V1.len(),
        WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
        WORLD_ENVIRONMENT_NULL_SERVICE_ID
    );
}

fn build_default_environment_frame(
    provider: &str,
    provider_route: &str,
    req: EnvironmentFrameRequest,
) -> EnvironmentFrameDto {
    let tod = normalized_day_from_time(&req);
    let day_index = req.time.game.day_index.min(u32::MAX as u64) as u32;
    let world_time_seconds = req.time.game.day_index as f64 * req.time.game.seconds_per_game_day.max(1.0)
        + req.time.game.seconds_of_day.max(0.0);

    let sun = sun_body(tod);
    let moon = moon_body(tod);
    let night_blend = clamp01_f32(1.0 - sun.intensity_lux_hint / 105_000.0);
    let dusk_dawn_blend = bell01(((tod - 0.25).abs()).min((tod - 0.75).abs()) * 4.0);
    let cloud_coverage = baseline_cloud_coverage(req.seed, tod);
    let overcast = clamp01_f32((cloud_coverage - 0.65) * 2.2);
    let haze = 0.04 + 0.08 * dusk_dawn_blend + 0.05 * cloud_coverage;
    let visibility = (20_000.0 * (1.0 - overcast * 0.35) * (1.0 - haze * 0.45)).max(500.0);

    let sky = SkyStateDto {
        zenith_color_linear: mix_color(Color3Dto::new(0.02, 0.025, 0.045), Color3Dto::new(0.18, 0.34, 0.62), 1.0 - night_blend),
        horizon_color_linear: mix_color(Color3Dto::new(0.05, 0.055, 0.085), Color3Dto::new(0.48, 0.62, 0.84), 1.0 - night_blend),
        sun_horizon_color_linear: mix_color(Color3Dto::new(0.16, 0.09, 0.06), Color3Dto::new(1.0, 0.48, 0.18), dusk_dawn_blend),
        opposite_horizon_color_linear: mix_color(Color3Dto::new(0.03, 0.04, 0.08), Color3Dto::new(0.32, 0.45, 0.68), 1.0 - night_blend),
        dusk_dawn_blend,
        night_blend,
        overcast_blend: overcast,
        light_pollution: 0.04 * night_blend,
    };

    let atmosphere = AtmosphereStateDto {
        fog_density: 0.01 + overcast * 0.04,
        fog_height_falloff: 0.12,
        fog_color_linear: mix_color(Color3Dto::new(0.09, 0.10, 0.14), Color3Dto::new(0.56, 0.62, 0.70), 1.0 - night_blend),
        haze_amount: haze,
        humidity: 0.30 + cloud_coverage * 0.20,
        aerosol_density: 0.08 + haze,
        visibility_distance_meters: visibility,
    };

    let weather = WeatherStateDto {
        weather_id: "weather.clear_baseline".to_owned(),
        intensity: cloud_coverage * 0.15,
        transition_progress: 1.0,
        tags: vec![
            "weather.clear".to_owned(),
            if night_blend > 0.65 { "time.night" } else { "time.day" }.to_owned(),
            "visibility.normal".to_owned(),
        ],
        ..WeatherStateDto::default()
    };

    let wind = WindStateDto {
        global_direction: normalize(Vec3Dto::new(0.92, 0.0, 0.38)),
        global_speed_mps: 2.0 + cloud_coverage * 1.5,
        gust_strength: 0.1 + overcast * 0.15,
        cloud_advection: Vec3Dto::new(2.0 + cloud_coverage, 0.0, 0.8),
    };

    let clouds = CloudStateDto {
        coverage: cloud_coverage,
        overcast,
        shadow_strength: clamp01_f32(cloud_coverage * 0.45),
        light_absorption: clamp01_f32(cloud_coverage * 0.25),
        layers: vec![CloudLayerDto {
            coverage: cloud_coverage,
            density: 0.18 + cloud_coverage * 0.35,
            wind_velocity: wind.cloud_advection,
            ..CloudLayerDto::default()
        }],
        volumes: Vec::new(),
        storm_cells: Vec::new(),
    };

    let lighting_intent = EnvironmentLightingIntentDto {
        sun_lux_hint: sun.intensity_lux_hint * (1.0 - clouds.light_absorption),
        moon_lux_hint: moon.intensity_lux_hint * (1.0 - clouds.light_absorption),
        ambient_intensity: 0.05 + (1.0 - night_blend) * 0.22 + cloud_coverage * 0.06,
        sky_light_intensity: 0.08 + (1.0 - night_blend) * 0.48,
        cloud_shadow_strength: clouds.shadow_strength,
        wetness_specular_boost: 0.0,
    };

    let gameplay_modifiers = EnvironmentGameplayModifiersDto {
        visibility_multiplier: clamp01_f32(visibility / 20_000.0),
        audio_masking_multiplier: 0.02 * cloud_coverage,
        weather_hazard_level: 0.0,
        shelter_score: 0.05 * cloud_coverage,
        surface_slipperiness_hint: 0.0,
    };

    let profile = if req.environment_profile.profile_id.trim().is_empty() {
        "environment.default".to_owned()
    } else {
        req.environment_profile.profile_id.clone()
    };
    let key = deterministic_key(&req, provider);

    EnvironmentFrameDto {
        frame_id: req.frame_id,
        world_instance_id: req.world_instance_id,
        world_time_seconds,
        time_of_day_normalized: tod,
        day_index,
        global: EnvironmentGlobalStateDto {
            active_region: req.active_region,
            active_biome: req.active_biome,
            active_weather_profile: weather.weather_id.clone(),
            active_environment_profile: profile.clone(),
            environment_seed: req.seed,
        },
        celestial: CelestialStateDto {
            sun,
            moon,
            moon_phase: 0.5,
            stars_visibility: night_blend * (1.0 - cloud_coverage * 0.75),
            night_sky_visibility: night_blend * (1.0 - cloud_coverage * 0.65),
        },
        sky,
        atmosphere,
        weather,
        clouds,
        wind,
        lighting_intent,
        gameplay_modifiers,
        exposure_intent: ExposureIntentDto {
            night_adaptation_hint: night_blend,
            storm_darkening: overcast * 0.15,
            sun_glare_hint: clamp01_f32(sun.intensity_lux_hint / 105_000.0) * (1.0 - cloud_coverage),
            interior_exterior_bias: 0.0,
        },
        environment_objects: Vec::new(),
        diagnostics: EnvironmentDiagnosticsDto {
            provider: provider.to_owned(),
            provider_route: provider_route.to_owned(),
            degraded: false,
            deterministic_key: key,
            active_profile: profile,
            reasons: vec![
                "engine.time provides clock authority".to_owned(),
                "engine.world.environment resolves environmental meaning".to_owned(),
                "engine.render remains a consumer of resolved packets".to_owned(),
            ],
            warnings: Vec::new(),
        },
    }
}

fn normalized_day_from_time(req: &EnvironmentFrameRequest) -> f32 {
    let normalized = req.time.game.normalized_day;
    if normalized.is_finite() && normalized >= 0.0 && normalized <= 1.0 {
        return normalized as f32;
    }
    let seconds_per_day = req.time.game.seconds_per_game_day.max(1.0);
    let seconds = req.time.game.seconds_of_day.rem_euclid(seconds_per_day);
    (seconds / seconds_per_day) as f32
}

fn deterministic_key(req: &EnvironmentFrameRequest, provider: &str) -> String {
    format!(
        "{}:{}:{}:{:.6}:{}",
        provider,
        req.world_instance_id,
        req.seed,
        normalized_day_from_time(req),
        req.environment_profile.profile_id
    )
}

fn sun_body(tod: f32) -> CelestialBodyDto {
    let tau = std::f32::consts::TAU;
    let orbit = tau * (tod - 0.25);
    let altitude = orbit.sin().asin();
    let visibility = smoothstep(0.0, 0.08, orbit.sin().max(0.0));
    let direction = normalize(Vec3Dto::new(orbit.cos(), orbit.sin(), (tau * tod).sin() * 0.35));
    CelestialBodyDto {
        direction_world: direction,
        altitude_radians: altitude,
        azimuth_radians: tau * tod,
        angular_radius_radians: 0.00465,
        color_linear: mix_color(Color3Dto::new(1.0, 0.47, 0.22), Color3Dto::new(1.0, 0.95, 0.82), visibility),
        intensity_lux_hint: 105_000.0 * visibility,
        visible: visibility > 0.01,
    }
}

fn moon_body(tod: f32) -> CelestialBodyDto {
    let tau = std::f32::consts::TAU;
    let orbit = tau * (tod + 0.25);
    let altitude_raw = orbit.sin();
    let visibility = smoothstep(0.0, 0.08, altitude_raw.max(0.0));
    CelestialBodyDto {
        direction_world: normalize(Vec3Dto::new(orbit.cos(), altitude_raw, (tau * (tod + 0.5)).sin() * 0.25)),
        altitude_radians: altitude_raw.asin(),
        azimuth_radians: tau * (tod + 0.5),
        angular_radius_radians: 0.00450,
        color_linear: Color3Dto::new(0.58, 0.66, 0.86),
        intensity_lux_hint: 0.25 * visibility,
        visible: visibility > 0.01,
    }
}

fn baseline_cloud_coverage(seed: u64, tod: f32) -> f32 {
    let seed_phase = ((seed ^ (seed >> 32)) as u32) as f32 / u32::MAX as f32;
    let wave = ((std::f32::consts::TAU * (tod + seed_phase)).sin() + 1.0) * 0.5;
    clamp01_f32(0.12 + wave * 0.18)
}

fn normalize(v: Vec3Dto) -> Vec3Dto {
    let len_sq = v.x * v.x + v.y * v.y + v.z * v.z;
    if len_sq <= f32::EPSILON {
        return Vec3Dto::zero();
    }
    let inv = len_sq.sqrt().recip();
    Vec3Dto::new(v.x * inv, v.y * inv, v.z * inv)
}

fn mix_color(a: Color3Dto, b: Color3Dto, t: f32) -> Color3Dto {
    let t = clamp01_f32(t);
    Color3Dto::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let denom = (edge1 - edge0).max(f32::EPSILON);
    let t = clamp01_f32((x - edge0) / denom);
    t * t * (3.0 - 2.0 * t)
}

fn bell01(x: f32) -> f32 {
    1.0 - smoothstep(0.0, 1.0, clamp01_f32(x))
}

fn clamp01(value: f64) -> f64 { value.clamp(0.0, 1.0) }
fn clamp01_f32(value: f32) -> f32 { value.clamp(0.0, 1.0) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_is_deterministic_for_same_request() {
        let mut req = EnvironmentFrameRequest::default();
        req.frame_id = 17;
        req.seed = 42;
        req.time.game.normalized_day = 0.5;
        let a = build_default_environment_frame("environment.default", WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, req.clone());
        let b = build_default_environment_frame("environment.default", WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, req);
        assert_eq!(a.diagnostics.deterministic_key, b.diagnostics.deterministic_key);
        assert_eq!(a.celestial.sun.direction_world, b.celestial.sun.direction_world);
        assert!(!a.diagnostics.degraded);
    }

    #[test]
    fn null_provider_returns_visible_degraded_frame() {
        let state = EnvironmentProviderState::new("environment.null", WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE, true);
        assert!(state.last_frame.diagnostics.degraded);
        assert_eq!(state.last_frame.diagnostics.provider_route, WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE);
    }
}
