use newengine_scripting_api::{
    decode_scripting_module_load_bytes_request, decode_scripting_request_bytes,
    encode_scripting_module_load_bytes_request, encode_scripting_request_bytes, ScriptModuleRef,
    ScriptingModuleLoadBytesRequest, ScriptingRequestBytes, ScriptingResponseStatus,
};

use crate::{validate_script_module_ref, ScriptingRuntimeState};

#[test]
fn baseline_runtime_returns_empty_opaque_response() {
    let mut state = ScriptingRuntimeState::default();
    let response = state.invoke_bytes(ScriptingRequestBytes {
        request_id: "r1".to_owned(),
        ..ScriptingRequestBytes::default()
    });
    assert_eq!(response.request_id, "r1");
    assert_eq!(response.status, ScriptingResponseStatus::Empty);
    assert!(response.payload_bytes.is_empty());
}

#[test]
fn validation_requires_ysc_module_ref() {
    let bad = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.source@entry"));
    assert!(!bad.ok);
    let good = validate_script_module_ref(ScriptModuleRef::new("scripts/foo.ysc"));
    assert!(good.ok);
}

#[test]
fn binary_load_stores_opaque_byte_count() {
    let mut state = ScriptingRuntimeState::default();
    let response = state.load_module_bytes(ScriptingModuleLoadBytesRequest {
        module_ref: ScriptModuleRef::new("scripts/foo.ysc"),
        module_bytes: vec![1, 2, 3, 4],
        ..ScriptingModuleLoadBytesRequest::default()
    });
    assert!(response.ok);
    assert_eq!(response.module.module_bytes_len, 4);
    assert_eq!(state.runtime_info().loaded_module_count, 1);
}

#[test]
fn binary_wire_methods_accept_binary_envelopes() {
    let load = ScriptingModuleLoadBytesRequest {
        module_ref: ScriptModuleRef::new("scripts/foo.ysc"),
        module_bytes: vec![1, 2, 3],
        ..ScriptingModuleLoadBytesRequest::default()
    };
    assert!(decode_scripting_module_load_bytes_request(
        &encode_scripting_module_load_bytes_request(&load)
    )
    .is_ok());

    let request = ScriptingRequestBytes {
        request_id: "r1".to_owned(),
        ..ScriptingRequestBytes::default()
    };
    assert!(decode_scripting_request_bytes(&encode_scripting_request_bytes(&request)).is_ok());
}
