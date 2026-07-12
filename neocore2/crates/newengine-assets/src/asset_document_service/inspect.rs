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
        self.hydrate_lifecycle_section(&logical_path, &mut document);
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
        match client.thumbnail_json_v1(json!({ "logical_path": logical_path })) {
            Ok(value) => {
                if let Some(label) = value
                    .get("thumbnail")
                    .and_then(Value::as_str)
                    .or_else(|| value.get("cache_key").and_then(Value::as_str))
                {
                    document.preview.thumbnail_ref = label.to_owned();
                }
                fields.push(json_field("thumbnail", "Preview", value, false));
            }
            Err(error) => document.diagnostics.push(AssetDocumentDiagnostic::warn(
                "assets.thumbnail.unavailable",
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
