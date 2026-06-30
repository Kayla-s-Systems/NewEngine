#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    asset_document_action_id, asset_edit_method, asset_inspect_method, file_type_method,
    AssetAccess, AssetDecodeRequest, AssetDocument, AssetDocumentAction, AssetDocumentDiagnostic,
    AssetDocumentField, AssetDocumentPreview, AssetDocumentRequest, AssetDocumentSection,
    AssetFileManifest, AssetFileTypeDescriptor, AssetFileTypeProbeRequest,
    AssetFileTypeProbeResult, AssetPatch, AssetPatchOperation, AssetPatchResult, AssetService,
    ASSETS_EDIT_BACKEND_CAPABILITY_ID, ASSETS_EDIT_SERVICE_ID, ASSETS_EDIT_SERVICE_METHODS,
    ASSETS_INSPECT_BACKEND_CAPABILITY_ID, ASSETS_INSPECT_SERVICE_ID,
    ASSETS_INSPECT_SERVICE_METHODS, ASSET_LIST_FILE_MANIFEST_OUTPUT, ENGINE_ASSETS_EDIT_SERVICE_ID,
    ENGINE_ASSETS_INSPECT_SERVICE_ID, ENGINE_ASSET_TYPES_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_schema_api::{
    SchemaPatchDtoV1, SchemaPatchOperationV1, SchemaPropertyDescriptorV1, SchemaTransactionDtoV1,
    SchemaTypeDescriptorV1, SchemaValueKindV1, SCHEMA_RUNTIME_CONTRACT,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json,
    register_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize)]
pub struct AssetInspectServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub policy: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssetEditServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub methods: &'static [&'static str],
    pub backend: &'static str,
    pub policy: &'static str,
}

#[derive(Clone)]
struct AssetInspectState {
    host: HostApiV1,
}

#[derive(Clone)]
struct AssetEditState {
    host: HostApiV1,
}

impl AssetInspectState {
    fn new(host: HostApiV1) -> Self {
        Self { host }
    }

