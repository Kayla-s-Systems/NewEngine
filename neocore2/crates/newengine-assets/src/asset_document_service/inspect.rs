use super::*;

impl AssetInspectState {
    pub(super) fn new(host: HostApiV1) -> Self {
        Self {
            assets: AssetServiceClient::new(host.clone()),
            host,
            asset_types_service_id: RString::from(ENGINE_ASSET_TYPES_SERVICE_ID),
            resolve_method: MethodName::from(file_type_method::RESOLVE_JSON_V1),
        }
    }

    pub(super) fn inspect_document(&self, request: AssetDocumentRequest) -> AssetDocument {
        let asset_ref = normalize_asset_ref(&request.asset_ref);
        let mut diagnostics = Vec::new();
        if asset_ref.is_empty() {
            return AssetDocument {
                title: "No asset selected".to_owned(),
                diagnostics: vec![AssetDocumentDiagnostic::warn(
                    "asset_ref.empty",
                    "inspect_document_json_v1 requires asset_ref",
                )],
                ..AssetDocument::default()
            };
        }

        let descriptor_result = self.resolve_descriptor(&asset_ref);
        let descriptor = match descriptor_result {
            Ok(result) => {
                if !result.known {
                    diagnostics.push(AssetDocumentDiagnostic::warn(
                        "asset.type.unknown",
                        format!("no provider descriptor registered for '{}'; showing VFS-level metadata only", result.extension),
                    ));
                }
                result.descriptor
            }
            Err(error) => {
                diagnostics.push(AssetDocumentDiagnostic::warn(
                    "assets.types.unavailable",
                    error,
                ));
                None
            }
        };

        let (logical_path, entry_selector) = split_entry_ref(&asset_ref);
        let extension = path_extension(&logical_path);
        let title = entry_selector
            .as_ref()
            .map(|entry| format!("{} @ {}", file_name(&logical_path), entry))
            .unwrap_or_else(|| file_name(&logical_path));

        let schema_editable = descriptor
            .as_ref()
            .map(|d| d.schema_editable || d.editable || !d.edit_contract.trim().is_empty())
            .unwrap_or(false);
        let can_apply_patch = descriptor
            .as_ref()
            .map(|d| d.write_back_available && !d.writer_capability.trim().is_empty())
            .unwrap_or(false);
        let writer_capability = descriptor
            .as_ref()
            .map(|d| d.writer_capability.clone())
            .unwrap_or_default();
        let write_owner = if can_apply_patch {
            writer_capability.clone()
        } else if schema_editable {
            "missing format writer provider".to_owned()
        } else {
            "read-only provider schema".to_owned()
        };

        let icon = icon_for_descriptor(descriptor.as_ref(), &extension).to_owned();
        let mut document = AssetDocument {
            asset_ref: asset_ref.clone(),
            title,
            icon: icon.clone(),
            document_kind: descriptor
                .as_ref()
                .map(|d| d.asset_kind.clone())
                .unwrap_or_else(|| "asset_document".to_owned()),
            asset_kind: descriptor
                .as_ref()
                .map(|d| d.asset_kind.clone())
                .unwrap_or_else(|| "asset".to_owned()),
            content_kind: descriptor.as_ref().and_then(|d| d.content_kind),
            semantic_gateway: descriptor
                .as_ref()
                .map(|d| d.semantic_gateway.clone())
                .unwrap_or_else(|| "engine.assets".to_owned()),
            provider_service: descriptor
                .as_ref()
                .map(|d| d.handler_service.clone())
                .unwrap_or_default(),
            inspect_contract: descriptor
                .as_ref()
                .map(|d| d.inspect_contract.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "asset.inspect.generic.v1".to_owned()),
            edit_contract: descriptor
                .as_ref()
                .map(|d| d.edit_contract.clone())
                .unwrap_or_default(),
            editable: can_apply_patch,
            editable_fields_available: schema_editable,
            can_apply_patch,
            write_owner,
            writer_capability,
            preview: AssetDocumentPreview {
                kind: descriptor
                    .as_ref()
                    .map(|d| d.asset_kind.clone())
                    .unwrap_or_else(|| "asset".to_owned()),
                icon,
                thumbnail_ref: String::new(),
                summary: descriptor
                    .as_ref()
                    .map(|d| d.notes.clone())
                    .unwrap_or_else(|| "generic VFS asset document".to_owned()),
            },
            descriptor: descriptor.clone(),
            diagnostics,
            ..AssetDocument::default()
        };

        document.sections.push(identity_section(
            &asset_ref,
            &logical_path,
            entry_selector.as_deref(),
            &extension,
        ));
        document
            .sections
            .push(type_descriptor_section(descriptor.as_ref()));
        document
            .sections
            .push(provider_contract_section(descriptor.as_ref()));
        document
            .sections
            .push(edit_policy_section(descriptor.as_ref()));
        if document.editable_fields_available {
            document.sections.push(editable_schema_section(
                &document.title,
                descriptor.as_ref(),
                document.can_apply_patch,
            ));
        }
        if document.editable_fields_available && !document.can_apply_patch {
            document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "write_back.missing_capability",
                "editable schema is available, but Apply/Save is disabled until a format/package writer capability is registered",
            ));
        }

        self.hydrate_list_file_manifest_section(
            &logical_path,
            entry_selector.as_deref(),
            &mut document,
        );
        self.hydrate_text_document(
            &logical_path,
            entry_selector.as_deref(),
            &extension,
            &mut document,
        );
        self.hydrate_lifecycle_section(&logical_path, &mut document);
        match self.assets.dirty_state_json_v1(&logical_path) {
            Ok(state) => {
                document.dirty = state.dirty;
                if state.staged_operations > 0 {
                    document.diagnostics.push(AssetDocumentDiagnostic::info(
                        "edit.session.dirty",
                        format!(
                            "{} staged operation(s) await provider rebuild",
                            state.staged_operations
                        ),
                    ));
                }
            }
            Err(error) => document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "edit.session.unavailable",
                error,
            )),
        }
        document.schema_type = Some(asset_document_schema_type(&document));
        document.schema_contract = SCHEMA_RUNTIME_CONTRACT.to_owned();
        document.actions = asset_document_actions(&document, entry_selector.as_deref());
        document
    }

    fn resolve_descriptor(&self, asset_ref: &str) -> Result<AssetFileTypeProbeResult, String> {
        let payload = serde_json::to_vec(&AssetFileTypeProbeRequest {
            logical_path: asset_ref.to_owned(),
        })
        .map_err(|e| e.to_string())?;
        let res = (self.host.call_service_v1)(
            self.asset_types_service_id.clone(),
            self.resolve_method.clone(),
            Blob::from(payload),
        );
        let bytes = res
            .into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())?;
        serde_json::from_slice::<AssetFileTypeProbeResult>(&bytes).map_err(|e| e.to_string())
    }

    fn hydrate_list_file_manifest_section(
        &self,
        logical_path: &str,
        selected_entry: Option<&str>,
        document: &mut AssetDocument,
    ) {
        let Some(descriptor) = document.descriptor.as_ref() else {
            return;
        };
        if !descriptor
            .outputs
            .iter()
            .any(|output| output == ASSET_LIST_FILE_MANIFEST_OUTPUT)
        {
            return;
        }
        let client = &self.assets;
        let request = AssetDecodeRequest {
            logical_path: logical_path.to_owned(),
            output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
            selector: json!({}),
                    format_descriptor: None,
};
        let bytes = match client.decode_v1(&request) {
            Ok(bytes) => bytes,
            Err(error) => {
                document.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "listfile.manifest.unavailable",
                    error,
                ));
                return;
            }
        };
        let manifest = match serde_json::from_slice::<AssetFileManifest>(&bytes) {
            Ok(manifest) => manifest,
            Err(error) => {
                document.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "listfile.manifest.invalid",
                    format!("provider returned invalid AssetFileManifest: {error}"),
                ));
                return;
            }
        };

        let mut fields = vec![
            string_field("source", "Source", &manifest.source, false),
            string_field("file_kind", "File Kind", &manifest.file_kind, false),
            string_field("codec", "Codec", &manifest.codec, false),
            json_field(
                "entry_count",
                "Entry Count",
                json!(manifest.entries.len()),
                false,
            ),
        ];
        if let Some(entry_name) = selected_entry {
            if let Some(entry) = manifest.entries.iter().find(|entry| {
                entry.name == entry_name || entry.entry_ref.ends_with(&format!("@{entry_name}"))
            }) {
                fields.push(string_field(
                    "selected_entry",
                    "Selected Entry",
                    &entry.entry_ref,
                    false,
                ));
                fields.push(string_field(
                    "selected_kind",
                    "Selected Kind",
                    &entry.asset_kind,
                    false,
                ));
                fields.push(string_field(
                    "selected_route",
                    "Selected Route",
                    &entry.route.gateway,
                    false,
                ));
                fields.push(json_field(
                    "selected_dependencies",
                    "Dependencies",
                    json!(entry.dependencies),
                    false,
                ));
            }
        }
        for (idx, entry) in manifest.entries.iter().take(12).enumerate() {
            fields.push(string_field(
                &format!("entry_{idx:02}"),
                &format!("Entry {}", idx + 1),
                &format!(
                    "{} · {} · {}",
                    entry.name, entry.asset_kind, entry.route.gateway
                ),
                false,
            ));
        }
        document.sections.push(AssetDocumentSection {
            id: "listfile_entries".to_owned(),
            title: "NEF8/ListFile entries".to_owned(),
            fields,
        });
    }

    fn hydrate_text_document(
        &self,
        logical_path: &str,
        selected_entry: Option<&str>,
        extension: &str,
        document: &mut AssetDocument,
    ) {
        const MAX_EDITOR_TEXT_BYTES: usize = 1024 * 1024;
        if selected_entry.is_some() || !is_text_asset_extension(extension) {
            return;
        }
        let bytes = match self.assets.text_v1(logical_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                document.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "text.decode.unavailable",
                    format!("engine.assets text decode failed: {error}"),
                ));
                return;
            }
        };
        let byte_len = bytes.len();
        let truncated = byte_len > MAX_EDITOR_TEXT_BYTES;
        let visible_bytes = if truncated {
            &bytes[..utf8_prefix_len(&bytes, MAX_EDITOR_TEXT_BYTES)]
        } else {
            bytes.as_slice()
        };
        let content = match std::str::from_utf8(visible_bytes) {
            Ok(content) => content.to_owned(),
            Err(error) => {
                document.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "text.decode.non_utf8",
                    format!("asset is not valid UTF-8: {error}"),
                ));
                return;
            }
        };
        let writable = !truncated
            && self
                .assets
                .package_writer_info_json_v1(json!({}))
                .ok()
                .and_then(|value| {
                    value
                        .get("operations")
                        .and_then(|operations| operations.get("loose_vfs_write_back"))
                        .and_then(Value::as_bool)
                })
                .unwrap_or(false);
        let line_count = text_line_count(&content);
        document.text = Some(AssetDocumentText {
            encoding: "utf-8".to_owned(),
            language: text_language_for_extension(extension).to_owned(),
            content,
            byte_len: byte_len as u64,
            line_count,
            truncated,
            editable: writable,
        });
        document.preview.kind = "text".to_owned();
        document.preview.summary = format!(
            "UTF-8 text · {} line(s) · {} bytes{}",
            line_count,
            byte_len,
            if truncated {
                " · preview truncated"
            } else {
                ""
            }
        );
        if writable {
            document.editable = true;
            document.editable_fields_available = true;
            document.can_apply_patch = true;
            document.edit_contract = "newengine.asset.text.utf8.v1".to_owned();
            document.writer_capability = ASSETS_PACKAGE_WRITER_CAPABILITY_ID.to_owned();
            document.write_owner = "engine.assets.package_writer".to_owned();
        } else if truncated {
            document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "text.editor.size_limit",
                format!(
                    "text editor is read-only because the asset exceeds {} bytes",
                    MAX_EDITOR_TEXT_BYTES
                ),
            ));
        }
    }

    fn hydrate_lifecycle_section(&self, logical_path: &str, document: &mut AssetDocument) {
        let client = &self.assets;
        let mut fields = Vec::new();
        match client.uid_json_v1(logical_path) {
            Ok(value) => fields.push(json_field("uid", "UID", value, false)),
            Err(error) => document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "assets.uid.unavailable",
                error,
            )),
        }
        match client.status_json_v1(logical_path) {
            Ok(value) => fields.push(json_field("status", "Status", value, false)),
            Err(error) => document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "assets.status.unavailable",
                error,
            )),
        }
        if !fields.is_empty() {
            document.sections.push(AssetDocumentSection {
                id: "lifecycle".to_owned(),
                title: "Lifecycle / import state".to_owned(),
                fields,
            });
        }
    }
}

