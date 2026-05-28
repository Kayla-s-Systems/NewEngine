#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.scene` gateway runtime service.
//!
//! This crate hosts the current scene IO gateway candidate. It is intentionally
//! separate from product profiles: profiles choose to register it, but do not own
//! scene load/save service transport or gateway metadata.

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_plugin_api::{Blob, MethodName};
use newengine_scene::{SceneAsset, SceneAssetOptions, SCENE_ASSET_SCHEMA_V1, SCENE_ASSET_STATUS_TRANSITIONAL_JSON};
use newengine_scene_io::{method as scene_method, ENGINE_SCENE_SERVICE_ID, SCENE_BACKEND_CAPABILITY_ID};
pub use newengine_engine_runtime::SceneBridge;

use newengine_service_kit::{
    ok_json, payload_json, register_engine_gateway_provider_service, EngineGatewayProviderDecl,
    JsonServiceRouter,
};

pub const SCENE_GATEWAY_OWNER: &str = "newengine-scene-runtime.scene-gateway";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneGatewayAssetMounts {
    pub app_dir_name: &'static str,
    pub app_assets_env: &'static str,
}

impl SceneGatewayAssetMounts {
    #[inline]
    pub const fn new(app_dir_name: &'static str, app_assets_env: &'static str) -> Self {
        Self { app_dir_name, app_assets_env }
    }
}

#[derive(Clone)]
pub struct EngineSceneGatewayService {
    scene: Arc<SceneBridge>,
    asset_mounts: Option<SceneGatewayAssetMounts>,
}


fn normalize_scene_path(path: &str) -> String {
    let mut s = path.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") { s = rest.to_owned(); }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") { s = s.replace("//", "/"); }
    s
}

fn reject_ytyp_scene_path(path: &str) -> Result<(), String> {
    let lower = path.split('@').next().unwrap_or(path).to_ascii_lowercase();
    if lower.ends_with(&format!(".{}", newengine_asset_format_nef8::ytyp::EXTENSION)) {
        return Err(format!(
            "engine.scene load_json_v1 cannot load '{path}' as a scene path: definition dictionary assets are owned by engine.assets.definitions, not engine.scene"
        ));
    }
    Ok(())
}

fn validate_scene_asset_contract(path: &str, asset: &SceneAsset) -> Result<(), String> {
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

fn call_gateway_json(service: &str, method: &str, request: serde_json::Value) -> Result<serde_json::Value, String> {
    let host = newengine_plugin_host::default_host_api();
    let payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    let result = (host.call_service_v1)(
        abi_stable::std_types::RString::from(service),
        MethodName::from(method),
        Blob::from(payload),
    );
    let bytes = result.into_result().map_err(|e| e.to_string())?.into_vec();
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(|e| format!("{service}.{method} returned non-json: {e}"))
}

fn validate_definition_ref_through_gateways(definition_ref: &str) -> Result<(), String> {
    let normalized = normalize_scene_path(definition_ref);
    let reference = newengine_assets_api::require_asset_reference_extension(&normalized, &["ytyp"], true)
        .map_err(|e| format!("scene definition_ref must be .ytyp@entry and resolved outside engine.scene: {e}"))?;

    let graph = call_gateway_json(
        newengine_assets_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
        newengine_assets_api::asset_graph_method::RESOLVE_V1,
        serde_json::json!({ "root_ref": reference.canonical }),
    )?;
    if graph.get("root_ref").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
        return Err(format!("engine.assets.graph returned no root_ref for scene definition_ref='{}'", reference.canonical));
    }

    let validation = call_gateway_json(
        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID,
        newengine_assets_api::definitions_method::VALIDATE_V1,
        serde_json::json!({ "definition_ref": reference.canonical }),
    )?;
    if !validation.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("engine.assets.definitions rejected scene definition_ref='{}': {}", reference.canonical, validation));
    }
    Ok(())
}

fn validate_scene_definition_refs(asset: &SceneAsset) -> Result<(), String> {
    for entity in &asset.entities {
        let Some(definition_ref) = entity.definition_ref.as_deref().map(str::trim).filter(|it| !it.is_empty()) else {
            continue;
        };
        validate_definition_ref_through_gateways(definition_ref)?;
    }
    Ok(())
}

