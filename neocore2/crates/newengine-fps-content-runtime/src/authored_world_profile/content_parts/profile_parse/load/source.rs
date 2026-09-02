pub fn load_authored_world_profile() -> Result<AuthoredWorldProfile, Vec<String>> {
    load_profile_from_asset_manager()
}

pub fn load_authored_world_profile_from_resolved_map(
    logical_path: &str,
    map_ref: &str,
    index: &newengine_assets_api::MapIndexV1,
) -> Result<AuthoredWorldProfile, Vec<String>> {
    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        return Err(vec![format!(
            "AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        )]);
    }
    let assets =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    load_profile_asset_with_resolved_map(
        &assets,
        logical_path,
        Some((map_ref.to_owned(), index.clone())),
    )
    .map_err(|error| {
        vec![format!(
            "path='{logical_path}' resolved_map='{map_ref}' err='{error}'"
        )]
    })
}

fn load_profile_from_asset_manager() -> Result<AuthoredWorldProfile, Vec<String>> {
    use newengine_assets::AssetService;

    if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "authored-world: AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        );
        return Err(vec![format!(
            "AssetManager service '{}' unavailable while resolving authored map",
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID
        )]);
    }

    let assets =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let candidates = profile_asset_candidates();
    newengine_ulog_api::ulog::info!(
        "authored-world ymap read: begin gateway='{}' candidates={} mount_policy='profile-owned VFS mounts already established' decode_policy='AssetManager decode_v1 only'",
        newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
        candidates.len(),
    );

    let mut errors = Vec::new();
    for (index, logical_path) in candidates.into_iter().enumerate() {
        newengine_ulog_api::ulog::info!(
            "authored-world ymap read: candidate begin index={} path='{}'",
            index,
            logical_path,
        );
        match load_profile_asset(&assets, &logical_path) {
            Ok(profile) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                newengine_ulog_api::ulog::info!(
                    "authored-world ymap read: candidate selected index={} path='{}' trace={}",
                    index,
                    logical_path,
                    trace,
                );
                newengine_ulog_api::ulog::info!(
                    "authored-world: loaded authored map asset='{}'",
                    logical_path,
                );
                return Ok(profile);
            }
            Err(e) => {
                let trace = assets
                    .resolve_trace_json_v1(&logical_path)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|te| format!("{{\"trace_error\":\"{te}\"}}"));
                let message = format!("path='{logical_path}' err='{e}' trace={trace}");
                newengine_ulog_api::ulog::info!(
                    "authored-world ymap read: candidate rejected index={} {}",
                    index,
                    message
                );
                errors.push(message);
            }
        }
    }

    Err(errors)
}

fn load_profile_asset(
    assets: &newengine_assets::AssetServiceClient,
    logical_path: &str,
) -> Result<AuthoredWorldProfile, String> {
    load_profile_asset_with_resolved_map(assets, logical_path, None)
}

fn load_profile_asset_with_resolved_map(
    assets: &newengine_assets::AssetServiceClient,
    logical_path: &str,
    resolved_map: Option<(String, newengine_assets_api::MapIndexV1)>,
) -> Result<AuthoredWorldProfile, String> {
    let (map_reference, descriptor) = assets
        .require_semantic_asset_reference_v1(
            logical_path,
            newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
            false,
        )
        .map_err(|error| {
            format!(
                "non-canonical authored map rejected path='{logical_path}' policy='authored maps must resolve through the registered engine.assets.maps format': {error}"
            )
        })?;

    newengine_ulog_api::ulog::info!(
        "authored-world map read: canonical accepted path='{}' module='{}' kind='{}'",
        map_reference.logical_path,
        descriptor.module_id,
        descriptor.asset_kind,
    );

    let output_kind = ASSET_LIST_FILE_BODY_OUTPUT;
    let request = AssetDecodeRequest {
        logical_path: map_reference.logical_path.clone(),
        output_kind: output_kind.to_owned(),
        selector: serde_json::Value::Null,
        format_descriptor: None,
    };
    newengine_ulog_api::ulog::info!(
        "authored-world ymap read: decode start path='{}' output='{}' selector=null",
        logical_path,
        output_kind,
    );
    let payload = assets.decode_v1(&request).map_err(|e| {
        format!("asset.decode_v1 failed path='{logical_path}' output='{output_kind}' err='{e}'")
    })?;
    newengine_ulog_api::ulog::info!(
        "authored-world ymap read: decode complete path='{}' output='{}' payload_bytes={}",
        logical_path,
        output_kind,
        payload.len(),
    );
    if !authored_xml::body_is_xml(&payload) {
        return Err(format!(
            "ymap body must be XML path='{logical_path}' output='{output_kind}' payload_bytes={} policy='authored map metadata uses XML presentation inside NEF8; JSON runtime map bodies are forbidden'",
            payload.len()
        ));
    }
    if ymap_schema(&payload, logical_path)?.as_str() == "newengine.map.definition.v2" {
        let value = parse_ymap_xml_payload(&payload, logical_path)?;
        log_ymap_value_summary(logical_path, &value);
        let authored_profile = if value.pointer("/map/profile").is_some() {
            Some(parse_map_definition_payload(value, logical_path)?)
        } else {
            None
        };
        return if let Some((map_ref, index)) = resolved_map {
            load_discrete_map_profile_from_index(logical_path, authored_profile, map_ref, index)
        } else {
            load_discrete_map_profile(logical_path, authored_profile)
        };
    }
    let value = parse_ymap_xml_payload(&payload, logical_path)?;
    log_ymap_value_summary(logical_path, &value);
    newengine_ulog_api::ulog::info!(
        "authored-world: decoded authored .ymap path='{}' output='{}' policy='NEF8/ListFile body from engine.assets; XML map semantics stay outside AssetManager'",
        logical_path,
        output_kind
    );
    parse_map_definition_payload(value, logical_path)
}
