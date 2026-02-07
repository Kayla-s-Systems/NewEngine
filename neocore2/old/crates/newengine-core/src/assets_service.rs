#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::host_api;
use abi_stable::std_types::{RResult, RString};
use newengine_assets::store::ImporterBindingInfo;
use newengine_assets::types::{AssetKey, AssetState};
use newengine_assets::AssetStore;
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;

pub const ASSET_SERVICE_ID: &str = "asset.manager";

pub mod method {
    pub const STATS_JSON: &str = "asset.stats_json";
    pub const IMPORTERS_JSON: &str = "asset.importers_json";
    pub const LIST_JSON: &str = "asset.list_json";
    pub const INFO_JSON: &str = "asset.info_json";
    pub const LOAD: &str = "asset.load";
    pub const RELOAD: &str = "asset.reload";
}

#[derive(Debug, Serialize)]
struct AssetStatsResp {
    sources: usize,
    importers: usize,
    importers_bindings: usize,
    state_entries: usize,
    blobs_ready: usize,
    blobs_bytes: u64,
    queue_len: usize,
}

#[derive(Debug, Serialize)]
struct ImporterBindingResp {
    ext: String,
    stable_id: String,
    output_type_id: String,
    priority: i32,
}

#[derive(Debug, Serialize)]
struct AssetListItem {
    id_u128: String,
    state: String,
    type_id: Option<String>,
    format: Option<String>,
    bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct AssetInfoResp {
    ok: bool,
    logical_path: String,
    id_u128: String,
    ext: Option<String>,
    state: String,
    type_id: Option<String>,
    format: Option<String>,
    bytes: Option<u64>,
    importer_candidates: Vec<ImporterBindingResp>,
    queue_len: usize,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoadResp {
    ok: bool,
    id_u128: Option<String>,
    error: Option<String>,
}

pub struct AssetManagerService {
    store: Arc<AssetStore>,
}

impl AssetManagerService {
    pub fn new(store: Arc<AssetStore>) -> Self {
        Self { store }
    }
}

impl ServiceV1 for AssetManagerService {
    fn id(&self) -> CapabilityId {
        RString::from(ASSET_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let d = json!({
          "id": ASSET_SERVICE_ID,
          "version": 1,
          "methods": [
            { "name": method::STATS_JSON, "payload": "empty", "returns": "json AssetStatsResp" },
            { "name": method::IMPORTERS_JSON, "payload": "empty", "returns": "json [ImporterBindingResp]" },
            { "name": method::LIST_JSON, "payload": "empty", "returns": "json [AssetListItem]" },
            { "name": method::INFO_JSON, "payload": "utf8 logical_path", "returns": "json AssetInfoResp" },
            { "name": method::LOAD, "payload": "utf8 logical_path", "returns": "json LoadResp" },
            { "name": method::RELOAD, "payload": "utf8 logical_path", "returns": "json LoadResp" }
          ],
          "console": {
            "commands": [
              {
                "name": "asset.stats",
                "help": "Asset store stats",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::STATS_JSON,
                "payload": "empty"
              },
              {
                "name": "asset.importers",
                "help": "List importer bindings (ext -> importer)",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::IMPORTERS_JSON,
                "payload": "empty"
              },
              {
                "name": "asset.list",
                "help": "List known assets snapshot (ids/states)",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::LIST_JSON,
                "payload": "empty"
              },
              {
                "name": "asset.info",
                "help": "Pretty asset record: asset.info <logical_path>",
                "usage": "asset.info <logical_path>",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::INFO_JSON,
                "payload": "raw"
              },
              {
                "name": "asset.load",
                "help": "Enqueue asset load: asset.load <logical_path>",
                "usage": "asset.load <logical_path>",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::LOAD,
                "payload": "raw"
              },
              {
                "name": "asset.reload",
                "help": "Reload asset: asset.reload <logical_path>",
                "usage": "asset.reload <logical_path>",
                "kind": "service_call",
                "service_id": ASSET_SERVICE_ID,
                "method": method::RELOAD,
                "payload": "raw"
              }
            ]
          }
        });

        RString::from(d.to_string())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let m = method.to_string();

        match m.as_str() {
            method::STATS_JSON => {
                let s = self.store.stats_snapshot();
                let resp = AssetStatsResp {
                    sources: s.sources,
                    importers: s.importers,
                    importers_bindings: s.importers_bindings,
                    state_entries: s.state_entries,
                    blobs_ready: s.blobs_ready,
                    blobs_bytes: s.blobs_bytes,
                    queue_len: s.queue_len,
                };
                let bytes = serde_json::to_vec(&resp).unwrap_or_default();
                RResult::ROk(Blob::from(bytes))
            }
            method::IMPORTERS_JSON => {
                let bindings = self.store.importer_bindings();
                let resp: Vec<ImporterBindingResp> = bindings
                    .into_iter()
                    .map(|b: ImporterBindingInfo| ImporterBindingResp {
                        ext: b.ext,
                        stable_id: b.stable_id.to_string(),
                        output_type_id: b.output_type_id.to_string(),
                        priority: b.priority.0,
                    })
                    .collect();
                let bytes = serde_json::to_vec(&resp).unwrap_or_default();
                RResult::ROk(Blob::from(bytes))
            }
            method::LIST_JSON => {
                let list = self.store.list_snapshot(256);
                let resp: Vec<AssetListItem> = list
                    .into_iter()
                    .map(|x| AssetListItem {
                        id_u128: format!("{:032x}", x.id_u128),
                        state: x.state,
                        type_id: x.type_id,
                        format: x.format,
                        bytes: x.bytes,
                    })
                    .collect();
                let bytes = serde_json::to_vec(&resp).unwrap_or_default();
                RResult::ROk(Blob::from(bytes))
            }
            method::INFO_JSON => {
                use std::path::Path;

                let logical_path = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                if logical_path.is_empty() {
                    let bytes = serde_json::to_vec(&AssetInfoResp {
                        ok: false,
                        logical_path: logical_path.clone(),
                        id_u128: String::new(),
                        ext: None,
                        state: "invalid".into(),
                        type_id: None,
                        format: None,
                        bytes: None,
                        importer_candidates: Vec::new(),
                        queue_len: self.store.queue_len(),
                        error: Some("empty path".into()),
                    })
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                }

                let key = AssetKey::new(&logical_path, 0);
                let id = key.id();
                let id_u128 = format!("{:032x}", id.to_u128());

                let ext = Path::new(&logical_path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_start_matches('.').to_ascii_lowercase());

                let st = self.store.state(id);
                let (state_str, state_err) = match st {
                    AssetState::Unloaded => ("unloaded".to_string(), None),
                    AssetState::Loading => ("loading".to_string(), None),
                    AssetState::Ready => ("ready".to_string(), None),
                    AssetState::Failed(e) => ("failed".to_string(), Some(e.to_string())),
                };

                let (type_id, format, bytes_len) = match self.store.get_blob(id) {
                    Some(b) => (
                        Some(b.type_id.to_string()),
                        Some(b.format.to_string()),
                        Some(b.payload.len() as u64),
                    ),
                    None => (None, None, None),
                };

                let mut importer_candidates = Vec::new();
                if let Some(ext) = ext.as_ref() {
                    for b in self.store.importer_bindings() {
                        if &b.ext == ext {
                            importer_candidates.push(ImporterBindingResp {
                                ext: b.ext,
                                stable_id: b.stable_id.to_string(),
                                output_type_id: b.output_type_id.to_string(),
                                priority: b.priority.0,
                            });
                        }
                    }
                    importer_candidates.sort_by(|a, b| {
                        b.priority
                            .cmp(&a.priority)
                            .then_with(|| a.stable_id.cmp(&b.stable_id))
                    });
                }

                let resp = AssetInfoResp {
                    ok: true,
                    logical_path,
                    id_u128,
                    ext,
                    state: state_str,
                    type_id,
                    format,
                    bytes: bytes_len,
                    importer_candidates,
                    queue_len: self.store.queue_len(),
                    error: state_err,
                };

                let bytes = serde_json::to_vec(&resp).unwrap_or_default();
                RResult::ROk(Blob::from(bytes))
            }
            method::LOAD => {
                let path = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                if path.is_empty() {
                    let bytes = serde_json::to_vec(&LoadResp {
                        ok: false,
                        id_u128: None,
                        error: Some("empty path".to_string()),
                    })
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                }

                match self.store.load_path(&path) {
                    Ok(id) => {
                        let bytes = serde_json::to_vec(&LoadResp {
                            ok: true,
                            id_u128: Some(format!("{:032x}", id.to_u128())),
                            error: None,
                        })
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                    Err(e) => {
                        let bytes = serde_json::to_vec(&LoadResp {
                            ok: false,
                            id_u128: None,
                            error: Some(e.to_string()),
                        })
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                }
            }
            method::RELOAD => {
                let path = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                if path.is_empty() {
                    let bytes = serde_json::to_vec(&LoadResp {
                        ok: false,
                        id_u128: None,
                        error: Some("empty path".to_string()),
                    })
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                }

                match self.store.reload_path(&path) {
                    Ok(id) => {
                        let bytes = serde_json::to_vec(&LoadResp {
                            ok: true,
                            id_u128: Some(format!("{:032x}", id.to_u128())),
                            error: None,
                        })
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                    Err(e) => {
                        let bytes = serde_json::to_vec(&LoadResp {
                            ok: false,
                            id_u128: None,
                            error: Some(e.to_string()),
                        })
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                }
            }
            _ => RResult::RErr(RString::from(format!("unknown method: {m}"))),
        }
    }
}

/// Register asset manager service into host services.
pub fn register_asset_manager_service(asset_store: Arc<AssetStore>) {
    let svc = AssetManagerService::new(asset_store);
    let dyn_svc: ServiceV1Dyn<'static> =
        ServiceV1Dyn::from_value(svc, abi_stable::sabi_trait::TD_Opaque);

    let _ = host_api::host_register_service_impl(dyn_svc, false);
}