    fn inspect_document(&self, request: AssetDocumentRequest) -> AssetDocument {
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

        let mut document = AssetDocument {
            asset_ref: asset_ref.clone(),
            title,
            icon: icon_for_descriptor(descriptor.as_ref(), &extension).to_owned(),
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
                icon: icon_for_descriptor(descriptor.as_ref(), &extension).to_owned(),
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
            RString::from(ENGINE_ASSET_TYPES_SERVICE_ID),
            MethodName::from(file_type_method::RESOLVE_JSON_V1),
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
        let client = newengine_assets_api::AssetServiceClient::new(self.host.clone());
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
        let client = newengine_assets_api::AssetServiceClient::new(self.host.clone());
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

impl AssetEditState {
    fn new(host: HostApiV1) -> Self {
        Self { host }
    }

    fn validate_patch(&self, patch: AssetPatch) -> AssetPatchResult {
        let mut result = AssetPatchResult {
            asset_ref: normalize_asset_ref(&patch.asset_ref),
            ..AssetPatchResult::default()
        };
        if result.asset_ref.is_empty() {
            result.diagnostics.push(AssetDocumentDiagnostic::error(
                "asset_ref.empty",
                "patch requires asset_ref",
            ));
            return result;
        }
        if let Some(schema_patch) = patch.schema_patch.as_ref() {
            if schema_patch.target_ref.trim().is_empty()
                || schema_patch.target_ref != result.asset_ref
            {
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "schema.patch.target_mismatch",
                    "schema_patch.target_ref must match AssetPatch.asset_ref before provider validation",
                ));
                return result;
            }
            if schema_patch.operations.len() != patch.operations.len() {
                result.diagnostics.push(AssetDocumentDiagnostic::warn(
                    "schema.patch.operation_projection",
                    "schema_patch operation count differs from transport operations; provider will validate canonical schema operations first",
                ));
            }
        } else {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "schema.patch.missing",
                "AssetPatch has no SchemaPatchDtoV1 projection; accepting legacy transport only for compatibility during P2 migration",
            ));
        }
        if patch.transaction.is_none() {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "schema.transaction.missing",
                "AssetPatch has no SchemaTransactionDtoV1; undo/redo history will not be able to replay this change through engine.schema",
            ));
        }
        if patch.operations.is_empty() {
            result.accepted = true;
            result.diagnostics.push(AssetDocumentDiagnostic::info(
                "patch.empty",
                "empty patch is valid and has no write effect",
            ));
            return result;
        }
        if patch.edit_contract.trim().is_empty()
            || patch.edit_contract == newengine_assets_api::ASSETS_EDIT_RUNTIME_CONTRACT
        {
            result.diagnostics.push(AssetDocumentDiagnostic::warn(
                "edit.contract.generic",
                "generic asset edit provider can validate transport only; format provider must supply an explicit edit_contract before write-back",
            ));
            result.accepted = false;
            return result;
        }
        result.accepted = true;
        result.dirty = true;
        result.diagnostics.push(AssetDocumentDiagnostic::info(
            "patch.accepted",
            "patch transport is valid; provider-specific writer owns final validation",
        ));
        result
    }

    fn apply_patch(&self, patch: AssetPatch) -> AssetPatchResult {
        let mut result = self.validate_patch(patch.clone());
        if !result.accepted {
            return result;
        }

        let Some(first_op) = patch.operations.first() else {
            result.written = false;
            result.dirty = false;
            return result;
        };

        let operation = match first_op.op.trim().to_ascii_lowercase().as_str() {
            "remove" | "delete" => "delete",
            "rename" => "rename",
            "add" | "create" | "replace" | "update" => "update",
            _ => {
                result.accepted = false;
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "patch.operation.unsupported",
                    format!("unsupported asset patch operation '{}'", first_op.op),
                ));
                return result;
            }
        };

        let client = newengine_assets_api::AssetServiceClient::new(self.host.clone());
        let mut payload = json!({
            "target_ref": result.asset_ref,
            "operation": operation,
            "verify_after_build": true,
            "dry_run": false,
        });
        if operation == "update" {
            payload["payload_json"] = first_op.value.clone();
        }
        if operation == "rename" {
            if let Some(new_name) = first_op.value.as_str() {
                payload["new_name"] = json!(new_name);
            } else if let Some(new_name) = first_op.value.get("name").and_then(Value::as_str) {
                payload["new_name"] = json!(new_name);
            }
        }

        match client.list_file_repack_json_v1(payload) {
            Ok(value) => {
                let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
                let applied = value
                    .get("applied")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                result.accepted = ok;
                result.written = applied;
                result.dirty = !applied;
                result.diagnostics.push(AssetDocumentDiagnostic::info(
                    "listfile.repack",
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("NEF8 ListFile writer completed"),
                ));
                if !ok {
                    result.diagnostics.push(AssetDocumentDiagnostic::warn(
                        "listfile.repack.not_applied",
                        "writer rejected or dry-ran the patch",
                    ));
                }
                result
            }
            Err(error) => {
                result.accepted = false;
                result.written = false;
                result.dirty = true;
                result.diagnostics.push(AssetDocumentDiagnostic::error(
                    "listfile.repack.failed",
                    error,
                ));
                result
            }
        }
    }
}

