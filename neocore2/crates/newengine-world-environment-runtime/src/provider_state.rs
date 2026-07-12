use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_service_kit::ok_json;
use newengine_world_environment_api::{
    EnvironmentFrameDto, EnvironmentFrameRequest, EnvironmentInvokeRequest,
    EnvironmentPreviewTimeRequest, EnvironmentRestoreRequest, EnvironmentRestoreResponse,
    EnvironmentSampleAtPositionRequest, EnvironmentSampleAtPositionResponse,
    EnvironmentServiceInfo, EnvironmentSnapshotRequest, EnvironmentSnapshotResponse, Vec3Dto,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};

use crate::{
    constants::WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1,
    default_provider::{build_default_environment_frame, deterministic_key},
    math::clamp01,
    payload::{decode_blob, decode_value},
};

#[derive(Clone, Debug)]
pub(crate) struct EnvironmentProviderState {
    provider: &'static str,
    provider_route: &'static str,
    degraded: bool,
    pub(crate) last_frame: EnvironmentFrameDto,
}

impl EnvironmentProviderState {
    #[inline]
    pub(crate) fn new(
        provider: &'static str,
        provider_route: &'static str,
        degraded: bool,
    ) -> Self {
        let last_frame = if degraded {
            EnvironmentFrameDto::neutral_degraded(
                0,
                "world.runtime.default",
                "environment.null.initial",
            )
        } else {
            build_default_environment_frame(
                provider,
                provider_route,
                EnvironmentFrameRequest::default(),
            )
        };
        Self {
            provider,
            provider_route,
            degraded,
            last_frame,
        }
    }

    #[inline]
    pub(crate) fn info(&self) -> EnvironmentServiceInfo {
        if self.degraded {
            EnvironmentServiceInfo::null_provider(self.provider)
        } else {
            EnvironmentServiceInfo::default_provider(self.provider)
        }
    }

    pub(crate) fn frame_json_v1(
        &mut self,
        request: EnvironmentFrameRequest,
    ) -> EnvironmentFrameDto {
        let frame = if self.degraded {
            let key = deterministic_key(&request, self.provider);
            EnvironmentFrameDto::neutral_degraded(request.frame_id, request.world_instance_id, key)
        } else {
            build_default_environment_frame(self.provider, self.provider_route, request)
        };
        self.last_frame = frame.clone();
        frame
    }

    pub(crate) fn sample_at_position_json_v1(
        &self,
        request: EnvironmentSampleAtPositionRequest,
    ) -> EnvironmentSampleAtPositionResponse {
        let speed = request.frame.wind.global_speed_mps;
        let direction = request.frame.wind.global_direction;
        EnvironmentSampleAtPositionResponse {
            position: request.position,
            cell: request.cell,
            visibility_multiplier: request.frame.gameplay_modifiers.visibility_multiplier,
            wind_velocity: Vec3Dto::new(
                direction.x * speed,
                direction.y * speed,
                direction.z * speed,
            ),
            weather_tags: request.frame.weather.tags.clone(),
            diagnostics: request.frame.diagnostics,
        }
    }

    pub(crate) fn snapshot_json_v1(
        &self,
        request: EnvironmentSnapshotRequest,
    ) -> EnvironmentSnapshotResponse {
        let mut frame = self.last_frame.clone();
        if !request.include_objects {
            frame.environment_objects.clear();
            frame.clouds.volumes.clear();
            frame.clouds.storm_cells.clear();
        }
        EnvironmentSnapshotResponse {
            schema: WORLD_ENVIRONMENT_SNAPSHOT_SCHEMA_V1.to_owned(),
            frame,
        }
    }

    pub(crate) fn restore_json_v1(
        &mut self,
        request: EnvironmentRestoreRequest,
    ) -> EnvironmentRestoreResponse {
        self.last_frame = request.snapshot.frame;
        EnvironmentRestoreResponse {
            ok: true,
            frame: self.last_frame.clone(),
        }
    }

    pub(crate) fn preview_time_json_v1(
        &mut self,
        request: EnvironmentPreviewTimeRequest,
    ) -> EnvironmentFrameDto {
        let mut frame_request = request.base_request;
        frame_request.time.game.normalized_day = clamp01(request.normalized_time_of_day as f64);
        frame_request.time.game.seconds_of_day = frame_request.time.game.normalized_day
            * frame_request.time.game.seconds_per_game_day.max(1.0);
        self.frame_json_v1(frame_request)
    }

    pub(crate) fn invoke_json(&mut self, payload: Blob) -> RResult<Blob, RString> {
        let request = match decode_blob::<EnvironmentInvokeRequest>(&payload) {
            Ok(request) => request,
            Err(error) => return RResult::RErr(error),
        };

        match request.method.as_str() {
            WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1 => {
                let request = match decode_value::<EnvironmentFrameRequest>(request.payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.frame_json_v1(request))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1 => {
                let request =
                    match decode_value::<EnvironmentSampleAtPositionRequest>(request.payload) {
                        Ok(request) => request,
                        Err(error) => return RResult::RErr(error),
                    };
                ok_json(self.sample_at_position_json_v1(request))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1 => {
                let request = match decode_value::<EnvironmentSnapshotRequest>(request.payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.snapshot_json_v1(request))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1 => {
                let request = match decode_value::<EnvironmentRestoreRequest>(request.payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.restore_json_v1(request))
            }
            WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1 => {
                let request = match decode_value::<EnvironmentPreviewTimeRequest>(request.payload) {
                    Ok(request) => request,
                    Err(error) => return RResult::RErr(error),
                };
                ok_json(self.preview_time_json_v1(request))
            }
            other => RResult::RErr(RString::from(format!(
                "engine.world.environment invoke_json unknown target method '{other}'"
            ))),
        }
    }
}
