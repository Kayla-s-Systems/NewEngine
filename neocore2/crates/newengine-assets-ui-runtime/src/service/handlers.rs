use super::*;

pub(super) fn compile_document(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiCompileRequest,
) -> Result<AssetsUiCompileResponse, String> {
    compile_request::compile_document(state, request)
}

pub(super) fn compile_from_ref(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<AssetsUiCompileResponse, String> {
    compile_request::compile_document(state, compile_request::compile_request_from_ref(request))
}

pub(super) fn document(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
    schema: &'static str,
) -> Result<serde_json::Value, String> {
    let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;
    Ok(serde_json::json!({
        "ok": true,
        "schema": schema,
        "document_ref": resolved.document_ref,
        "logical_path": resolved.logical_path,
        "vfs_path": resolved.vfs_path,
        "entry": resolved.entry,
        "xmlcentral": xml,
    }))
}

pub(super) fn validate(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<AssetsUiDiagnosticResponse, String> {
    let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;
    Ok(AssetsUiDiagnosticResponse {
        ok: true,
        document_ref: resolved.document_ref.clone(),
        logical_path: resolved.logical_path,
        entry_id: resolved.entry.clone(),
        entry: resolved.entry,
        source_span: source_span_for_offset(&xml, 0, &resolved.document_ref),
        message: format!(
            "valid binary .neui decoded to XMLcentral bytes={} root={}",
            xml.len(),
            root_name(&xml).unwrap_or("unknown")
        ),
        ..Default::default()
    })
}

pub(super) fn dependencies(
    state: &mut AssetsUiRuntimeState,
    request: AssetsUiRefRequest,
) -> Result<serde_json::Value, String> {
    let (xml, _, resolved) = compile_request::load_xmlcentral(state, request)?;
    Ok(serde_json::json!({
        "ok": true,
        "schema": "newengine.assets.ui.dependencies.response.v1",
        "document_ref": resolved.document_ref,
        "logical_path": resolved.logical_path,
        "entry": resolved.entry,
        "dependencies": extract_dependencies(&xml),
    }))
}