fn asset_document_actions(
    document: &AssetDocument,
    selected_entry: Option<&str>,
) -> Vec<AssetDocumentAction> {
    let mut actions = Vec::new();
    let target_ref = normalize_asset_ref(&document.asset_ref);
    let can_write = document.can_apply_patch;
    let has_entry_selection = selected_entry.is_some() || target_ref.contains('@');

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::ADD_ENTRY.to_owned(),
        label: "Add".to_owned(),
        tooltip: "Add a new entry through the selected file type provider. Requires a source/template payload before AssetPatch can be emitted.".to_owned(),
        enabled: false,
        disabled_reason: if can_write { "requires add-entry dialog/schema payload" } else { "writer capability unavailable" }.to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.add_entry.input.v1",
            "Add Entry Input",
            vec![
                schema_property("entry_name", "Entry Name", SchemaValueKindV1::String, json!(""), true),
                schema_property("template_kind", "Template Kind", SchemaValueKindV1::Enum, json!(document.asset_kind.clone()), true),
            ],
        )),
    });

    let delete_patch = if can_write && has_entry_selection {
        let schema_op = SchemaPatchOperationV1 {
            op: "delete".to_owned(),
            path: "/entries/current".to_owned(),
            property_id: "entries.current".to_owned(),
            value: Value::Null,
            old_value: None,
        };
        Some(AssetPatch {
            asset_ref: target_ref.clone(),
            provider_service: document.provider_service.clone(),
            edit_contract: document.edit_contract.clone(),
            requester: "ui.assets.catalog".to_owned(),
            operations: vec![AssetPatchOperation {
                op: "delete".to_owned(),
                path: "/entries/current".to_owned(),
                value: Value::Null,
                old_value: None,
                schema_operation: Some(schema_op.clone()),
            }],
            schema_patch: Some(schema_patch_for_document(
                document,
                &target_ref,
                "asset-delete",
                vec![schema_op.clone()],
            )),
            transaction: Some(schema_transaction_for_document(
                document,
                &target_ref,
                "asset-delete",
                vec![schema_op],
                Vec::new(),
            )),
            ..AssetPatch::default()
        })
    } else {
        None
    };
    actions.push(AssetDocumentAction {
        id: asset_document_action_id::DELETE.to_owned(),
        label: "Delete".to_owned(),
        tooltip: "Delete the selected ListFile entry through engine.assets.edit.".to_owned(),
        enabled: delete_patch.is_some(),
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !has_entry_selection {
            "select a file@entry item first".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: delete_patch,
        input_schema: None,
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::RENAME.to_owned(),
        label: "Rename".to_owned(),
        tooltip: "Rename the selected entry. This action requires a rename dialog before a provider patch can be emitted.".to_owned(),
        enabled: false,
        disabled_reason: if can_write && has_entry_selection { "requires rename dialog value" } else { "select writable file@entry item first" }.to_owned(),
        requires_input: true,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: None,
        input_schema: Some(asset_action_input_schema(
            "newengine.assets.action.rename.input.v1",
            "Rename Entry Input",
            vec![schema_property("new_name", "New Name", SchemaValueKindV1::String, json!(selected_entry.unwrap_or("")), true)],
        )),
    });

    actions.push(AssetDocumentAction {
        id: asset_document_action_id::SAVE.to_owned(),
        label: "Save".to_owned(),
        tooltip: "Apply the current dirty AssetPatch through engine.assets.edit.".to_owned(),
        enabled: can_write && document.dirty,
        disabled_reason: if !can_write {
            "writer capability unavailable".to_owned()
        } else if !document.dirty {
            "no dirty AssetPatch to save".to_owned()
        } else {
            String::new()
        },
        requires_input: false,
        target_gateway: newengine_assets_api::ENGINE_ASSETS_EDIT_SERVICE_ID.to_owned(),
        method: asset_edit_method::APPLY_PATCH_JSON_V1.to_owned(),
        patch_template: if can_write && document.dirty {
            Some(AssetPatch {
                asset_ref: target_ref.clone(),
                provider_service: document.provider_service.clone(),
                edit_contract: document.edit_contract.clone(),
                requester: "ui.assets.catalog".to_owned(),
                operations: Vec::new(),
                schema_patch: Some(schema_patch_for_document(
                    document,
                    &target_ref,
                    "asset-save",
                    Vec::new(),
                )),
                transaction: Some(schema_transaction_for_document(
                    document,
                    &target_ref,
                    "asset-save",
                    Vec::new(),
                    Vec::new(),
                )),
                ..AssetPatch::default()
            })
        } else {
            None
        },
        input_schema: None,
    });

    actions
}

