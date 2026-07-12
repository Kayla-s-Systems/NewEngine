use super::*;

pub(crate) fn compile_request_from_ref(request: AssetsUiRefRequest) -> AssetsUiCompileRequest {
    AssetsUiCompileRequest {
        document_ref: first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]),
        ui_ref: String::new(),
        logical_path: request.logical_path,
        entry: request.entry,
        mount_runtime: false,
        ..Default::default()
    }
}

pub(crate) fn error_response_from_message(message: String) -> AssetsUiDiagnosticResponse {
    AssetsUiDiagnosticResponse {
        message,
        ..Default::default()
    }
}

pub(crate) fn error_response_from_compile_error(
    message: String,
    request: &AssetsUiCompileRequest,
) -> AssetsUiDiagnosticResponse {
    let combined = first_non_empty([request.document_ref.as_str(), request.ui_ref.as_str()]);
    let (path, entry) = if !combined.trim().is_empty() {
        split_ref(&combined)
    } else {
        (
            normalize_logical_path(&request.logical_path),
            normalize_entry(&request.entry),
        )
    };
    let entry = if entry.trim().is_empty() {
        "surface".to_owned()
    } else {
        entry
    };
    AssetsUiDiagnosticResponse {
        document_ref: if path.trim().is_empty() {
            String::new()
        } else {
            format!("{}@{}", path, entry)
        },
        logical_path: path.clone(),
        entry: entry.clone(),
        entry_id: entry,
        source_span: UiSourceSpan {
            source_ref: path,
            line: 0,
            column: 0,
        },
        message,
        ..Default::default()
    }
}
