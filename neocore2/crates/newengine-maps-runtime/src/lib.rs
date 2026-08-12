#![forbid(unsafe_op_in_unsafe_fn)]

//! Semantic runtime provider for `engine.assets.maps`.
//!
//! The provider owns `.ymap` map composition semantics and returns DTOs only.
//! It never receives or mutates ECS/world internals; `engine.scene` / `engine.world`
//! apply stages own instantiation.

mod parsing;

use std::collections::BTreeMap;

use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    maps_method, require_asset_reference_extension, AssetDecodeRequest, MapCellRequestV1,
    MapDependenciesV1, MapIndexV1, MapRefRequestV1, MapResolvedCellV1, MapValidationV1,
    ASSET_LIST_FILE_BODY_OUTPUT, ENGINE_ASSETS_MAPS_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
    LIST_FILE_MAGIC_NEF8, MAPS_BACKEND_CAPABILITY_ID, MAPS_RUNTIME_CONTRACT, MAPS_SERVICE_ID,
    MAPS_SERVICE_METHODS, MAP_INDEX_ENTRY,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use serde::Serialize;

use parsing::{parse_discrete_map_xml, ParsedMapV1};

pub const MAPS_GATEWAY_OWNER: &str = "newengine-maps-runtime.engine-runtime-provider";

#[derive(Clone)]
pub struct MapsRuntimeState {
    client: AssetServiceClient,
    parsed_cache: BTreeMap<String, ParsedMapV1>,
}

impl MapsRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,
            parsed_cache: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MapsServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub capability: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub methods: &'static [&'static str],
    pub map_schema: &'static str,
    pub cell_schema: &'static str,
    pub ownership_policy: &'static str,
}

pub fn maps_service_info() -> MapsServiceInfo {
    MapsServiceInfo {
        id: MAPS_SERVICE_ID,
        gateway: ENGINE_ASSETS_MAPS_SERVICE_ID,
        provider: "NorthStarDiscreteMapsRuntimeProvider",
        contract: MAPS_RUNTIME_CONTRACT,
        capability: MAPS_BACKEND_CAPABILITY_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_MAPS_SERVICE_ID,
        methods: MAPS_SERVICE_METHODS,
        map_schema: newengine_assets_api::MAP_INDEX_SCHEMA_V1,
        cell_schema: newengine_assets_api::MAP_CELL_SCHEMA_V1,
        ownership_policy: ".ymap owns discrete map topology and .ytyp placements only; engine.assets owns VFS/NEF8 bytes; engine.scene/engine.world own apply-stage mutation",
    }
}

fn canonical_map_source(request: &MapRefRequestV1) -> Result<(String, String), String> {
    let reference = require_asset_reference_extension(&request.map_ref, &["ymap"], false)?;
    if let Some(entry) = reference.entry.as_deref() {
        if !entry.eq_ignore_ascii_case(MAP_INDEX_ENTRY) {
            return Err(format!(
                "engine.assets.maps index requires .ymap@map or bare .ymap, got '{}'",
                reference.canonical
            ));
        }
    }
    let canonical = format!("{}@{}", reference.logical_path, MAP_INDEX_ENTRY);
    Ok((reference.logical_path, canonical))
}

fn load_map_body(state: &MapsRuntimeState, source: &str) -> Result<(Vec<u8>, Vec<String>), String> {
    match state.client.raw_bytes_v1(source) {
        Ok(body) if body.get(0..4) != Some(&LIST_FILE_MAGIC_NEF8[..]) => Ok((
            body,
            vec![".ymap loose authoring body read through engine.assets raw_bytes_v1".to_owned()],
        )),
        Ok(_nef8) => decode_map_body(state, source).map(|body| {
            (
                body,
                vec![".ymap NEF8 ListFile body decoded through engine.assets".to_owned()],
            )
        }),
        Err(read_error) => decode_map_body(state, source)
            .map(|body| {
                (
                    body,
                    vec![".ymap body decoded through engine.assets after raw_bytes_v1 miss".to_owned()],
                )
            })
            .map_err(|decode_error| {
                format!(
                    "engine.assets.maps: .ymap unavailable source='{source}' read_err='{read_error}' decode_err='{decode_error}'"
                )
            }),
    }
}

