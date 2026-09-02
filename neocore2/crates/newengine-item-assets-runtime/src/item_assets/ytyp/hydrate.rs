pub fn hydrate_item_package_from_ytyp(package: &mut AuthoredItemPackage) -> Result<usize, String> {
    let mut hydrated = 0usize;
    for authored in &mut package.items {
        let definition_ref = authored.definition_ref.trim().replace('\\', "/");
        if definition_ref.is_empty() {
            continue;
        }
        let payload = serde_json::to_vec(&serde_json::json!({
            "definition_ref": definition_ref,
        }))
        .map_err(|error| format!("weapon YTYP request encode failed: {error}"))?;
        let bytes = newengine_core::call_service_v1_optional(
            newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
            newengine_assets_api::definitions_method::ENTRY_JSON_V1,
            &payload,
        )
        .map_err(|error| {
            format!(
                "weapon YTYP lookup failed item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?
        .ok_or_else(|| {
            format!(
                "weapon YTYP definitions service unavailable item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        let entry: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "weapon YTYP entry JSON invalid item='{}' ref='{}': {error}",
                authored.id, definition_ref
            )
        })?;
        let namespace = weapon_namespace(&entry).ok_or_else(|| {
            format!(
                "weapon YTYP has no newengine.weapon metadata item='{}' ref='{}'",
                authored.id, definition_ref
            )
        })?;
        apply_weapon_ytyp_namespace(authored, namespace)?;
        hydrated += 1;
    }
    Ok(hydrated)
}