fn is_text_asset_extension(extension: &str) -> bool {
    matches!(
        extension.trim().to_ascii_lowercase().as_str(),
        "txt"
            | "md"
            | "json"
            | "json5"
            | "xml"
            | "toml"
            | "yaml"
            | "yml"
            | "ini"
            | "cfg"
            | "ron"
            | "csv"
            | "glsl"
            | "vert"
            | "frag"
            | "comp"
            | "tesc"
            | "tese"
            | "geom"
            | "wgsl"
            | "hlsl"
            | "lua"
            | "py"
            | "rs"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cs"
            | "js"
            | "ts"
            | "css"
            | "html"
            | "bat"
            | "cmd"
            | "ps1"
            | "sh"
            | "log"
            | "conf"
            | "properties"
            | "env"
            | "gitignore"
            | "java"
            | "kt"
            | "kts"
            | "sql"
    )
}

fn text_language_for_extension(extension: &str) -> &'static str {
    match extension.trim().to_ascii_lowercase().as_str() {
        "md" => "markdown",
        "json" | "json5" => "json",
        "xml" => "xml",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "vert" | "frag" | "comp" | "tesc" | "tese" | "geom" | "glsl" => "glsl",
        "wgsl" => "wgsl",
        "hlsl" => "hlsl",
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "html" => "html",
        "css" => "css",
        "lua" => "lua",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "sql" => "sql",
        "properties" | "conf" | "env" | "gitignore" | "log" => "text",
        _ => "text",
    }
}