fn identity_section(
    asset_ref: &str,
    logical_path: &str,
    entry_selector: Option<&str>,
    extension: &str,
) -> AssetDocumentSection {
    AssetDocumentSection {
        id: "identity".to_owned(),
        title: "Identity".to_owned(),
        fields: vec![
            string_field("asset_ref", "Asset Ref", asset_ref, false),
            string_field("logical_path", "Logical Path", logical_path, false),
            string_field(
                "entry_selector",
                "Entry Selector",
                entry_selector.unwrap_or("<whole file>"),
                false,
            ),
            string_field("extension", "Extension", extension, false),
        ],
    }
}

fn type_descriptor_section(descriptor: Option<&AssetFileTypeDescriptor>) -> AssetDocumentSection {
    let Some(desc) = descriptor else {
        return AssetDocumentSection {
            id: "type".to_owned(),
            title: "Type descriptor".to_owned(),
            fields: vec![string_field("known", "Known Type", "false", false)],
        };
    };
    AssetDocumentSection {
        id: "type".to_owned(),
        title: "Type descriptor".to_owned(),
        fields: vec![
            string_field("asset_kind", "Asset Kind", &desc.asset_kind, false),
            string_field("codec_type", "Codec Type", &desc.codec_type, false),
            string_field("container", "Container", &desc.container, false),
            string_field("byte_owner", "Byte Owner", &desc.byte_owner, false),
            string_field(
                "semantic_gateway",
                "Semantic Gateway",
                &desc.semantic_gateway,
                false,
            ),
            string_field(
                "selector_syntax",
                "Selector Syntax",
                desc.selector_syntax.as_deref().unwrap_or("<none>"),
                false,
            ),
            json_field(
                "content_kind",
                "Content Kind",
                json!(desc.content_kind),
                false,
            ),
            json_field("outputs", "Outputs", json!(desc.outputs), false),
            json_field(
                "consumer_domains",
                "Consumers",
                json!(desc.consumer_domains),
                false,
            ),
        ],
    }
}

fn provider_contract_section(descriptor: Option<&AssetFileTypeDescriptor>) -> AssetDocumentSection {
    let Some(desc) = descriptor else {
        return AssetDocumentSection {
            id: "provider_contract".to_owned(),
            title: "Provider contracts".to_owned(),
            fields: vec![string_field(
                "contract",
                "Contract",
                "unregistered type",
                false,
            )],
        };
    };
    AssetDocumentSection {
        id: "provider_contract".to_owned(),
        title: "Provider contracts".to_owned(),
        fields: vec![
            string_field(
                "handler_service",
                "Handler Service",
                &desc.handler_service,
                false,
            ),
            string_field("read_method", "Read Method", &desc.read_method, false),
            string_field(
                "inspect_contract",
                "Inspect Contract",
                &desc.inspect_contract,
                false,
            ),
            string_field("edit_contract", "Edit Contract", &desc.edit_contract, false),
            json_field(
                "preview_provider",
                "Preview Provider",
                json!(desc.preview_provider),
                false,
            ),
            json_field(
                "runtime_ready",
                "Runtime Ready",
                json!(desc.runtime_ready),
                false,
            ),
        ],
    }
}

