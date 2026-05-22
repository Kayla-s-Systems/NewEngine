#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    definitions_method, AssetDecodeRequest, ENGINE_DEFINITIONS_SERVICE_ID,
    DEFINITIONS_BACKEND_CAPABILITY_ID, DEFINITIONS_RUNTIME_CONTRACT,
    DEFINITIONS_SERVICE_ID, DEFINITIONS_SERVICE_METHODS,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

use crate::AssetServiceClient;

#[derive(Clone)]
struct SemanticAssetGatewayState {
    client: AssetServiceClient,
}

#[derive(Clone, Debug, Serialize)]
struct SemanticGatewayInfo {
    service_id: &'static str,
    gateway: &'static str,
    provider: &'static str,
    contract: &'static str,
    byte_owner: &'static str,
    methods: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
struct DefinitionRefRequest {
    definition_ref: String,
    source: String,
    entry: Option<String>,
}

impl Default for DefinitionRefRequest {
    fn default() -> Self {
        Self { definition_ref: String::new(), source: String::new(), entry: None }
    }
}

#[derive(Clone, Debug, Serialize)]
struct StableDiagnostic {
    ok: bool,
    code: &'static str,
    message: String,
    gateway: &'static str,
    byte_owner: &'static str,
}

fn definitions_info() -> SemanticGatewayInfo {
    SemanticGatewayInfo {
        service_id: DEFINITIONS_SERVICE_ID,
        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
        provider: "EngineOwnedDefinitionsProvider",
        contract: DEFINITIONS_RUNTIME_CONTRACT,
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
        methods: DEFINITIONS_SERVICE_METHODS.to_vec(),
    }
}

fn decode_definition_request(request: &DefinitionRefRequest) -> Result<(String, String), String> {
    let raw = if !request.definition_ref.trim().is_empty() {
        request.definition_ref.trim().to_owned()
    } else if !request.source.trim().is_empty() {
        match request.entry.as_deref().map(str::trim).filter(|it| !it.is_empty()) {
            Some(entry) => format!("{}@{}", request.source.trim(), entry),
            None => request.source.trim().to_owned(),
        }
    } else {
        return Err("definitions API requires definition_ref='.ytyp@entry' or source + entry".to_owned());
    };
    let reference = newengine_assets_api::require_asset_reference_extension(&raw, &["ytyp"], true)
        .map_err(|e| e.to_string())?;
    let entry = reference.entry.clone().unwrap_or_default();
    Ok((reference.logical_path, entry))
}

fn decode_definition_source(request: &DefinitionRefRequest) -> Result<String, String> {
    let raw = if !request.definition_ref.trim().is_empty() {
        request.definition_ref.trim().to_owned()
    } else if !request.source.trim().is_empty() {
        request.source.trim().to_owned()
    } else {
        return Err("definitions API requires definition_ref or source ending in .ytyp".to_owned());
    };
    let reference = newengine_assets_api::require_asset_reference_extension(&raw, &["ytyp"], false)
        .map_err(|e| e.to_string())?;
    Ok(reference.logical_path)
}

fn decode_definition_manifest_json(
    state: &SemanticAssetGatewayState,
    request: &DefinitionRefRequest,
) -> Result<(String, serde_json::Value), String> {
    let path = decode_definition_source(request)?;
    let bytes = state.client.decode_v1(&AssetDecodeRequest {
        logical_path: path.clone(),
        output_kind: definitions_method::MANIFEST_JSON_V1.to_owned(),
        selector: serde_json::Value::Null,
    }).map_err(|e| format!("engine.definitions: manifest decode failed path='{path}' err='{e}'"))?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| format!("engine.definitions: codec returned non-json manifest path='{path}' err='{e}'"))?;
    Ok((path, value))
}

fn decode_definition_entry_json(
    state: &SemanticAssetGatewayState,
    request: &DefinitionRefRequest,
) -> Result<(String, String, serde_json::Value), String> {
    let (path, entry) = decode_definition_request(request)?;
    let bytes = state.client.decode_v1(&AssetDecodeRequest {
        logical_path: path.clone(),
        output_kind: definitions_method::ENTRY_JSON_V1.to_owned(),
        selector: serde_json::json!({ "entry": entry }),
    }).map_err(|e| format!("engine.definitions: decode failed path='{path}' entry='{entry}' err='{e}'"))?;
    let value = serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| format!("engine.definitions: codec returned non-json path='{path}' entry='{entry}' err='{e}'"))?;
    Ok((path, entry, value))
}

