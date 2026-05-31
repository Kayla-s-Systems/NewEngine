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
    EnvironmentFrameDto, EnvironmentFrameRequest, EnvironmentInvokeRequest,
    EnvironmentPreviewTimeRequest, EnvironmentRestoreRequest, EnvironmentRestoreResponse,
    EnvironmentSampleAtPositionRequest, EnvironmentSampleAtPositionResponse,
    EnvironmentServiceInfo, EnvironmentSnapshotRequest, EnvironmentSnapshotResponse, Vec3Dto,
    ENGINE_WORLD_ENVIRONMENT_SERVICE_ID, WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
    WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID, WORLD_ENVIRONMENT_NULL_SERVICE_ID,
    WORLD_ENVIRONMENT_REQUIRED_METHODS_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INFO, WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE,
    WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};
mod celestial;
mod consumer_packets;
mod default_provider;
mod math;
mod phenomena;
mod profile_catalog;
mod visual_asset_catalog;
mod weather_profile;

use default_provider::{build_default_environment_frame, deterministic_key};
use math::clamp01;

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
        assert_eq!(a.weather.state, b.weather.state);
        assert_eq!(a.environment_objects, b.environment_objects);
        assert_eq!(a.visual_assets.visual_group_id, "environment.visuals.game_ready_skydome.v1");
        assert_eq!(a.visual_assets.texture_dictionary_ref, "textures/fps/skydome.ytd");
        assert_eq!(a.visual_assets.sky_texture_ref, "textures/fps/skydome.ytd@starfield");
        assert_eq!(a.visual_assets.cloud_density_texture_ref, "textures/fps/skydome.ytd@baseperlinnoise3channel");
        assert_eq!(a.visual_assets.moon_disk_texture_ref, "textures/fps/skydome.ytd@moon_new");
        assert!(!a.visual_assets.sun_disk_texture_ref.contains("textures/sky/celestial.ytd"));
        assert_eq!(a.visual_assets.sky_texture_ref, a.consumer_packets.render.sky_texture_ref);
        assert_eq!(a.visual_assets.visual_group_id, a.consumer_packets.render.visual_group_id);
        assert!(!a.consumer_packets.streaming.residency_intents.is_empty() || a.clouds.coverage <= 0.20);
        assert!(!a.diagnostics.degraded);
    }

    #[test]
    fn profile_selection_is_exact_descriptor_not_substring_weather_force() {
        let mut req = EnvironmentFrameRequest::default();
        req.environment_profile.profile_id = "environment.fake_storm_name_that_is_not_registered".to_owned();
        req.seed = 7;
        req.time.game.normalized_day = 0.50;
        let frame = build_default_environment_frame("environment.default", WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, req);
        assert_eq!(frame.global.active_environment_profile, "environment.default");
        assert!(frame.diagnostics.warnings.iter().any(|warning| warning.contains("unknown environment profile")));
        assert!(frame.diagnostics.reasons.iter().any(|reason| reason.contains("weather_table=")));
    }


    #[test]
    fn visual_asset_refs_use_existing_grouped_skydome_dictionary() {
        let frame = build_default_environment_frame(
            "environment.default",
            WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
            EnvironmentFrameRequest::default(),
        );
        let serialized = serde_json::to_string(&frame.visual_assets).expect("visual assets serialize");
        assert!(serialized.contains("textures/fps/skydome.ytd"));
        assert!(!serialized.contains("textures/sky/highlands_sky.ytd"));
        assert!(!serialized.contains("textures/sky/default_sky.ytd"));
        assert!(!serialized.contains("textures/sky/alpine_sky.ytd"));
        assert!(!serialized.contains("textures/sky/desert_sky.ytd"));
        assert!(!serialized.contains("textures/sky/celestial.ytd"));
    }

    #[test]
    fn null_provider_returns_visible_degraded_frame() {
        let state = EnvironmentProviderState::new("environment.null", WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE, true);
        assert!(state.last_frame.diagnostics.degraded);
        assert_eq!(state.last_frame.diagnostics.provider_route, WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE);
    }
}
