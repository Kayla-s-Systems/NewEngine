use super::*;

#[test]
fn module_ref_detects_ysc_module() {
    let module_ref = ScriptModuleRef::new("scripts/missions/intro.ysc");
    assert!(module_ref.is_ysc_module_ref());
    assert_eq!(module_ref.module_id, "scripts_missions_intro_ysc");
}

#[test]
fn service_info_has_no_known_language_list() {
    let info = ScriptingServiceInfo::default();
    assert!(info
        .features
        .iter()
        .any(|item| item == "no-language-whitelist"));
    assert!(info
        .methods
        .iter()
        .any(|item| item == SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1));
    assert!(!info
        .features
        .iter()
        .any(|item| item.contains("known-language")));
    assert!(!info.features.iter().any(|item| item.contains("compat")));
}

#[test]
fn response_preserves_request_id_only() {
    let request = ScriptingRequestBytes {
        request_id: "req-1".to_owned(),
        ..ScriptingRequestBytes::default()
    };
    let response = ScriptingResponseBytes::empty_for(&request);
    assert_eq!(response.request_id, "req-1");
    assert_eq!(response.status, ScriptingResponseStatus::Empty);
    assert!(response.payload_bytes.is_empty());
}