fn collect_definition_refs(value: &serde_json::Value) -> Vec<String> {
    fn walk(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::String(text) => {
                let mut normalized = text.trim().replace('\\', "/");
                while normalized.contains("//") { normalized = normalized.replace("//", "/"); }
                let lower = normalized.to_ascii_lowercase();
                if [".ydd@", ".nemat@", ".ytd@", ".ybn@", ".ycol@", ".nebrain@", ".nepat@", ".nemem@"].iter().any(|needle| lower.contains(needle)) {
                    out.push(normalized.trim_start_matches('/').to_owned());
                }
            }
            serde_json::Value::Array(items) => {
                for item in items { walk(item, out); }
            }
            serde_json::Value::Object(map) => {
                for value in map.values() { walk(value, out); }
            }
            _ => {}
        }
    }
    let mut refs = Vec::new();
    walk(value, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn definitions_manifest_direct(state: &mut SemanticAssetGatewayState, payload: newengine_plugin_api::Blob) -> RResult<newengine_plugin_api::Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.definitions.manifest.v1",
            "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
            "byte_owner": newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
            "provider": "EngineOwnedDefinitionsProvider"
        }));
    }
    let request = match serde_json::from_slice::<DefinitionRefRequest>(&payload) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(format!("engine.definitions: invalid manifest request: {e}"))),
    };
    match decode_definition_manifest_json(state, &request) {
        Ok((_path, value)) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn definitions_invoke(state: &mut SemanticAssetGatewayState, payload: newengine_plugin_api::Blob) -> RResult<newengine_plugin_api::Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value.get("method").and_then(|v| v.as_str()).unwrap_or(definitions_method::VALIDATE_V1);
    let request = serde_json::from_value::<DefinitionRefRequest>(value.get("request").cloned().unwrap_or_default())
        .unwrap_or_default();
    match method {
        definitions_method::VALIDATE_V1 => match decode_definition_request(&request) {
            Ok((path, entry)) => ok_json(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "byte_owner": newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
                "logical_path": path,
                "entry": entry
            })),
            Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.invalid_ref", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID }),
        },
        definitions_method::ENTRY_JSON_V1 => match decode_definition_entry_json(state, &request) {
            Ok((_path, _entry, value)) => ok_json(value),
            Err(e) => RResult::RErr(RString::from(e)),
        },
        definitions_method::RESOLVE_REFS_V1 => match decode_definition_entry_json(state, &request) {
            Ok((path, entry, value)) => ok_json(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "definition_ref": format!("{path}@{entry}"),
                "refs": collect_definition_refs(&value),
                "resolver": newengine_assets_api::ENGINE_ASSET_GRAPH_SERVICE_ID
            })),
            Err(e) => RResult::RErr(RString::from(e)),
        },
        definitions_method::DESCRIBE_SIDE_EFFECTS_V1 => match decode_definition_request(&request) {
            Ok((path, entry)) => ok_json(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "byte_owner": newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
                "definition_ref": format!("{path}@{entry}"),
                "side_effect_policy": "descriptive-only; consumer decides; no ECS/render/physics mutation",
                "domains": ["engine.model", "engine.materials", "engine.textures", "engine.physics", "engine.ai", "engine.streaming"]
            })),
            Err(e) => ok_json(StableDiagnostic { ok: false, code: "definitions.invalid_ref", message: e, gateway: ENGINE_DEFINITIONS_SERVICE_ID, byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID }),
        },
        definitions_method::MANIFEST_JSON_V1 => {
            if request.definition_ref.trim().is_empty() && request.source.trim().is_empty() {
                ok_json(serde_json::json!({
                    "schema": "newengine.definitions.manifest.v1",
                    "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                    "byte_owner": newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
                    "provider": "EngineOwnedDefinitionsProvider",
                    "entry_method": definitions_method::ENTRY_JSON_V1,
                    "resolve_refs_method": definitions_method::RESOLVE_REFS_V1
                }))
            } else {
                match decode_definition_manifest_json(state, &request) {
                    Ok((_path, value)) => ok_json(value),
                    Err(e) => RResult::RErr(RString::from(e)),
                }
            }
        },
        other => RResult::RErr(RString::from(format!("engine.definitions: unknown invoke method '{other}'"))),
    }
}

fn definitions_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        DEFINITIONS_SERVICE_ID,
        "newengine-assets.engine-owned-definitions-provider",
        DEFINITIONS_BACKEND_CAPABILITY_ID,
        DEFINITIONS_SERVICE_METHODS.iter().copied(),
    )
    .protocol(DEFINITIONS_RUNTIME_CONTRACT)
    .features(["definition-entry", "archetype-metadata", "side-effect-description"])
    .gateway("engine-owned engine.definitions semantic facade over engine.assets")
    .notes("Semantic .ytyp gateway. It reads definitions only; it does not mutate ECS/render/physics.");

    JsonServiceRouter::with_state(DEFINITIONS_SERVICE_ID, SemanticAssetGatewayState { client })
        .describe_json(&description)
        .info(definitions_info)
        .blob(definitions_method::MANIFEST_JSON_V1, definitions_manifest_direct)
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::VALIDATE_V1, |_state, request| {
            let (path, entry) = decode_definition_request(&request)?;
            Ok(serde_json::json!({ "ok": true, "logical_path": path, "entry": entry, "gateway": ENGINE_DEFINITIONS_SERVICE_ID }))
        })
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::ENTRY_JSON_V1, |state, request| {
            decode_definition_entry_json(state, &request).map(|(_, _, value)| value)
        })
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::RESOLVE_REFS_V1, |state, request| {
            let (path, entry, value) = decode_definition_entry_json(state, &request)?;
            Ok(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "definition_ref": format!("{path}@{entry}"),
                "refs": collect_definition_refs(&value),
                "resolver": newengine_assets_api::ENGINE_ASSET_GRAPH_SERVICE_ID
            }))
        })
        .post_json_result::<DefinitionRefRequest, serde_json::Value, _>(definitions_method::DESCRIBE_SIDE_EFFECTS_V1, |_state, request| {
            let (path, entry) = decode_definition_request(&request)?;
            Ok(serde_json::json!({
                "ok": true,
                "gateway": ENGINE_DEFINITIONS_SERVICE_ID,
                "definition_ref": format!("{path}@{entry}"),
                "policy": "facts-only; no direct ECS/render/physics mutation"
            }))
        })
        .blob(definitions_method::INVOKE_JSON, definitions_invoke)
        .blob(definitions_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_definitions_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_DEFINITIONS_SERVICE_ID,
        service_kind: EngineServiceKind::Definitions,
        provider_service: DEFINITIONS_SERVICE_ID,
        capability: DEFINITIONS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: "newengine-assets.engine-owned-definitions-provider",
        service: definitions_service(client),
    })
}
