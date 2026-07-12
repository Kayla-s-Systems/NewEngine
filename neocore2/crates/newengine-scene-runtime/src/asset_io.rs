use abi_stable::std_types::{RResult, RString};
use newengine_assets::AssetServiceClient;
use newengine_plugin_api::Blob;
use newengine_scene::{SceneAsset, SCENE_ASSET_SCHEMA_V1, SCENE_ASSET_STATUS_TRANSITIONAL_JSON};

use crate::state::EngineSceneGatewayService;
use crate::transport::{json_result, parse_payload};
use crate::validation::{
    normalize_scene_path, reject_ytyp_scene_path, validate_scene_asset_contract,
    validate_scene_definition_refs,
};

impl EngineSceneGatewayService {
    pub(crate) fn load_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        json_result(self.load_json(&payload))
    }

    fn load_json(&self, payload: &Blob) -> Result<serde_json::Value, String> {
        let request = parse_payload(payload)?;
        let path = request
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "engine.scene load_json_v1 requires non-empty path".to_owned())?;
        let path = normalize_scene_path(path);
        reject_ytyp_scene_path(&path)?;

        let replace = request
            .get("replace")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if !replace {
            return Err(
                "engine.scene load_json_v1 currently supports replace=true only".to_owned(),
            );
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            return Err(format!(
                "engine.scene cannot load '{path}': asset gateway '{}' is unavailable",
                newengine_assets::consts::ASSET_SERVICE_ID
            ));
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        if let Some(mounts) = self.asset_mounts {
            let roots = newengine_runtime_host::asset_bootstrap::collect_app_asset_roots(
                mounts.app_dir_name,
                mounts.app_assets_env,
            );
            newengine_runtime_host::asset_bootstrap::mount_asset_roots_best_effort(&assets, &roots);
        }

        let bytes = assets.text_v1(&path).map_err(|error| {
            format!("engine.scene load_json_v1 asset read failed path='{path}' err='{error}'")
        })?;
        let asset = serde_json::from_slice::<SceneAsset>(&bytes).map_err(|error| {
            format!("engine.scene load_json_v1 scene json parse failed path='{path}' err='{error}'")
        })?;
        validate_scene_asset_contract(&path, &asset)?;
        validate_scene_definition_refs(&asset)?;

        let authority = self.scene.authority_snapshot();
        if authority.has_split_world_authority() {
            newengine_ulog_api::ulog::warn!(
                "engine.scene load_json_v1 running while world authority is split authority='{}' notes='{}'",
                authority.authority_label(),
                authority.notes.join(";")
            );
        }
        let scene_lock = self.scene.scene();
        let mut scene = scene_lock.write();
        scene.load_asset(&asset).map_err(|error| {
            format!("engine.scene load_json_v1 scene apply failed path='{path}' err='{error}'")
        })?;
        drop(scene);

        Ok(serde_json::json!({
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
            "authority_topology": self.authority_json(),
        }))
    }

    pub(crate) fn save_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        json_result(self.save_json(&payload))
    }

    fn save_json(&self, payload: &Blob) -> Result<serde_json::Value, String> {
        let request = parse_payload(payload)?;
        let path = request
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let pretty = request
            .get("pretty")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let include_empty_entities = request
            .get("options")
            .and_then(|value| value.get("include_empty_entities"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

        let asset = self.current_scene_asset(include_empty_entities);
        let payload = serde_json::to_value(&asset).map_err(|error| error.to_string())?;
        let payload_text = if pretty {
            serde_json::to_string_pretty(&asset)
        } else {
            serde_json::to_string(&asset)
        }
        .map_err(|error| error.to_string())?;

        Ok(serde_json::json!({
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
            "payload_text": payload_text,
        }))
    }
}