fn editable_schema_section(
    title: &str,
    descriptor: Option<&AssetFileTypeDescriptor>,
    can_apply_patch: bool,
) -> AssetDocumentSection {
    let document_kind = descriptor
        .map(|d| d.asset_kind.as_str())
        .unwrap_or("asset_document");
    let mut fields = vec![
        schema_field(
            "display_name",
            "Display Name",
            json!(title),
            SchemaValueKindV1::String,
            true,
        ),
        schema_field(
            "editor_tags",
            "Editor Tags",
            json!([]),
            SchemaValueKindV1::StringList,
            true,
        ),
        schema_field(
            "import_notes",
            "Import Notes",
            json!(""),
            SchemaValueKindV1::String,
            true,
        ),
        string_field("document_kind", "Document Kind", document_kind, false),
        json_field(
            "save_enabled",
            "Save Enabled",
            json!(can_apply_patch),
            false,
        ),
    ];
    if !can_apply_patch {
        fields.push(string_field(
            "save_blocked_by",
            "Save Blocked By",
            "missing format/package writer capability",
            false,
        ));
    }
    AssetDocumentSection {
        id: "editable_schema".to_owned(),
        title: "Editable schema".to_owned(),
        fields,
    }
}

fn edit_policy_section(descriptor: Option<&AssetFileTypeDescriptor>) -> AssetDocumentSection {
    let schema_editable = descriptor
        .map(|d| d.schema_editable || d.editable || !d.edit_contract.trim().is_empty())
        .unwrap_or(false);
    let can_apply_patch = descriptor
        .map(|d| d.write_back_available && !d.writer_capability.trim().is_empty())
        .unwrap_or(false);
    let writer_capability = descriptor
        .map(|d| d.writer_capability.as_str())
        .unwrap_or("");
    AssetDocumentSection {
        id: "edit_policy".to_owned(),
        title: "Edit policy".to_owned(),
        fields: vec![
            json_field(
                "schema_editable",
                "Schema Editable",
                json!(schema_editable),
                false,
            ),
            json_field(
                "can_apply_patch",
                "Can Apply Patch",
                json!(can_apply_patch),
                false,
            ),
            string_field(
                "patch_route",
                "Patch Route",
                ENGINE_ASSETS_EDIT_SERVICE_ID,
                false,
            ),
            string_field(
                "writer_capability",
                "Writer Capability",
                if writer_capability.is_empty() {
                    "missing"
                } else {
                    writer_capability
                },
                false,
            ),
            string_field(
                "write_owner",
                "Write Owner",
                if can_apply_patch {
                    "format/package writer provider"
                } else {
                    "missing format writer provider"
                },
                false,
            ),
        ],
    }
}

fn string_field(id: &str, label: &str, value: &str, editable: bool) -> AssetDocumentField {
    schema_field(id, label, json!(value), SchemaValueKindV1::String, editable)
}

fn json_field(id: &str, label: &str, value: Value, editable: bool) -> AssetDocumentField {
    schema_field(id, label, value, SchemaValueKindV1::Json, editable)
}

fn schema_field(
    id: &str,
    label: &str,
    value: Value,
    value_kind: SchemaValueKindV1,
    editable: bool,
) -> AssetDocumentField {
    AssetDocumentField {
        id: id.to_owned(),
        label: label.to_owned(),
        value_kind: value_kind.as_str().to_owned(),
        value: value.clone(),
        editable,
        schema_property: Some(schema_property(id, label, value_kind, value, editable)),
        ..AssetDocumentField::default()
    }
}

fn schema_property(
    id: &str,
    label: &str,
    value_kind: SchemaValueKindV1,
    value: Value,
    editable: bool,
) -> SchemaPropertyDescriptorV1 {
    let mut property = if editable {
        SchemaPropertyDescriptorV1::editable(id, label, value_kind, value)
    } else {
        SchemaPropertyDescriptorV1::readonly(id, label, value_kind, value)
    };
    property.json_pointer = format!("/properties/{id}");
    property.source_domain = "engine.assets.inspect".to_owned();
    property.tags.push(if editable {
        "editable".to_owned()
    } else {
        "readonly".to_owned()
    });
    property
}

