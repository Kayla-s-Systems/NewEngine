#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::erased_types::TD_Opaque;
use abi_stable::std_types::{RResult, RString};

use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::plugins::{default_host_api, has_service};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1_TO};
use newengine_scene::{SceneAsset, SceneAssetOptions};

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::scene_bridge::SceneBridge;

#[derive(Clone)]
pub struct SceneIoHostService {
    scene: Arc<SceneBridge>,
}

impl SceneIoHostService {
    #[inline]
    pub fn new(scene: Arc<SceneBridge>) -> Self {
        Self { scene }
    }

    #[inline]
    fn ok_json(v: serde_json::Value) -> RResult<Blob, RString> {
        match serde_json::to_vec(&v) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    #[inline]
    fn err_json(msg: &str) -> RResult<Blob, RString> {
        Self::ok_json(serde_json::json!({"ok": false, "error": msg}))
    }

    #[inline]
    fn read_scene_bytes_best_effort(path: &str) -> Result<Vec<u8>, String> {
        let p = path.trim();
        if p.is_empty() {
            return Err("scene.load: empty path".to_string());
        }

        // Prefer AssetManager service when available.
        if has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            let assets = AssetServiceClient::new(default_host_api());

            if let Ok(id) = assets.load(p) {
                let _ = wait_ready(&assets, &id, Duration::from_millis(350));
                if let Ok((_meta, payload)) = assets.blob_wire_v1(&id) {
                    if !payload.is_empty() {
                        return Ok(payload);
                    }
                }
            }
        }

        // Fallback: direct filesystem read.
        std::fs::read(p).map_err(|e| format!("scene.load: read failed path='{p}' err='{e}'"))
    }

    #[inline]
    fn write_scene_bytes_best_effort(path: &str, bytes: &[u8]) -> Result<(), String> {
        let p = path.trim();
        if p.is_empty() {
            return Err("scene.save: empty path".to_string());
        }

        if let Some(parent) = Path::new(p).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("scene.save: mkdir failed dir='{}' err='{e}'", parent.display()))?;
            }
        }

        std::fs::write(p, bytes)
            .map_err(|e| format!("scene.save: write failed path='{p}' err='{e}'"))
    }
}

impl ServiceV1 for SceneIoHostService {
    #[inline]
    fn id(&self) -> CapabilityId {
        CapabilityId::from(newengine_scene_io::SCENE_IO_SERVICE_ID)
    }

