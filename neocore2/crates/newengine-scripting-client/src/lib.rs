#![forbid(unsafe_op_in_unsafe_fn)]

//! Typed client for selectorless, asset-backed script modules routed through
//! the generic `engine.scripting` gateway.

use std::collections::BTreeMap;

use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_scripting_api::{
    decode_scripting_module_load_bytes_response, decode_scripting_response_bytes,
    encode_scripting_module_load_bytes_request, encode_scripting_request_bytes, ScriptDiagnostic,
    ScriptModuleRef, ScriptingModuleLoadBytesRequest, ScriptingRequestBytes,
    ScriptingResponseStatus, ENGINE_SCRIPTING_SERVICE_ID, SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
};
use serde::{de::DeserializeOwned, Serialize};

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
        let request = AssetDecodeRequest {
            logical_path: self.script_ref.clone(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        };
        let module_bytes = assets.decode_v1(&request).map_err(|error| {
            format!(
                "failed to decode selectorless script module '{}' through engine.assets: {error}",
                self.script_ref
            )
        })?;
        let request = ScriptingModuleLoadBytesRequest {
            module_ref: ScriptModuleRef::new(&self.script_ref),
            module_bytes,
            permissions: Vec::new(),
            metadata: BTreeMap::from([
                ("purpose".to_owned(), self.purpose.clone()),
                ("content_type".to_owned(), "text/x-lua".to_owned()),
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

fn ensure_gateways() -> Result<(), String> {
    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        return Err(format!(
            "asset gateway '{}' is unavailable",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        ));
    }
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
}