fn asset_document_schema_type(document: &AssetDocument) -> SchemaTypeDescriptorV1 {
    let mut properties = Vec::new();
    for section in &document.sections {
        for field in &section.fields {
            if let Some(mut property) = field.schema_property.clone() {
                property.tags.push(format!("section:{}", section.id));
                properties.push(property);
            }
        }
    }
    SchemaTypeDescriptorV1 {
        type_id: format!(
            "newengine.assets.document.{}",
            document.asset_kind.replace([' ', '/'], "_")
        ),
        display_name: document.title.clone(),
        domain: "engine.assets.inspect".to_owned(),
        kind: document.document_kind.clone(),
        resource_ref: Some(document.asset_ref.clone()),
        properties,
        capabilities: [document.writer_capability.clone()]
            .into_iter()
            .filter(|cap| !cap.trim().is_empty())
            .collect(),
        tags: vec!["asset-document".to_owned(), document.asset_kind.clone()],
        ..SchemaTypeDescriptorV1::default()
    }
}

fn asset_action_input_schema(
    type_id: &str,
    display_name: &str,
    properties: Vec<SchemaPropertyDescriptorV1>,
) -> SchemaTypeDescriptorV1 {
    SchemaTypeDescriptorV1 {
        type_id: type_id.to_owned(),
        display_name: display_name.to_owned(),
        domain: "engine.assets.edit".to_owned(),
        kind: "action_input".to_owned(),
        properties,
        tags: vec!["asset-action-input".to_owned()],
        ..SchemaTypeDescriptorV1::default()
    }
}

fn schema_patch_for_document(
    document: &AssetDocument,
    target_ref: &str,
    reason: &str,
    operations: Vec<SchemaPatchOperationV1>,
) -> SchemaPatchDtoV1 {
    SchemaPatchDtoV1 {
        target_type: document
            .schema_type
            .as_ref()
            .map(|schema| schema.type_id.clone())
            .unwrap_or_else(|| document.document_kind.clone()),
        target_ref: target_ref.to_owned(),
        requester: "ui.assets.catalog".to_owned(),
        transaction_id: format!("{}:{}", reason, target_ref),
        operations,
        ..SchemaPatchDtoV1::default()
    }
}

fn schema_transaction_for_document(
    document: &AssetDocument,
    target_ref: &str,
    reason: &str,
    operations: Vec<SchemaPatchOperationV1>,
    undo_operations: Vec<SchemaPatchOperationV1>,
) -> SchemaTransactionDtoV1 {
    SchemaTransactionDtoV1 {
        transaction_id: format!("{}:{}", reason, target_ref),
        target_type: document
            .schema_type
            .as_ref()
            .map(|schema| schema.type_id.clone())
            .unwrap_or_else(|| document.document_kind.clone()),
        target_ref: target_ref.to_owned(),
        requester: "ui.assets.catalog".to_owned(),
        reason: reason.to_owned(),
        operations,
        undo_operations,
        ..SchemaTransactionDtoV1::default()
    }
}

fn icon_for_descriptor(
    descriptor: Option<&AssetFileTypeDescriptor>,
    extension: &str,
) -> &'static str {
    match descriptor
        .map(|d| d.asset_kind.as_str())
        .unwrap_or(extension)
    {
        "texture_dictionary" => "textures/ui/icons/assetBrowser.ytd@texture",
        "material_library" => "textures/ui/icons/assetBrowser.ytd@material",
        "drawable_dictionary" | "drawable" => "textures/ui/icons/assetBrowser.ytd@model",
        "archetype_dictionary" | "map_data" => "textures/ui/icons/assetBrowser.ytd@world",
        "asset_package" => "textures/ui/icons/assetBrowser.ytd@package",
        "ui_dictionary" => "textures/ui/icons/assetBrowser.ytd@ui",
        "font_dictionary" => "textures/ui/icons/assetBrowser.ytd@ui",
        "script_module" => "textures/ui/icons/assetBrowser.ytd@script",
        _ => "textures/ui/icons/assetBrowser.ytd@generic",
    }
}

fn normalize_asset_ref(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out.trim_start_matches('/').to_owned()
}