fn utf8_prefix_len(bytes: &[u8], limit: usize) -> usize {
    let mut end = limit.min(bytes.len());
    while end > 0 && std::str::from_utf8(&bytes[..end]).is_err() {
        end -= 1;
    }
    end
}

fn text_line_count(content: &str) -> usize {
    if content.is_empty() {
        1
    } else {
        content.lines().count() + usize::from(content.ends_with('\n'))
    }
}

#[cfg(test)]
mod text_document_tests {
    use super::*;

    #[test]
    fn text_extension_detection_is_explicit() {
        assert!(is_text_asset_extension("json"));
        assert!(is_text_asset_extension("RS"));
        assert!(is_text_asset_extension("log"));
        assert!(is_text_asset_extension("properties"));
        assert!(!is_text_asset_extension("ytd"));
        assert!(!is_text_asset_extension("ydd"));
    }

    #[test]
    fn utf8_prefix_never_splits_a_codepoint() {
        let bytes = "abcЖ".as_bytes();
        assert_eq!(
            std::str::from_utf8(&bytes[..utf8_prefix_len(bytes, 4)]).unwrap(),
            "abc"
        );
    }

    #[test]
    fn text_line_count_preserves_trailing_empty_line() {
        assert_eq!(
            text_line_count(
                "a
b
"
            ),
            3
        );
        assert_eq!(text_line_count(""), 1);
    }
}