    #[inline]
    fn describe(&self) -> RString {
        let v = serde_json::json!({
            "id": newengine_scene_io::SCENE_IO_SERVICE_ID,
            "version": 1,
            "schema": "kalitech.scene.asset.v1",
            "methods": [
                newengine_scene_io::method::FORMATS_JSON,
                newengine_scene_io::method::LOAD_JSON_V1,
                newengine_scene_io::method::SAVE_JSON_V1,
            ],
            "notes": "Host fallback implementation. A plugin may override this service id.",
        });
        RString::from(serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let m = method.as_str();

        match m {
            newengine_scene_io::method::FORMATS_JSON => {
                return Self::ok_json(serde_json::json!({
                    "ok": true,
                    "formats": [
                        {
                            "id": "kalitech.scene.asset.v1",
                            "ext": ".scene.json",
                            "can_read": true,
                            "can_write": true,
                        },
                        {
                            "id": "kalitech.scene.asset.v1",
                            "ext": ".nescene.json",
                            "can_read": true,
                            "can_write": true,
                        }
                    ]
                }));
            }

            newengine_scene_io::method::LOAD_JSON_V1 => {
                let s = match String::from_utf8(payload.into_vec()) {
                    Ok(v) => v,
                    Err(_) => return Self::err_json("scene.load: request is not utf-8"),
                };

                let v: serde_json::Value = match serde_json::from_str(&s) {
                    Ok(v) => v,
                    Err(e) => return Self::err_json(&format!("scene.load: bad json request: {e}")),
                };

                let path = v
                    .get("path")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .trim();

                let replace = v
                    .get("replace")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(true);

                if path.is_empty() {
                    return Self::err_json("scene.load: missing path");
                }

                let bytes = match Self::read_scene_bytes_best_effort(path) {
                    Ok(b) => b,
                    Err(e) => return Self::err_json(&e),
                };

                let asset: SceneAsset = match serde_json::from_slice(&bytes) {
                    Ok(a) => a,
                    Err(e) => {
                        return Self::err_json(&format!(
                            "scene.load: failed to parse SceneAsset json path='{path}' err='{e}'"
                        ))
                    }
                };

                if replace {
                    self.scene.cmd_load_scene_asset(asset);
                } else {
                    return Self::err_json("scene.load: merge mode not implemented yet");
                }

                return Self::ok_json(serde_json::json!({
                    "ok": true,
                    "queued": true,
                    "path": path,
                }));
            }

            newengine_scene_io::method::SAVE_JSON_V1 => {
                let s = match String::from_utf8(payload.into_vec()) {
                    Ok(v) => v,
                    Err(_) => return Self::err_json("scene.save: request is not utf-8"),
                };

                let v: serde_json::Value = match serde_json::from_str(&s) {
                    Ok(v) => v,
                    Err(e) => return Self::err_json(&format!("scene.save: bad json request: {e}")),
                };

                let path = v
                    .get("path")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .trim();

                let pretty = v
                    .get("pretty")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(true);

                let include_empty_entities = v
                    .get("options")
                    .and_then(|o| o.get("include_empty_entities"))
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);

                if path.is_empty() {
                    return Self::err_json("scene.save: missing path");
                }

                // Extract asset under a write lock (to ensure GUID invariants are satisfied).
                let asset = {
                    let scene = self.scene.scene();
                    let mut s = scene.write();
                    s.to_asset(SceneAssetOptions {
                        include_empty_entities,
                    })
                };

                let json_bytes = if pretty {
                    serde_json::to_vec_pretty(&asset).map_err(|e| e.to_string())
                } else {
                    serde_json::to_vec(&asset).map_err(|e| e.to_string())
                };

                let bytes = match json_bytes {
                    Ok(b) => b,
                    Err(e) => return Self::err_json(&format!("scene.save: serialize failed: {e}")),
                };

                if let Err(e) = Self::write_scene_bytes_best_effort(path, &bytes) {
                    return Self::err_json(&e);
                }

                return Self::ok_json(serde_json::json!({
                    "ok": true,
                    "path": path,
                    "bytes": bytes.len(),
                    "schema": asset.schema,
                    "version": asset.version,
                    "entities": asset.entities.len(),
                }));
            }

            _ => {}
        }

        RResult::RErr(RString::from(format!("scene io: unknown method '{m}'")))
    }
}

/// Register a host fallback scene IO service if no plugin provides it.
#[inline]
pub fn register_scene_io_best_effort(scene: Arc<SceneBridge>) {
    if has_service(newengine_scene_io::SCENE_IO_SERVICE_ID) {
        log::info!(
            "scene io: service '{}' already provided; skipping host fallback",
            newengine_scene_io::SCENE_IO_SERVICE_ID
        );
        return;
    }

    let svc = SceneIoHostService::new(scene);
    let dyn_svc = ServiceV1_TO::from_value(svc, TD_Opaque);

    let host = default_host_api();
    match (host.register_service_v1)(dyn_svc) {
        RResult::ROk(()) => {
            log::info!(
                "scene io: host fallback registered id='{}'",
                newengine_scene_io::SCENE_IO_SERVICE_ID
            );
        }
        RResult::RErr(e) => {
            log::error!(
                "scene io: failed to register host fallback id='{}' err='{}'",
                newengine_scene_io::SCENE_IO_SERVICE_ID,
                e
            );
        }
    }
}