fn split_entry_ref(asset_ref: &str) -> (String, Option<String>) {
    let mut parts = asset_ref.splitn(2, '@');
    let path = parts.next().unwrap_or_default().to_owned();
    let entry = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    (path, entry)
}

fn path_extension(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

fn file_name(path: &str) -> String {
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn inspect_service_info() -> AssetInspectServiceInfo {
    AssetInspectServiceInfo {
        id: ASSETS_INSPECT_SERVICE_ID,
        gateway: ENGINE_ASSETS_INSPECT_SERVICE_ID,
        methods: ASSETS_INSPECT_SERVICE_METHODS,
        backend: "engine.assets.starvault.asset-document-inspect",
        policy: "schema-driven DTO; Asset Browser/UI does not parse file formats",
    }
}

fn edit_service_info() -> AssetEditServiceInfo {
    AssetEditServiceInfo {
        id: ASSETS_EDIT_SERVICE_ID,
        gateway: ENGINE_ASSETS_EDIT_SERVICE_ID,
        methods: ASSETS_EDIT_SERVICE_METHODS,
        backend: "engine.assets.starvault.asset-document-edit",
        policy: "validates patch DTOs; write-back requires explicit provider edit_contract/package writer capability",
    }
}

#[derive(Deserialize)]
struct InvokeEnvelope {
    method: String,
    #[serde(default)]
    request: Value,
}

fn inspect_invoke_json(state: &mut AssetInspectState, payload: Blob) -> RResult<Blob, RString> {
    let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
        Ok(envelope) => envelope,
        Err(e) => {
            return RResult::RErr(RString::from(format!(
                "assets.inspect: invalid invoke_json payload: {e}"
            )))
        }
    };
    match envelope.method.as_str() {
        asset_inspect_method::INSPECT_DOCUMENT_JSON_V1
        | asset_inspect_method::PREVIEW_JSON_V1
        | asset_inspect_method::VALIDATE_REF_JSON_V1 => {
            let request = match serde_json::from_value::<AssetDocumentRequest>(envelope.request) {
                Ok(request) => request,
                Err(e) => {
                    return RResult::RErr(RString::from(format!(
                        "assets.inspect: invalid document request: {e}"
                    )))
                }
            };
            ok_json(state.inspect_document(request))
        }
        other => RResult::RErr(RString::from(format!(
            "assets.inspect: unknown invoke method '{other}'"
        ))),
    }
}

fn edit_invoke_json(state: &mut AssetEditState, payload: Blob) -> RResult<Blob, RString> {
    let envelope = match serde_json::from_slice::<InvokeEnvelope>(payload.as_slice()) {
        Ok(envelope) => envelope,
        Err(e) => {
            return RResult::RErr(RString::from(format!(
                "assets.edit: invalid invoke_json payload: {e}"
            )))
        }
    };
    match envelope.method.as_str() {
        asset_edit_method::VALIDATE_PATCH_JSON_V1 => {
            let patch = match serde_json::from_value::<AssetPatch>(envelope.request) {
                Ok(patch) => patch,
                Err(e) => {
                    return RResult::RErr(RString::from(format!("assets.edit: invalid patch: {e}")))
                }
            };
            ok_json(state.validate_patch(patch))
        }
        asset_edit_method::APPLY_PATCH_JSON_V1 => {
            let patch = match serde_json::from_value::<AssetPatch>(envelope.request) {
                Ok(patch) => patch,
                Err(e) => {
                    return RResult::RErr(RString::from(format!("assets.edit: invalid patch: {e}")))
                }
            };
            ok_json(state.apply_patch(patch))
        }
        asset_edit_method::DIRTY_STATE_JSON_V1 => ok_json(AssetPatchResult {
            accepted: true,
            diagnostics: vec![AssetDocumentDiagnostic::info(
                "dirty_state.generic",
                "generic edit provider has no local dirty cache",
            )],
            ..AssetPatchResult::default()
        }),
        other => RResult::RErr(RString::from(format!(
            "assets.edit: unknown invoke method '{other}'"
        ))),
    }
}

pub fn asset_document_inspect_gateway_service(
    host: HostApiV1,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSETS_INSPECT_SERVICE_ID,
        "newengine-assets.document-inspect",
        ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
        ASSETS_INSPECT_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_INSPECT_SERVICE_ID)
    .protocol("json")
    .features(["schema-driven-asset-document", "provider-routed-inspection", "ui-agnostic"])
    .notes("Asset Browser requests AssetDocument DTOs here. Format parsing belongs to provider/domain contracts, not to UI.");

    JsonServiceRouter::with_state(ASSETS_INSPECT_SERVICE_ID, AssetInspectState::new(host))
        .describe_json(&description)
        .info(inspect_service_info)
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::INSPECT_DOCUMENT_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::PREVIEW_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .post_json::<AssetDocumentRequest, AssetDocument, _>(
            asset_inspect_method::VALIDATE_REF_JSON_V1,
            |state, request| state.inspect_document(request),
        )
        .blob(asset_inspect_method::INVOKE_JSON, inspect_invoke_json)
        .blob(asset_inspect_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn asset_document_edit_gateway_service(
    host: HostApiV1,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        ASSETS_EDIT_SERVICE_ID,
        "newengine-assets.document-edit",
        ASSETS_EDIT_BACKEND_CAPABILITY_ID,
        ASSETS_EDIT_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_EDIT_SERVICE_ID)
    .protocol("json")
    .features(["asset-patch-dto", "provider-validated-writeback", "explicit-writer-capability"])
    .notes("Generic edit route validates patch transport. Real write-back is owned by format/package writer providers.");

    JsonServiceRouter::with_state(ASSETS_EDIT_SERVICE_ID, AssetEditState::new(host))
        .describe_json(&description)
        .info(edit_service_info)
        .post_json::<AssetPatch, AssetPatchResult, _>(
            asset_edit_method::VALIDATE_PATCH_JSON_V1,
            |state, patch| state.validate_patch(patch),
        )
        .post_json::<AssetPatch, AssetPatchResult, _>(
            asset_edit_method::APPLY_PATCH_JSON_V1,
            |state, patch| state.apply_patch(patch),
        )
        .post_json::<Value, AssetPatchResult, _>(
            asset_edit_method::DIRTY_STATE_JSON_V1,
            |_state, _payload| AssetPatchResult {
                accepted: true,
                diagnostics: vec![AssetDocumentDiagnostic::info(
                    "dirty_state.generic",
                    "generic edit provider has no local dirty cache",
                )],
                ..AssetPatchResult::default()
            },
        )
        .blob(asset_edit_method::INVOKE_JSON, edit_invoke_json)
        .blob(asset_edit_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

pub fn register_asset_document_gateways_best_effort(host: HostApiV1) -> bool {
    let inspect_ok = register_engine_gateway_provider_service_dynamic_best_effort(
        EngineGatewayProviderDeclDynamic {
            gateway: ENGINE_ASSETS_INSPECT_SERVICE_ID,
            service_kind: "assets.inspect",
            provider_service: ASSETS_INSPECT_SERVICE_ID,
            provider_route: "engine.assets.starvault.document_inspect",
            capability: ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-assets.document-inspect",
            service: asset_document_inspect_gateway_service(host.clone()),
        },
    );
    let edit_ok = register_engine_gateway_provider_service_dynamic_best_effort(
        EngineGatewayProviderDeclDynamic {
            gateway: ENGINE_ASSETS_EDIT_SERVICE_ID,
            service_kind: "assets.edit",
            provider_service: ASSETS_EDIT_SERVICE_ID,
            provider_route: "engine.assets.starvault.document_edit",
            capability: ASSETS_EDIT_BACKEND_CAPABILITY_ID,
            priority: 0,
            owner: "newengine-assets.document-edit",
            service: asset_document_edit_gateway_service(host),
        },
    );
    inspect_ok && edit_ok
}
