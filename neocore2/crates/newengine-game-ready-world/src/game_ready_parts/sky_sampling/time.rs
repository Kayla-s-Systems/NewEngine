use super::*;

pub(crate) fn time_snapshot_for_sky_cycle() -> Option<newengine_core::time::TimeSnapshotV1> {
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

pub(crate) fn authored_time_snapshot_for_sky_cycle(
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

pub(crate) fn environment_frame_for_sky_cycle(
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
pub(crate) fn sync_game_ready_day_night_to_engine_time(day_night: &GameReadyDayNightSpec) {
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
