use super::*;

#[test]
fn environment_service_ids_are_world_subdomain_gateway_first() {
    assert_eq!(
        ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
        "engine.world.environment"
    );
    assert_eq!(
        WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.engine_gateway_id,
        ENGINE_WORLD_ENVIRONMENT_SERVICE_ID
    );
    assert_eq!(
        WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.provider_service_id,
        WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID
    );
    assert_eq!(
        WORLD_ENVIRONMENT_BACKEND_SERVICE_SPEC.backend_capability_id,
        WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID
    );
}

#[test]
fn environment_request_has_no_renderer_state() {
    let json = serde_json::to_value(EnvironmentFrameRequest::default()).unwrap();
    assert!(json.get("time").is_some());
    assert!(json.get("observer_position").is_some());
    assert!(json.get("vulkan").is_none());
    assert!(json.get("gpu_cloud_history").is_none());
    assert!(json.get("renderer_exposure_buffer").is_none());
}

#[test]
fn environment_frame_has_consumer_packets_without_renderer_ownership() {
    let frame = EnvironmentFrameDto::default();
    assert_eq!(frame.consumer_packets.render.cloud_coverage, 0.0);
    assert!(frame.consumer_packets.ai.is_night);
    assert_eq!(frame.consumer_packets.physics.precipitation_intensity, 0.0);
}
