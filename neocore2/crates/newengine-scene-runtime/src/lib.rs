#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned `engine.scene` gateway runtime service.
//!
//! This crate hosts the current scene IO gateway candidate. It is intentionally
//! separate from product profiles: profiles choose to register it, but do not own
//! scene load/save service transport or gateway metadata.

use std::sync::Arc;

use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_plugin_api::Blob;
use newengine_scene::{SceneAsset, SceneAssetOptions};
use newengine_scene_io::{method as scene_method, ENGINE_SCENE_SERVICE_ID, SCENE_BACKEND_CAPABILITY_ID};
pub use newengine_engine_runtime::SceneBridge;

use newengine_service_kit::{
    ok_json, payload_json, register_engine_owned_gateway_service, EngineOwnedGatewayDecl,
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
            "origin": "engine-owned",
            "owner": SCENE_GATEWAY_OWNER,
            "version": 1,
            "formats": [
                {
                    "id": "newengine.scene.asset.v1",
                    "schema": "kalitech.scene.asset.v1",
                    "media_type": "application/json",
                    "load": true,
                    "save": true
                }
            ],
            "authority_topology": self.authority_json(),
            "methods": [
                scene_method::FORMATS_JSON,
                scene_method::LOAD_JSON_V1,
                scene_method::SAVE_JSON_V1
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

        let bytes = match assets.text_v1(path) {
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
        "origin": "engine-owned",
        "owner": SCENE_GATEWAY_OWNER,
        "capability": SCENE_BACKEND_CAPABILITY_ID,
        "methods": [
            scene_method::FORMATS_JSON,
            scene_method::LOAD_JSON_V1,
            scene_method::SAVE_JSON_V1
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
    match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
        gateway: ENGINE_SCENE_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Scene,
        provider_service: ENGINE_SCENE_SERVICE_ID,
        capability: SCENE_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: SCENE_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => log::info!(
            "engine.scene gateway registered source=engine-owned service='{}' capability='{}' owner='{}'",
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