fn decode_map_body(state: &MapsRuntimeState, source: &str) -> Result<Vec<u8>, String> {
    state
        .client
        .decode_v1(&AssetDecodeRequest {
            logical_path: source.to_owned(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|error| {
            format!(
                "engine.assets.maps: asset.decode_v1 failed source='{source}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{error}'"
            )
        })
}

fn load_parsed_map(
    state: &mut MapsRuntimeState,
    request: &MapRefRequestV1,
) -> Result<(String, ParsedMapV1), String> {
    let (source, canonical) = canonical_map_source(request)?;
    if let Some(parsed) = state.parsed_cache.get(&source) {
        return Ok((canonical, parsed.clone()));
    }
    let (body, mut transport_warnings) = load_map_body(state, &source)?;
    if !newengine_authored_xml::body_is_xml(&body) {
        return Err(format!(
            "engine.assets.maps: YMAP v2 semantic body must currently be XML source='{source}' bytes={} schema='newengine.map.definition.v2'",
            body.len()
        ));
    }
    let mut parsed = parse_discrete_map_xml(&source, &body)?;
    transport_warnings.append(&mut parsed.warnings);
    parsed.warnings = transport_warnings;
    state.parsed_cache.insert(source, parsed.clone());
    Ok((canonical, parsed))
}

fn map_index(state: &mut MapsRuntimeState, request: MapRefRequestV1) -> Result<MapIndexV1, String> {
    load_parsed_map(state, &request).map(|(_, parsed)| parsed.index)
}

fn map_cell(
    state: &mut MapsRuntimeState,
    request: MapCellRequestV1,
) -> Result<MapResolvedCellV1, String> {
    let map_request = MapRefRequestV1 {
        map_ref: request.map_ref,
    };
    let (canonical_map_ref, parsed) = load_parsed_map(state, &map_request)?;
    let cell = parsed.cells.get(&request.coord).cloned().ok_or_else(|| {
        format!(
            "engine.assets.maps: cell not declared map='{}' coord={},{}",
            canonical_map_ref, request.coord.x, request.coord.z
        )
    })?;
    let cell_ref = format!(
        "{}@{}",
        canonical_map_ref.split('@').next().unwrap_or_default(),
        request.coord.canonical_entry()
    );
    Ok(MapResolvedCellV1 {
        map_ref: canonical_map_ref,
        cell_ref,
        index: parsed.index,
        cell,
    })
}

fn map_dependencies(
    state: &mut MapsRuntimeState,
    request: MapRefRequestV1,
) -> Result<MapDependenciesV1, String> {
    let (canonical_map_ref, parsed) = load_parsed_map(state, &request)?;
    Ok(MapDependenciesV1 {
        map_ref: canonical_map_ref,
        dependencies: parsed.dependencies,
    })
}

fn map_validation(state: &mut MapsRuntimeState, request: MapRefRequestV1) -> MapValidationV1 {
    let requested_ref = request.map_ref.clone();
    match load_parsed_map(state, &request) {
        Ok((canonical_map_ref, parsed)) => MapValidationV1 {
            ok: true,
            map_ref: canonical_map_ref,
            cell_count: u32::try_from(parsed.cells.len()).unwrap_or(u32::MAX),
            placement_count: parsed
                .cells
                .values()
                .map(|cell| u32::try_from(cell.placements.len()).unwrap_or(u32::MAX))
                .fold(0_u32, u32::saturating_add),
            errors: Vec::new(),
            warnings: parsed.warnings,
        },
        Err(error) => MapValidationV1 {
            ok: false,
            map_ref: requested_ref,
            errors: vec![error],
            ..Default::default()
        },
    }
}

fn invoke_json(state: &mut MapsRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let method = value
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(maps_method::VALIDATE_V1);
    let request_value = value.get("request").cloned().unwrap_or_default();
    match method {
        maps_method::INDEX_V1 => {
            let request =
                serde_json::from_value::<MapRefRequestV1>(request_value).unwrap_or_default();
            match map_index(state, request) {
                Ok(index) => ok_json(index),
                Err(error) => RResult::RErr(RString::from(error)),
            }
        }
        maps_method::CELL_V1 => {
            let request =
                serde_json::from_value::<MapCellRequestV1>(request_value).unwrap_or_default();
            match map_cell(state, request) {
                Ok(cell) => ok_json(cell),
                Err(error) => RResult::RErr(RString::from(error)),
            }
        }
        maps_method::VALIDATE_V1 => {
            let request =
                serde_json::from_value::<MapRefRequestV1>(request_value).unwrap_or_default();
            ok_json(map_validation(state, request))
        }
        maps_method::DEPENDENCIES_V1 => {
            let request =
                serde_json::from_value::<MapRefRequestV1>(request_value).unwrap_or_default();
            match map_dependencies(state, request) {
                Ok(dependencies) => ok_json(dependencies),
                Err(error) => RResult::RErr(RString::from(error)),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.maps: unknown invoke method '{other}'"
        ))),
    }
}

pub fn maps_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        MAPS_SERVICE_ID,
        MAPS_GATEWAY_OWNER,
        MAPS_BACKEND_CAPABILITY_ID,
        MAPS_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_MAPS_SERVICE_ID)
    .protocol(MAPS_RUNTIME_CONTRACT)
    .features([
        "discrete-map-index-v1",
        "independent-cell-addressing",
        "ytyp-only-placements",
        "map-layer-composition",
        "deterministic-validation",
    ])
    .notes("Semantic YMAP v2 provider. The map is an index plus independently addressable cells; world mutation remains in scene/world apply stages.");

    JsonServiceRouter::with_state(MAPS_SERVICE_ID, MapsRuntimeState::new(client))
        .describe_json(&description)
        .info(maps_service_info)
        .post_json_result::<MapRefRequestV1, MapIndexV1, _>(maps_method::INDEX_V1, map_index)
        .post_json_result::<MapCellRequestV1, MapResolvedCellV1, _>(maps_method::CELL_V1, map_cell)
        .post_json_result::<MapRefRequestV1, MapValidationV1, _>(
            maps_method::VALIDATE_V1,
            |state, request| Ok(map_validation(state, request)),
        )
        .post_json_result::<MapRefRequestV1, MapDependenciesV1, _>(
            maps_method::DEPENDENCIES_V1,
            map_dependencies,
        )
        .blob(maps_method::INVOKE_JSON, invoke_json)
        .blob(maps_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_maps_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_MAPS_SERVICE_ID,
        service_kind: EngineServiceKind::AssetMaps,
        provider_service: MAPS_SERVICE_ID,
        provider_route: "engine.assets.starvault.maps",
        capability: MAPS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: MAPS_GATEWAY_OWNER,
        service: maps_gateway_service(client),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_contract_is_map_specific() {
        let info = maps_service_info();
        assert_eq!(info.gateway, "engine.assets.maps");
        assert_eq!(info.capability, "assets.maps.backend");
        assert!(info.methods.contains(&"assets.maps.cell_v1"));
        assert!(info.ownership_policy.contains("apply-stage mutation"));
    }
}
