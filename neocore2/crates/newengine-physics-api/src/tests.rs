use super::*;

#[test]
fn capability_profiles_preserve_expected_features() {
    let deterministic = PhysicsBackendCapabilities::deterministic_default();
    assert!(deterministic.supports(PhysicsFeature::Contacts));
    assert!(!deterministic.supports(PhysicsFeature::NativeBackend));

    let native = PhysicsBackendCapabilities::native_backend_default();
    assert!(native.supports(PhysicsFeature::NativeBackend));
    assert!(native.supports(PhysicsFeature::MeshColliders));
}

#[test]
fn frame_input_empty_preserves_defaults() {
    let frame = PhysicsFrameInput::empty(7, 11, 1.0 / 60.0);
    assert_eq!(frame.frame_index, 7);
    assert_eq!(frame.fixed_tick, 11);
    assert_eq!(frame.gravity, 9.81);
    assert_eq!(frame.contact_skin, 0.035);
    assert!(frame.bodies.is_empty());
}

#[test]
fn protocol_roundtrips_json() {
    let request = PhysicsServiceRequest::StepFrame(PhysicsFrameInput::empty(1, 2, 0.016));
    let bytes = encode_json(&request).unwrap();
    let decoded: PhysicsServiceRequest = decode_json(&bytes).unwrap();
    assert!(matches!(decoded, PhysicsServiceRequest::StepFrame(_)));
}

#[test]
fn heightfield_validation_detects_square_payload() {
    let collider = HeightfieldColliderDto {
        sample_count_x: 2,
        sample_count_z: 2,
        spacing: [1.0, 1.0],
        local_origin: [0.0, 0.0, 0.0],
        heights: vec![0.0; 4],
        min_height: 0.0,
        max_height: 0.0,
    };
    assert!(collider.is_square_for_native_heightfield());
}
