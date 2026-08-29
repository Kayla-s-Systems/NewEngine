#![forbid(unsafe_op_in_unsafe_fn)]

//! Typed client for selectorless, asset-backed script modules routed through
//! the generic `engine.scripting` gateway.

mod editor;
pub use editor::*;

use std::collections::BTreeMap;

use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_scripting_api::{
    decode_scripting_module_load_bytes_response, decode_scripting_response_bytes,
    encode_scripting_module_load_bytes_request, encode_scripting_request_bytes, ScriptDiagnostic,
    ScriptModuleRef, ScriptingCompletionRequest, ScriptingCompletionResponse,
    ScriptingModuleLoadBytesRequest, ScriptingRequestBytes, ScriptingResponseStatus,
    ScriptingSignatureHelpRequest, ScriptingSignatureHelpResponse, ScriptingToolingCatalog,
    ScriptingToolingFunction, ENGINE_SCRIPTING_SERVICE_ID,
    SCRIPTING_SERVICE_METHOD_COMPLETE_JSON_V1, SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_SET_TOOLING_CATALOG_JSON_V1,
    SCRIPTING_SERVICE_METHOD_SIGNATURE_HELP_JSON_V1,
};
use serde::{de::DeserializeOwned, Serialize};

#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptingToolingClient;

impl ScriptingToolingClient {
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    pub fn complete(
        &self,
        request: &ScriptingCompletionRequest,
    ) -> Result<ScriptingCompletionResponse, String> {
        ensure_scripting_gateway()?;
        let payload = serde_json::to_vec(request)
            .map_err(|error| format!("scripting completion request JSON encode failed: {error}"))?;
        let response = newengine_core::call_service_v1(
            ENGINE_SCRIPTING_SERVICE_ID,
            SCRIPTING_SERVICE_METHOD_COMPLETE_JSON_V1,
            &payload,
        )
        .map_err(|error| format!("engine.scripting completion failed: {error}"))?;
        serde_json::from_slice(&response)
            .map_err(|error| format!("scripting completion response JSON decode failed: {error}"))
    }

    pub fn signature_help(
        &self,
        request: &ScriptingSignatureHelpRequest,
    ) -> Result<ScriptingSignatureHelpResponse, String> {
        ensure_scripting_gateway()?;
        let payload = serde_json::to_vec(request).map_err(|error| {
            format!("scripting signature-help request JSON encode failed: {error}")
        })?;
        let response = newengine_core::call_service_v1(
            ENGINE_SCRIPTING_SERVICE_ID,
            SCRIPTING_SERVICE_METHOD_SIGNATURE_HELP_JSON_V1,
            &payload,
        )
        .map_err(|error| format!("engine.scripting signature help failed: {error}"))?;
        serde_json::from_slice(&response).map_err(|error| {
            format!("scripting signature-help response JSON decode failed: {error}")
        })
    }

    pub fn set_tooling_catalog(&self, catalog: &ScriptingToolingCatalog) -> Result<(), String> {
        ensure_scripting_gateway()?;
        let payload = serde_json::to_vec(catalog)
            .map_err(|error| format!("scripting tooling catalog JSON encode failed: {error}"))?;
        newengine_core::call_service_v1(
            ENGINE_SCRIPTING_SERVICE_ID,
            SCRIPTING_SERVICE_METHOD_SET_TOOLING_CATALOG_JSON_V1,
            &payload,
        )
        .map_err(|error| format!("engine.scripting tooling catalog install failed: {error}"))?;
        Ok(())
    }

    pub fn refresh_generated_northstar_catalog(&self) -> Result<ScriptingToolingCatalog, String> {
        if !newengine_core::has_engine_gateway_route(newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID)
        {
            return Err(format!(
                "schema gateway '{}' is unavailable",
                newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID
            ));
        }
        let request = serde_json::json!({
            "target_language": "typescript",
            "module_id": "northstar.typescript.generated"
        });
        let payload = serde_json::to_vec(&request)
            .map_err(|error| format!("schema binding-manifest request encode failed: {error}"))?;
        let response = newengine_core::call_service_v1(
            newengine_schema_api::ENGINE_SCHEMA_SERVICE_ID,
            newengine_schema_api::schema_method::BINDING_MANIFEST_V1,
            &payload,
        )
        .map_err(|error| format!("engine.schema binding manifest failed: {error}"))?;
        let manifest: newengine_schema_api::SchemaBindingManifestV1 =
            serde_json::from_slice(&response).map_err(|error| {
                format!("schema binding-manifest response decode failed: {error}")
            })?;
        let catalog = tooling_catalog_from_schema_manifest(&manifest);
        self.set_tooling_catalog(&catalog)?;
        Ok(catalog)
    }
}