impl EngineSceneGatewayService {
    #[inline]
    pub fn new(scene: Arc<SceneBridge>) -> Self {
        Self { scene, asset_mounts: None }
    }

    #[inline]
    pub fn with_asset_mounts(
        scene: Arc<SceneBridge>,
        asset_mounts: SceneGatewayAssetMounts,
    ) -> Self {
        Self { scene, asset_mounts: Some(asset_mounts) }
    }


    fn authority_json(&self) -> serde_json::Value {
        let snap = self.scene.authority_snapshot();
        serde_json::json!({
            "authority": snap.authority_label(),
            "split": snap.has_split_world_authority(),
            "ecs_owner": snap.ecs.as_ref().map(|r| r.provider_owner_id.clone()),
            "entity_owner": snap.entity.as_ref().map(|r| r.provider_owner_id.clone()),
            "scene_owner": snap.scene.as_ref().map(|r| r.provider_owner_id.clone()),
            "notes": snap.notes.clone(),
        })
    }

    fn formats_json(&self) -> RResult<Blob, RString> {
        ok_json(serde_json::json!({
            "id": ENGINE_SCENE_SERVICE_ID,
            "origin": "engine-runtime",
            "owner": SCENE_GATEWAY_OWNER,
            "version": 1,
            "formats": [
                {
                    "id": "newengine.scene.asset.v1",
                    "schema": SCENE_ASSET_SCHEMA_V1,
                    "status": SCENE_ASSET_STATUS_TRANSITIONAL_JSON,
                    "media_type": "application/json",
                    "load": true,
                    "save": true,
                    "not_ytyp": true,
                    "not_definition_dictionary": true,
                    "allowed_definition_ref_field": "entities[].definition_ref",
                    "definition_resolution": [
                        newengine_assets_api::ENGINE_ASSETS_GRAPH_SERVICE_ID,
                        newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID
                    ]
                }
            ],
            "authority_topology": self.authority_json(),
            "methods": [
                scene_method::FORMATS_JSON,
                scene_method::LOAD_JSON_V1,
                scene_method::SAVE_JSON_V1,
                scene_method::SHUTDOWN_V1
            ]
        }))
    }

    fn load_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let path = req
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(path) = path else {
            return RResult::RErr(RString::from("engine.scene load_json_v1 requires non-empty path"));
        };
        let path = normalize_scene_path(path);
        if let Err(e) = reject_ytyp_scene_path(&path) {
            return RResult::RErr(RString::from(e));
        }

