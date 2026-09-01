use super::*;

impl EngineAssetFacade {
    pub(crate) fn list_path(&self, logical_path: &str) -> Result<Vec<InspectorEntry>, String> {
        let value = self.client.vfs_list_json_v1(logical_path)?;
        let container_extensions = self.container_extensions();
        let entries = value
            .get("entries")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|value| {
                let mut entry = entry_from_vfs_value(value);
                if !entry.is_directory {
                    let extension = path_extension(&entry.logical_path);
                    entry.is_container = container_extensions.contains(&extension);
                }
                entry
            })
            .collect::<Vec<_>>();
        Ok(entries)
    }

    fn container_extensions(&self) -> std::collections::BTreeSet<String> {
        self.client
            .formats_json_v1()
            .ok()
            .and_then(|value| value.get("formats").and_then(Value::as_array).cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|format| {
                format
                    .get("container")
                    .is_some_and(|value| !value.is_null())
                    || format
                        .get("format")
                        .and_then(Value::as_str)
                        .is_some_and(|value| {
                            value.contains("listfile") || value.contains("container")
                        })
            })
            .filter_map(|format| {
                format
                    .get("ext")
                    .and_then(Value::as_str)
                    .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
            })
            .collect()
    }

    pub(crate) fn list_container(&self, logical_path: &str) -> Result<Vec<InspectorEntry>, String> {
        let logical_path = normalize_ref(logical_path);
        if logical_path.is_empty() || logical_path.contains('@') {
            return Err(
                "container listing requires a file reference without an entry selector".to_owned(),
            );
        }
        let request = AssetDecodeRequest {
            logical_path: logical_path.clone(),
            output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
            selector: json!({}),
                    format_descriptor: None,
};
        let bytes = self.client.decode_v1(&request)?;
        let manifest = serde_json::from_slice::<AssetFileManifest>(&bytes)
            .map_err(|error| format!("provider returned invalid AssetFileManifest: {error}"))?;
        if manifest.entries.is_empty() {
            return Err("provider manifest contains no addressable entries".to_owned());
        }
        let mut entries = manifest.entries;
        if let Ok(state) = self.client.dirty_state_json_v1(&logical_path) {
            apply_staged_projection(&logical_path, &mut entries, &state.staged_patches);
        }
        Ok(entries
            .into_iter()
            .map(|entry| {
                let byte_len = manifest_entry_byte_len(&entry.metadata);
                InspectorEntry {
                    name: entry.name,
                    logical_path: normalize_ref(&entry.entry_ref),
                    kind: "asset_entry".to_owned(),
                    asset_kind: entry.asset_kind,
                    semantic_gateway: if entry.route.gateway.trim().is_empty() {
                        "engine.assets.inspect".to_owned()
                    } else {
                        entry.route.gateway
                    },
                    is_directory: false,
                    is_container: false,
                    container_entry: true,
                    byte_len,
                }
            })
            .collect())
    }

    pub(crate) fn inspect(&self, asset_ref: &str) -> Result<AssetDocument, String> {
        self.client.inspect_document_json_v1(AssetDocumentRequest {
            asset_ref: normalize_ref(asset_ref),
            requester: ASSET_INSPECTOR_REQUESTER.to_owned(),
            ..AssetDocumentRequest::default()
        })
    }

    pub(crate) fn write_text(
        &self,
        logical_path: &str,
        text: String,
    ) -> Result<TextAssetWriteResponseV1, String> {
        self.client
            .package_write_text_json_v1(TextAssetWriteRequestV1 {
                logical_path: normalize_ref(logical_path),
                text,
                expected_hash: String::new(),
                requested_capability: ASSETS_PACKAGE_WRITER_CAPABILITY_ID.to_owned(),
            })
    }
}

fn entry_from_vfs_value(value: &Value) -> InspectorEntry {
    let name = first_string(value, &["name", "file_name", "display_name"])
        .unwrap_or_else(|| "<unnamed>".to_owned());
    let logical_path = first_string(value, &["logical_path", "path", "reference", "id"])
        .unwrap_or_else(|| name.clone());
    let kind = first_string(value, &["kind", "node_kind", "entry_kind"])
        .unwrap_or_else(|| "asset".to_owned());
    let is_directory =
        bool_field(value, &["is_dir", "directory", "is_directory"]).unwrap_or_else(|| {
            matches!(
                kind.trim().to_ascii_lowercase().as_str(),
                "directory" | "dir" | "folder" | "mount"
            )
        });
    InspectorEntry {
        name,
        logical_path: normalize_ref(&logical_path),
        kind,
        asset_kind: first_string(value, &["asset_kind", "content_kind", "type"])
            .unwrap_or_else(|| "asset".to_owned()),
        semantic_gateway: first_string(value, &["semantic_gateway", "gateway"])
            .unwrap_or_else(|| "engine.assets".to_owned()),
        is_directory,
        is_container: false,
        container_entry: false,
        byte_len: value
            .get("byte_len")
            .or_else(|| value.get("size"))
            .and_then(Value::as_u64),
    }
}

pub(super) fn manifest_entry_byte_len(
    metadata: &std::collections::BTreeMap<String, String>,
) -> Option<u64> {
    [
        "byte_len",
        "size",
        "payload_len",
        "compressed_len",
        "body_len",
    ]
    .iter()
    .find_map(|key| metadata.get(*key))
    .and_then(|value| value.trim().parse::<u64>().ok())
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

fn normalize_ref(value: &str) -> String {
    value.trim().replace('\\', "/").trim_matches('/').to_owned()
}

fn path_extension(path: &str) -> String {
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(super) fn apply_staged_projection(
    logical_path: &str,
    entries: &mut Vec<newengine_assets_api::AssetEntryManifest>,
    patches: &[AssetPatch],
) {
    for patch in patches {
        for operation in &patch.operations {
            let (_, entry_name) = patch
                .asset_ref
                .split_once('@')
                .map(|(path, entry)| (path, Some(entry)))
                .unwrap_or((patch.asset_ref.as_str(), None));
            let Some(entry_name) = entry_name else {
                continue;
            };
            match operation.op.trim().to_ascii_lowercase().as_str() {
                "delete" | "remove" => entries.retain(|entry| {
                    entry.name != entry_name
                        && entry.entry_ref != format!("{logical_path}@{entry_name}")
                }),
                "rename" => {
                    let new_name = operation
                        .value
                        .as_str()
                        .or_else(|| operation.value.get("name").and_then(Value::as_str));
                    if let Some(new_name) = new_name {
                        if let Some(entry) =
                            entries.iter_mut().find(|entry| entry.name == entry_name)
                        {
                            entry.name = new_name.to_owned();
                            entry.entry_ref = format!("{logical_path}@{new_name}");
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