fn tooling_catalog_from_schema_manifest(
    manifest: &newengine_schema_api::SchemaBindingManifestV1,
) -> ScriptingToolingCatalog {
    let functions = manifest
        .functions
        .iter()
        .map(|function| {
            let namespace = function
                .gateway
                .trim()
                .strip_prefix("engine.")
                .unwrap_or(function.gateway.trim())
                .to_owned();
            ScriptingToolingFunction {
                namespace,
                name: function.name.clone(),
                parameters: if function.request_type.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![format!("request: {}", function.request_type)]
                },
                return_type: function.response_type.clone(),
                detail: format!("{} :: {}", function.gateway, function.method),
                gateway: function.gateway.clone(),
                method: function.method.clone(),
            }
        })
        .collect::<Vec<_>>();
    let revision = stable_tooling_catalog_revision(manifest);
    ScriptingToolingCatalog {
        revision,
        root_namespace: "NorthStar".to_owned(),
        functions,
        diagnostics: manifest
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect(),
        ..ScriptingToolingCatalog::default()
    }
}

fn stable_tooling_catalog_revision(
    manifest: &newengine_schema_api::SchemaBindingManifestV1,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in serde_json::to_vec(manifest).unwrap_or_default() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Clone, Debug)]
pub struct AssetBackedScriptClient {
    script_ref: String,
    purpose: String,
}

impl AssetBackedScriptClient {
    #[inline]
    pub fn new(script_ref: impl Into<String>, purpose: impl Into<String>) -> Self {
        Self {
            script_ref: script_ref.into(),
            purpose: purpose.into(),
        }
    }

    #[inline]
    pub fn script_ref(&self) -> &str {
        &self.script_ref
    }

    pub fn load_module(&self) -> Result<(), String> {
        ensure_gateways()?;
        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let (module_bytes, origin) = load_script_module_bytes(&assets, &self.script_ref)?;
        let request = ScriptingModuleLoadBytesRequest {
            module_ref: ScriptModuleRef::new(&self.script_ref),
            module_bytes,
            permissions: Vec::new(),
            metadata: BTreeMap::from([
                ("purpose".to_owned(), self.purpose.clone()),
                (
                    "content_type".to_owned(),
                    "text/plain; charset=utf-8".to_owned(),
                ),
                ("module_origin".to_owned(), origin.as_str().to_owned()),
                (
                    "asset_resolution_policy".to_owned(),
                    newengine_assets::ASSET_RESOLUTION_POLICY_COMPILED_FIRST_SOURCE_FALLBACK_V1
                        .to_owned(),
                ),
            ]),
        };
        let payload = encode_scripting_module_load_bytes_request(&request);
        let response_bytes = newengine_core::call_service_v1(
            ENGINE_SCRIPTING_SERVICE_ID,
            SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
            &payload,
        )
        .map_err(|error| format!("engine.scripting module load failed: {error}"))?;
        let response =
            decode_scripting_module_load_bytes_response(&response_bytes).map_err(|error| {
                format!("engine.scripting module-load response decode failed: {error}")
            })?;
        if response.ok {
            Ok(())
        } else {
            Err(format!(
                "script module '{}' was rejected: {}",
                self.script_ref,
                diagnostics_summary(&response.diagnostics)
            ))
        }
    }

    pub fn invoke_bytes(
        &self,
        request_id: impl Into<String>,
        operation: &str,
        payload_bytes: Vec<u8>,
        context_bytes: Vec<u8>,
        metadata: BTreeMap<String, String>,
    ) -> Result<Vec<u8>, String> {
        ensure_gateways()?;
        let request = ScriptingRequestBytes {
            request_id: request_id.into(),
            script_ref: self.script_ref.clone(),
            operation: operation.to_owned(),
            payload_bytes,
            context_bytes,
            permissions: Vec::new(),
            metadata,
        };
        let payload = encode_scripting_request_bytes(&request);
        let response_bytes = newengine_core::call_service_v1(
            ENGINE_SCRIPTING_SERVICE_ID,
            SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
            &payload,
        )
        .map_err(|error| format!("engine.scripting invoke failed: {error}"))?;
        let response = decode_scripting_response_bytes(&response_bytes)
            .map_err(|error| format!("engine.scripting response decode failed: {error}"))?;
        if response.status != ScriptingResponseStatus::Ok {
            return Err(format!(
                "script export '{}::{}' failed with status {:?}: {}",
                self.script_ref,
                operation,
                response.status,
                diagnostics_summary(&response.diagnostics)
            ));
        }
        Ok(response.payload_bytes)
    }

    pub fn invoke_json<Request, Response>(
        &self,
        request_id: impl Into<String>,
        operation: &str,
        request: &Request,
        mut metadata: BTreeMap<String, String>,
    ) -> Result<Response, String>
    where
        Request: Serialize + ?Sized,
        Response: DeserializeOwned,
    {
        let payload = serde_json::to_vec(request)
            .map_err(|error| format!("script request JSON encode failed: {error}"))?;
        metadata
            .entry("payload_format".to_owned())
            .or_insert_with(|| "json".to_owned());
        let response = self.invoke_bytes(request_id, operation, payload, Vec::new(), metadata)?;
        serde_json::from_slice(&response)
            .map_err(|error| format!("script response JSON decode failed: {error}"))
    }

