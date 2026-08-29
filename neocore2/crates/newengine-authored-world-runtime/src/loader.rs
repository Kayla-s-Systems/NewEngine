use std::collections::BTreeMap;

use newengine_assets_api::{
    maps_method, MapCellRequestV1, MapIndexV1, MapRefRequestV1, MapResolvedCellV1,
};
use newengine_definitions_runtime::{DefinitionEntryV1, DefinitionRefRequest};

#[inline]
pub(crate) fn placement_requires_definition(
    placement: &newengine_assets_api::MapPlacementV1,
) -> Result<bool, String> {
    match placement.apply_mode.trim().to_ascii_lowercase().as_str() {
        "metadata_only" => Ok(false),
        "instantiate" | "static" | "static_mesh" | "visual" | "dynamic" | "dynamic_physics" => Ok(true),
        other => Err(format!(
            "authored-world placement '{}' has unsupported apply_mode '{}'; expected metadata_only|instantiate|static|static_mesh|visual|dynamic|dynamic_physics",
            placement.id, other
        )),
    }
}

pub(crate) struct LoadedAuthoredMap {
    pub map_ref: String,
    pub index: MapIndexV1,
    pub cells: Vec<MapResolvedCellV1>,
    pub definitions: BTreeMap<String, DefinitionEntryV1>,
}

fn call_json<Request: serde::Serialize, Response: serde::de::DeserializeOwned>(
    service: &str,
    method: &str,
    request: &Request,
) -> Result<Response, String> {
    let payload = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    let bytes = newengine_core::call_service_v1_optional(service, method, &payload)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("required authored-world gateway '{service}' is unavailable"))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!("authored-world gateway response decode failed service='{service}' method='{method}' err='{error}'")
    })
}

pub(crate) fn load_authored_map(logical_path: &str) -> Result<LoadedAuthoredMap, String> {
    let map_ref =
        newengine_assets_api::map_entry_ref(logical_path, newengine_assets_api::MAP_INDEX_ENTRY);
    let index: MapIndexV1 = call_json(
        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        maps_method::INDEX_V1,
        &MapRefRequestV1 {
            map_ref: map_ref.clone(),
        },
    )?;
    index.validate().map_err(|errors| {
        format!(
            "authored-world map index invalid map='{map_ref}': {}",
            errors.join(" | ")
        )
    })?;

    let mut cells = Vec::with_capacity(index.cells.len());
    let mut definitions = BTreeMap::new();
    for cell_ref in &index.cells {
        let resolved: MapResolvedCellV1 = call_json(
            newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
            maps_method::CELL_V1,
            &MapCellRequestV1 {
                map_ref: map_ref.clone(),
                coord: cell_ref.coord,
            },
        )?;
        resolved.cell.validate().map_err(|errors| {
            format!(
                "authored-world map cell invalid map='{map_ref}' cell={},{}: {}",
                cell_ref.coord.x,
                cell_ref.coord.z,
                errors.join(" | ")
            )
        })?;
        for placement in resolved
            .cell
            .placements
            .iter()
            .filter(|placement| placement.enabled)
        {
            if !placement_requires_definition(placement)?
                || definitions.contains_key(&placement.definition_ref)
            {
                continue;
            }
            let entry: DefinitionEntryV1 = call_json(
                newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
                newengine_assets_api::definitions_method::ENTRY_JSON_V1,
                &DefinitionRefRequest {
                    definition_ref: placement.definition_ref.clone(),
                    ..Default::default()
                },
            )?;
            definitions.insert(placement.definition_ref.clone(), entry);
        }
        cells.push(resolved);
    }

    Ok(LoadedAuthoredMap {
        map_ref,
        index,
        cells,
        definitions,
    })
}
