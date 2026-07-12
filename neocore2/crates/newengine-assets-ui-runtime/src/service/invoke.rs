use super::*;

pub(super) fn invoke_json(
    state: &mut AssetsUiRuntimeState,
    payload: Blob,
) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(assets_ui_method::COMPILE_DOCUMENT_V1);
    let request = value.get("request").cloned().unwrap_or_default();

    match method {
        assets_ui_method::COMPILE_DOCUMENT_V1 => {
            let request =
                serde_json::from_value::<AssetsUiCompileRequest>(request).unwrap_or_default();
            match handlers::compile_document(state, request.clone()) {
                Ok(response) => ok_json(response),
                Err(error) => ok_json(compile_request::error_response_from_compile_error(
                    error, &request,
                )),
            }
        }
        assets_ui_method::DOCUMENT_V1 | assets_ui_method::DUMP_XMLCENTRAL_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request).unwrap_or_default();
            let schema = if method == assets_ui_method::DUMP_XMLCENTRAL_V1 {
                "newengine.assets.ui.xmlcentral_dump.v1"
            } else {
                "newengine.assets.ui.document.response.v1"
            };
            match handlers::document(state, request, schema) {
                Ok(response) => ok_json(response),
                Err(error) => ok_json(compile_request::error_response_from_message(error)),
            }
        }
        assets_ui_method::VALIDATE_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request).unwrap_or_default();
            match handlers::validate(state, request) {
                Ok(response) => ok_json(response),
                Err(error) => ok_json(compile_request::error_response_from_message(error)),
            }
        }
        assets_ui_method::DEPENDENCIES_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request).unwrap_or_default();
            match handlers::dependencies(state, request) {
                Ok(response) => ok_json(response),
                Err(error) => ok_json(compile_request::error_response_from_message(error)),
            }
        }
        assets_ui_method::INSPECT_DIALECT_V1 => {
            let request = serde_json::from_value::<AssetsUiDialectInspectRequest>(request)
                .unwrap_or_default();
            ok_json(compile_request::inspect_dialect(state, request))
        }
        assets_ui_method::INVALIDATE_V1 => {
            let request =
                serde_json::from_value::<AssetsUiInvalidateRequest>(request).unwrap_or_default();
            ok_json(compile_request::invalidate_caches(state, request))
        }
        assets_ui_method::MANIFEST_V1
        | assets_ui_method::ENTRY_V1
        | assets_ui_method::REGISTRY_V1
        | assets_ui_method::BINDING_PLAN_V1 => {
            let request = serde_json::from_value::<AssetsUiRefRequest>(request).unwrap_or_default();
            match handlers::compile_from_ref(state, request) {
                Ok(response) => ok_json(response),
                Err(error) => ok_json(compile_request::error_response_from_message(error)),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.ui: unknown invoke_json method '{other}'"
        ))),
    }
}
