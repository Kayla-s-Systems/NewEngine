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

#[test]
fn completion_tooling_method_is_part_of_the_generic_scripting_surface() {
    assert!(scripting_service_methods()
        .iter()
        .any(|method| *method == SCRIPTING_SERVICE_METHOD_COMPLETE_JSON_V1));

    let request = ScriptingCompletionRequest::default();
    assert_eq!(request.schema, SCRIPTING_COMPLETION_REQUEST_SCHEMA_V1);
    let response = ScriptingCompletionResponse::default();
    assert_eq!(response.schema, SCRIPTING_COMPLETION_RESPONSE_SCHEMA_V1);
}

#[test]
fn signature_and_tooling_catalog_methods_are_generic_scripting_surface() {
    let methods = scripting_service_methods();
    assert!(methods
        .iter()
        .any(|method| *method == SCRIPTING_SERVICE_METHOD_SIGNATURE_HELP_JSON_V1));
    assert!(methods
        .iter()
        .any(|method| *method == SCRIPTING_SERVICE_METHOD_SET_TOOLING_CATALOG_JSON_V1));

    let signature_request = ScriptingSignatureHelpRequest::default();
    assert_eq!(
        signature_request.schema,
        SCRIPTING_SIGNATURE_HELP_REQUEST_SCHEMA_V1
    );
    let signature_response = ScriptingSignatureHelpResponse::default();
    assert_eq!(
        signature_response.schema,
        SCRIPTING_SIGNATURE_HELP_RESPONSE_SCHEMA_V1
    );

    let catalog = ScriptingToolingCatalog::default();
    assert_eq!(catalog.schema, SCRIPTING_TOOLING_CATALOG_SCHEMA_V1);
    assert_eq!(catalog.root_namespace, "NorthStar");
}
