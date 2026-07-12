use newengine_scripting_api::{
    ScriptDiagnostic, ScriptModuleRef, ScriptModuleRefValidationResponse, ScriptingModuleRef,
};

#[inline]
pub fn validate_script_module_ref(
    module_ref: ScriptModuleRef,
) -> ScriptModuleRefValidationResponse {
    let mut diagnostics = Vec::new();
    let reference = module_ref.reference.trim();
    if reference.is_empty() {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_EMPTY_MODULE_REF",
            "Script module reference must not be empty.",
        ));
    }
    if !module_ref.is_ysc_entry_ref() {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_MODULE_REF_NOT_YSC_ENTRY",
            "Runtime script modules must be addressed as file.ysc@entry.",
        ));
    }
    if reference.contains("..") || reference.contains('\\') || reference.starts_with('/') {
        diagnostics.push(ScriptDiagnostic::error(
            "SCRIPTING_UNSAFE_MODULE_REF",
            "Script module references must be normalized VFS logical paths.",
        ));
    }
    ScriptModuleRefValidationResponse {
        ok: diagnostics.is_empty(),
        module_ref,
        diagnostics,
    }
}

#[inline]
pub(crate) fn normalized_module_key(module_ref: &ScriptingModuleRef) -> String {
    if !module_ref.module_id.trim().is_empty() {
        return module_ref.module_id.trim().to_ascii_lowercase();
    }
    module_ref
        .reference
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_ascii_lowercase()
}