        let replace = req.get("replace").and_then(|v| v.as_bool()).unwrap_or(true);
        if !replace {
            return RResult::RErr(RString::from(
                "engine.scene load_json_v1 currently supports replace=true only",
            ));
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            return RResult::RErr(RString::from(format!(
                "engine.scene cannot load '{path}': asset gateway '{}' is unavailable",
                newengine_assets::consts::ASSET_SERVICE_ID
            )));
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        if let Some(mounts) = self.asset_mounts {
            let asset_roots = newengine_runtime_host::asset_bootstrap::collect_app_asset_roots(
                mounts.app_dir_name,
                mounts.app_assets_env,
            );
            newengine_runtime_host::asset_bootstrap::mount_asset_roots_best_effort(&assets, &asset_roots);
        }

        let bytes = match assets.text_v1(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 asset read failed path='{path}' err='{e}'"
                )));
            }
        };

        let asset = match serde_json::from_slice::<SceneAsset>(&bytes) {
            Ok(asset) => asset,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene json parse failed path='{path}' err='{e}'"
                )));
            }
        };
        if let Err(e) = validate_scene_asset_contract(&path, &asset) {
            return RResult::RErr(RString::from(e));
        }
        if let Err(e) = validate_scene_definition_refs(&asset) {
            return RResult::RErr(RString::from(e));
        }

        {
            let authority = self.scene.authority_snapshot();
            if authority.has_split_world_authority() {
                log::warn!(
                    "engine.scene load_json_v1 running while world authority is split authority='{}' notes='{}'",
                    authority.authority_label(),
                    authority.notes.join(";")
                );
            }
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            if let Err(e) = scene.load_asset(&asset) {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene apply failed path='{path}' err='{e}'"
                )));
            }
        }

        ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "replace": true,
            "entities": asset.entities.len(),
            "schema": asset.schema,
            "status": SCENE_ASSET_STATUS_TRANSITIONAL_JSON,
            "not_ytyp": true,
            "not_definition_dictionary": true,
            "definition_refs": asset.entities.iter().filter_map(|entity| entity.definition_ref.as_deref()).count(),
            "version": asset.version,
            "authority_topology": self.authority_json()
        }))
    }

    fn save_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match payload_json(&payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pretty = req.get("pretty").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_empty_entities = req
            .get("options")
            .and_then(|v| v.get("include_empty_entities"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let asset = {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            scene.to_asset(SceneAssetOptions { include_empty_entities })
        };

        let payload = match serde_json::to_value(&asset) {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        let payload_text = match if pretty {
            serde_json::to_string_pretty(&asset)
        } else {
            serde_json::to_string(&asset)
        } {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "stored": false,
            "storage": "caller-owned",
            "schema": SCENE_ASSET_SCHEMA_V1,
            "status": SCENE_ASSET_STATUS_TRANSITIONAL_JSON,
            "not_ytyp": true,
            "not_definition_dictionary": true,
            "authority_topology": self.authority_json(),
            "pretty": pretty,
            "payload": payload,
            "payload_text": payload_text
        }))
    }
}

pub fn scene_gateway_service(
    scene: Arc<SceneBridge>,
    asset_mounts: Option<SceneGatewayAssetMounts>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = match asset_mounts {
        Some(mounts) => EngineSceneGatewayService::with_asset_mounts(scene, mounts),
        None => EngineSceneGatewayService::new(scene),
    };
    let description = serde_json::json!({
        "id": ENGINE_SCENE_SERVICE_ID,
        "version": 1,
        "contract": "newengine.scene gateway >= 0.1.x",
        "origin": "engine-runtime",
        "owner": SCENE_GATEWAY_OWNER,
        "capability": SCENE_BACKEND_CAPABILITY_ID,
        "methods": [
            scene_method::FORMATS_JSON,
            scene_method::LOAD_JSON_V1,
            scene_method::SAVE_JSON_V1,
            scene_method::SHUTDOWN_V1
        ]
    });

    let formats_service = service.clone();
    let load_service = service.clone();
    let save_service = service;

    JsonServiceRouter::new(ENGINE_SCENE_SERVICE_ID)
        .describe_json(&description)
        .blob(scene_method::FORMATS_JSON, move |_unit, _payload| formats_service.formats_json())
        .blob(scene_method::LOAD_JSON_V1, move |_unit, payload| load_service.load_json_v1(payload))
        .blob(scene_method::SAVE_JSON_V1, move |_unit, payload| save_service.save_json_v1(payload))
        .blob(scene_method::SHUTDOWN_V1, move |_unit, _payload| RResult::ROk(Blob::from(Vec::new())))
        .into_service_v1()
}

pub fn register_scene_gateway_best_effort(
    scene: Arc<SceneBridge>,
    asset_mounts: Option<SceneGatewayAssetMounts>,
) {
    if newengine_plugin_host::has_service(ENGINE_SCENE_SERVICE_ID) {
        log::debug!("engine.scene gateway registration skipped; service already available");
        return;
    }

    let service = scene_gateway_service(scene, asset_mounts);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_SCENE_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Scene,
        provider_service: ENGINE_SCENE_SERVICE_ID,
        provider_route: "engine.scene.foundation",
        capability: SCENE_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: SCENE_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "engine.scene gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_SCENE_SERVICE_ID,
            SCENE_BACKEND_CAPABILITY_ID,
            SCENE_GATEWAY_OWNER
        ),
        Err(e) => log::error!(
            "engine.scene gateway registration failed id='{}' err='{}'",
            ENGINE_SCENE_SERVICE_ID,
            e
        ),
    }
}
