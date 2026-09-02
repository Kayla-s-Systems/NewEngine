use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, MethodName};
use newengine_scene::{SceneAsset, SCENE_ASSET_SCHEMA_V1, SCENE_ASSET_STATUS_TRANSITIONAL_JSON};

pub(crate) fn normalize_scene_path(path: &str) -> String {
    let mut normalized = path.trim().replace('\\', "/");
    while let Some(rest) = normalized.strip_prefix("./") {
        normalized = rest.to_owned();
    }
    normalized = normalized.trim_start_matches('/').to_owned();
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized
}

pub(crate) fn reject_ytyp_scene_path(path: &str) -> Result<(), String> {
    let client =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let probe = client.file_type_probe_v1(path).map_err(|error| {
        format!("engine.scene cannot validate asset ownership for scene path='{path}': {error}")
    })?;
    if probe.descriptor.as_ref().is_some_and(|descriptor| {
        descriptor.semantic_gateway == newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID
    }) {
        return Err(format!(
            "engine.scene load_json_v1 cannot load '{path}' as a scene path: registered definition assets are owned by engine.assets.definitions, not engine.scene"
        ));
    }
    Ok(())
}

pub(crate) fn validate_scene_asset_contract(path: &str, asset: &SceneAsset) -> Result<(), String> {
    if asset.schema != SCENE_ASSET_SCHEMA_V1 {
        return Err(format!(
            "engine.scene load_json_v1 expected schema='{}' status='{}' not_ytyp=true not_definition_dictionary=true path='{}' got schema='{}'",
            SCENE_ASSET_SCHEMA_V1,
            SCENE_ASSET_STATUS_TRANSITIONAL_JSON,
            path,
            asset.schema
        ));
    }
    Ok(())
}

fn call_gateway_json(
    service: &str,
    method: &str,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let host = newengine_plugin_host::default_host_api();
    let payload = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    let result = (host.call_service_v1)(
        RString::from(service),
        MethodName::from(method),
        Blob::from(payload),
    );
    let bytes = result
        .into_result()
        .map_err(|error| error.to_string())?
        .into_vec();
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|error| format!("{service}.{method} returned non-json: {error}"))
}

pub(crate) fn validate_definition_ref_through_gateways(definition_ref: &str) -> Result<(), String> {
    let normalized = normalize_scene_path(definition_ref);
    let client =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let (reference, _descriptor) = client
        .require_semantic_asset_reference_v1(
            &normalized,
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            true,
        )
        .map_err(|error| {
            format!(
                "scene definition_ref must resolve through engine.assets.definitions and include @entry: {error}"
            )
        })?;

    let graph = call_gateway_json(
        newengine_assets_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_assets_api::asset_graph_method::RESOLVE_V1,
        serde_json::json!({ "root_ref": reference.canonical }),
    )?;
    if graph
        .get("root_ref")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .is_empty()
    {
        return Err(format!(
            "engine.assets.graph returned no root_ref for scene definition_ref='{}'",
            reference.canonical
        ));
    }

    let validation = call_gateway_json(
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets_api::definitions_method::VALIDATE_V1,
        serde_json::json!({ "definition_ref": reference.canonical }),
    )?;
    if !validation
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err(format!(
            "engine.assets.definitions rejected scene definition_ref='{}': {}",
            reference.canonical, validation
        ));
    }
    Ok(())
}

pub(crate) fn validate_scene_definition_refs(asset: &SceneAsset) -> Result<(), String> {
    for entity in &asset.entities {
        let Some(definition_ref) = entity
            .definition_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        validate_definition_ref_through_gateways(definition_ref)?;
    }
    Ok(())
}