    pub fn invoke_json_unit<Response>(
        &self,
        request_id: impl Into<String>,
        operation: &str,
        metadata: BTreeMap<String, String>,
    ) -> Result<Response, String>
    where
        Response: DeserializeOwned,
    {
        self.invoke_json(request_id, operation, &serde_json::Value::Null, metadata)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScriptModuleOrigin {
    CompiledYsc,
    SourceFallback,
}

impl ScriptModuleOrigin {
    const fn as_str(self) -> &'static str {
        match self {
            Self::CompiledYsc => "compiled_ysc",
            Self::SourceFallback => "source_fallback",
        }
    }
}

fn load_script_module_bytes(
    assets: &AssetServiceClient,
    script_ref: &str,
) -> Result<(Vec<u8>, ScriptModuleOrigin), String> {
    let request = AssetDecodeRequest {
        logical_path: script_ref.to_owned(),
        output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
        selector: serde_json::Value::Null,
    };
    match assets.decode_v1(&request) {
        Ok(bytes) => Ok((bytes, ScriptModuleOrigin::CompiledYsc)),
        Err(decode_error) => {
            // Source mounts use the same canonical logical .ysc id with an alias to
            // Source/scripts/*.ts|*.lua|*.js. If no compiled YSC exists, text_v1
            // resolves that source candidate and returns its UTF-8 bytes directly.
            // If a compiled YSC exists but is corrupt, compiled-first resolution keeps
            // selecting it and text_v1 rejects the binary payload, so this does not
            // silently hide broken runtime artifacts behind authoring source.
            match assets.text_v1(script_ref) {
                Ok(bytes) => Ok((bytes, ScriptModuleOrigin::SourceFallback)),
                Err(source_error) => Err(format!(
                    "failed to load selectorless script module '{script_ref}': compiled YSC decode failed: {decode_error}; source fallback failed: {source_error}"
                )),
            }
        }
    }
}

fn ensure_gateways() -> Result<(), String> {
    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        return Err(format!(
            "asset gateway '{}' is unavailable",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        ));
    }
    ensure_scripting_gateway()
}

fn ensure_scripting_gateway() -> Result<(), String> {
    if !newengine_core::has_engine_gateway_route(ENGINE_SCRIPTING_SERVICE_ID) {
        return Err(format!(
            "scripting gateway '{}' is unavailable",
            ENGINE_SCRIPTING_SERVICE_ID
        ));
    }
    Ok(())
}

fn diagnostics_summary(diagnostics: &[ScriptDiagnostic]) -> String {
    if diagnostics.is_empty() {
        return "no provider diagnostics".to_owned();
    }
    diagnostics
        .iter()
        .map(|diagnostic| {
            if diagnostic.code.trim().is_empty() {
                diagnostic.message.clone()
            } else {
                format!("{}: {}", diagnostic.code, diagnostic.message)
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_keeps_selectorless_module_reference() {
        let client = AssetBackedScriptClient::new("scripts/fps_gameplay.ysc", "test");
        assert_eq!(client.script_ref(), "scripts/fps_gameplay.ysc");
        assert!(!client.script_ref().contains('@'));
    }

    #[test]
    fn tooling_client_is_provider_neutral() {
        let _client = ScriptingToolingClient::new();
        let request = ScriptingCompletionRequest {
            language_id: "typescript".to_owned(),
            source_text: "ret".to_owned(),
            cursor_byte_offset: 3,
            ..ScriptingCompletionRequest::default()
        };
        assert_eq!(
            request.schema,
            newengine_scripting_api::SCRIPTING_COMPLETION_REQUEST_SCHEMA_V1
        );
    }

    #[test]
    fn schema_binding_manifest_generates_northstar_tooling_catalog() {
        let manifest = newengine_schema_api::SchemaBindingManifestV1 {
            functions: vec![newengine_schema_api::SchemaBindingFunctionV1 {
                name: "raycast".to_owned(),
                method: "physics.raycast_v1".to_owned(),
                request_type: "RaycastRequest".to_owned(),
                response_type: "RaycastHit".to_owned(),
                gateway: "engine.physics".to_owned(),
            }],
            ..Default::default()
        };
        let catalog = tooling_catalog_from_schema_manifest(&manifest);
        assert_eq!(catalog.root_namespace, "NorthStar");
        assert_ne!(catalog.revision, 0);
        assert_eq!(catalog.functions.len(), 1);
        let function = &catalog.functions[0];
        assert_eq!(function.namespace, "physics");
        assert_eq!(function.name, "raycast");
        assert_eq!(function.parameters, vec!["request: RaycastRequest"]);
        assert_eq!(function.return_type, "RaycastHit");
    }

    #[test]
    fn module_origin_metadata_is_language_neutral() {
        assert_eq!(ScriptModuleOrigin::CompiledYsc.as_str(), "compiled_ysc");
        assert_eq!(
            ScriptModuleOrigin::SourceFallback.as_str(),
            "source_fallback"
        );
    }
}
