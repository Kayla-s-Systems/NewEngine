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
    default_provider::{
        build_default_environment_frame, build_default_environment_frame_with_history,
        deterministic_key,
    },
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
            build_default_environment_frame_with_history(
                self.provider,
                self.provider_route,
                request,
                Some(&self.last_frame),
            )
        };
        self.last_frame = frame.clone();
        frame
    }

    pub(crate) fn sample_at_position_json_v1(
        &self,
        request: EnvironmentSampleAtPositionRequest,
    ) -> EnvironmentSampleAtPositionResponse {
        let local = request.cell.and_then(|coord| {
            request
                .frame
                .spatial_atmosphere
                .iter()
                .find(|cell| cell.cell == coord)
        });
        let (atmosphere, weather, wind, visibility_multiplier) = if let Some(cell) = local {
            (
                &cell.atmosphere,
                &cell.weather,
                &cell.wind,
                (cell.atmosphere.visibility_distance_meters / 20_000.0).clamp(0.05, 1.0),
            )
        } else {
            (
                &request.frame.atmosphere,
                &request.frame.weather,
                &request.frame.wind,
                request.frame.gameplay_modifiers.visibility_multiplier,
            )
        };
        let speed = wind.global_speed_mps;
        let direction = wind.global_direction;
        let mut diagnostics = request.frame.diagnostics.clone();
        diagnostics.reasons.push(if let Some(cell) = local {
            format!(
                "sample_at_position source=mesoscale cell=({}, {})",
                cell.cell.x, cell.cell.z
            )
        } else {
            "sample_at_position source=global_column_fallback".to_owned()
        });
        EnvironmentSampleAtPositionResponse {
            position: request.position,
            cell: request.cell,
            visibility_multiplier,
            wind_velocity: Vec3Dto::new(
                direction.x * speed,
                direction.y * speed,
                direction.z * speed,
            ),
            surface_pressure_hpa: atmosphere.surface_pressure_hpa,
            temperature_celsius: atmosphere.temperature_celsius,
            dew_point_celsius: atmosphere.dew_point_celsius,
            relative_humidity: atmosphere.humidity,
            specific_humidity_g_per_kg: atmosphere.specific_humidity_g_per_kg,
            air_density_kg_m3: atmosphere.air_density_kg_m3,
            cloud_water_path_kg_m2: atmosphere.cloud_water_path_kg_m2,
            precipitation_rate_mm_per_hour: weather.precipitation.rate_mm_per_hour,
            cape_j_per_kg: atmosphere.cape_j_per_kg,
            cin_j_per_kg: atmosphere.cin_j_per_kg,
            weather_tags: weather.tags.clone(),
            diagnostics,
        }
    }

    pub(crate) fn inspect_text_v1(&self) -> String {
        let frame = &self.last_frame;
        let atmosphere = &frame.atmosphere;
        let weather = &frame.weather;
        let wind = &frame.wind;
        let mut lines = vec![
            format!(
                "world_time={:.3}s profile={} weather={:?}",
                frame.world_time_seconds,
                frame.global.active_environment_profile,
                weather.state
            ),
            format!(
                "P={:.2}hPa T={:.2}C Td={:.2}C RH={:.3} q={:.3}g/kg rho={:.4}kg/m3",
                atmosphere.surface_pressure_hpa,
                atmosphere.temperature_celsius,
                atmosphere.dew_point_celsius,
                atmosphere.humidity,
                atmosphere.specific_humidity_g_per_kg,
                atmosphere.air_density_kg_m3,
            ),
            format!(
                "LCL={:.1}m cloud_top={:.1}m CWP={:.4}kg/m2 PW={:.3}mm CAPE={:.1}J/kg CIN={:.1}J/kg",
                atmosphere.lifting_condensation_level_meters,
                atmosphere.convective_cloud_top_meters,
                atmosphere.cloud_water_path_kg_m2,
                atmosphere.precipitable_water_mm,
                atmosphere.cape_j_per_kg,
                atmosphere.cin_j_per_kg,
            ),
            format!(
                "precip={:?} {:.3}mm/h thunder={:.3} visibility={:.0}m fog={:.3} aerosol={:.3}",
                weather.precipitation.kind,
                weather.precipitation.rate_mm_per_hour,
                weather.thunder.probability,
                atmosphere.visibility_distance_meters,
                atmosphere.fog_density,
                atmosphere.aerosol_density,
            ),
            format!(
                "wind={:.2}m/s dir=({:.3},{:.3},{:.3}) gust={:.3} mesoscale_cells={} cell_size={:.1}m objects={}",
                wind.global_speed_mps,
                wind.global_direction.x,
                wind.global_direction.y,
                wind.global_direction.z,
                wind.gust_strength,
                frame.spatial_atmosphere.len(),
                frame.spatial_cell_size_meters,
                frame.environment_objects.len(),
            ),
        ];
        lines.extend(
            frame
                .diagnostics
                .reasons
                .iter()
                .filter(|reason| reason.contains("graph=") || reason.contains("radiation "))
                .map(|reason| format!("cause: {reason}")),
        );
        lines.join("\n")
    }

    pub(crate) fn inspect_cell_text_v1(&self, payload: &[u8]) -> Result<String, String> {
        let args = String::from_utf8_lossy(payload);
        let mut parts = args.split_whitespace();
        let x = parts
            .next()
            .ok_or_else(|| "usage: env.cell <x> <z>".to_owned())?
            .parse::<i32>()
            .map_err(|_| "env.cell x must be an integer".to_owned())?;
        let z = parts
            .next()
            .ok_or_else(|| "usage: env.cell <x> <z>".to_owned())?
            .parse::<i32>()
            .map_err(|_| "env.cell z must be an integer".to_owned())?;
        if parts.next().is_some() {
            return Err("usage: env.cell <x> <z>".to_owned());
        }
        let coord = newengine_world_api::WorldCellCoord::new(x, z);
        let cell = self
            .last_frame
            .spatial_atmosphere
            .iter()
            .find(|cell| cell.cell == coord)
            .ok_or_else(|| format!("atmospheric cell ({x},{z}) is not resident"))?;
        Ok(format!(
            "cell=({x},{z}) terrain={:.1}m albedo={:.3} moisture={:.3} roughness={:.4}m\nP={:.2}hPa T={:.2}C Td={:.2}C RH={:.3} q={:.3}g/kg rho={:.4}kg/m3\nLCL={:.1}m top={:.1}m CWP={:.4}kg/m2 PW={:.3}mm CAPE={:.1}J/kg CIN={:.1}J/kg\nweather={:?} precip={:?} {:.3}mm/h thunder={:.3} visibility={:.0}m\nwind={:.2}m/s dir=({:.3},{:.3},{:.3}) gust={:.3}",
            cell.surface.terrain_elevation_meters,
            cell.surface.albedo,
            cell.surface.moisture_availability,
            cell.surface.roughness_length_meters,
            cell.atmosphere.surface_pressure_hpa,
            cell.atmosphere.temperature_celsius,
            cell.atmosphere.dew_point_celsius,
            cell.atmosphere.humidity,
            cell.atmosphere.specific_humidity_g_per_kg,
            cell.atmosphere.air_density_kg_m3,
            cell.atmosphere.lifting_condensation_level_meters,
            cell.atmosphere.convective_cloud_top_meters,
            cell.atmosphere.cloud_water_path_kg_m2,
            cell.atmosphere.precipitable_water_mm,
            cell.atmosphere.cape_j_per_kg,
            cell.atmosphere.cin_j_per_kg,
            cell.weather.state,
            cell.weather.precipitation.kind,
            cell.weather.precipitation.rate_mm_per_hour,
            cell.weather.thunder.probability,
            cell.atmosphere.visibility_distance_meters,
            cell.wind.global_speed_mps,
            cell.wind.global_direction.x,
            cell.wind.global_direction.y,
            cell.wind.global_direction.z,
            cell.wind.gust_strength,
        ))
    }

    pub(crate) fn objects_text_v1(&self) -> String {
        if self.last_frame.environment_objects.is_empty() {
            return "no physical mesoscale environment objects".to_owned();
        }
        self.last_frame
            .environment_objects
            .iter()
            .map(|object| {
                let cells = object
                    .owning_cells
                    .iter()
                    .map(|cell| format!("({}, {})", cell.x, cell.z))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "id={} kind={:?} cells=[{}] bounds=({:.0},{:.0},{:.0})..({:.0},{:.0},{:.0}) tags={}",
                    object.id.stable_id,
                    object.kind,
                    cells,
                    object.bounds.min.x,
                    object.bounds.min.y,
                    object.bounds.min.z,
                    object.bounds.max.x,
                    object.bounds.max.y,
                    object.bounds.max.z,
                    object.tags.join("|")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
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
