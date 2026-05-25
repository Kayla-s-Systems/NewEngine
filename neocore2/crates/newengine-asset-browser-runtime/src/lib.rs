#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned `engine.assets.browser` read-model/control surface.
//!
//! Asset Browser is not a new byte owner and not a semantic parser. It composes:
//! - mounted VFS directory listings from AssetManager;
//! - provider-declared FileTypeRegistry descriptors;
//! - common ListFile manifests for `file@entry` dictionary browsing.

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetService, AssetServiceClient};
use newengine_assets_api::{
    asset_browser_method, file_type_method, AssetBrowserEntryMutationRequest,
    AssetBrowserListRequest, AssetBrowserListResponse, AssetBrowserLocation,
    AssetBrowserMutationResponse, AssetBrowserNode, AssetBrowserOpenRequest,
    AssetBrowserRebuildRequest, AssetBrowserSnapshotResponse, AssetDecodeRequest,
    AssetEntryManifest, AssetFileManifest, AssetFileTypeDescriptor, AssetFileTypeManifest,
    ASSET_BROWSER_BACKEND_CAPABILITY_ID, ASSET_BROWSER_RUNTIME_CONTRACT,
    ASSET_BROWSER_SERVICE_ID, ASSET_BROWSER_SERVICE_METHODS, ASSET_LIST_FILE_MANIFEST_OUTPUT,
    ENGINE_ASSET_FILE_TYPES_SERVICE_ID, ENGINE_ASSETS_BROWSER_SERVICE_ID,
};
use newengine_plugin_api::{Blob, HostApiV1, MethodName};
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_dynamic_best_effort, EngineOwnedGatewayDeclDynamic,
    JsonServiceRouter,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub const ASSET_BROWSER_GATEWAY_OWNER: &str = "newengine-asset-browser-runtime.engine-owned-provider";

#[derive(Clone)]
pub struct AssetBrowserRuntimeState {
    host: HostApiV1,
    client: AssetServiceClient,
}

impl AssetBrowserRuntimeState {
    #[inline]
    pub fn new(host: HostApiV1, client: AssetServiceClient) -> Self { Self { host, client } }
}

#[derive(Clone, Debug, Serialize)]
pub struct AssetBrowserServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub byte_owner: &'static str,
    pub descriptor_owner: &'static str,
    pub methods: &'static [&'static str],
    pub policy: &'static str,
}

pub fn asset_browser_service_info() -> AssetBrowserServiceInfo {
    AssetBrowserServiceInfo {
        id: ASSET_BROWSER_SERVICE_ID,
        gateway: ENGINE_ASSETS_BROWSER_SERVICE_ID,
        provider: "EngineOwnedAssetBrowserProvider",
        contract: ASSET_BROWSER_RUNTIME_CONTRACT,
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
        descriptor_owner: ENGINE_ASSET_FILE_TYPES_SERVICE_ID,
        methods: ASSET_BROWSER_SERVICE_METHODS,
        policy: "Asset Browser composes VFS listings, FileTypeRegistry descriptors and ListFile manifests; domain semantics remain behind engine.* gateways.",
    }
}

pub fn asset_browser_gateway_service(
    host: HostApiV1,
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        ASSET_BROWSER_SERVICE_ID,
        ASSET_BROWSER_GATEWAY_OWNER,
        ASSET_BROWSER_BACKEND_CAPABILITY_ID,
        ASSET_BROWSER_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_BROWSER_SERVICE_ID)
    .protocol(ASSET_BROWSER_RUNTIME_CONTRACT)
    .features([
        "vfs-directory-listing",
        "file-type-descriptor-projection",
        "listfile-open-as-directory",
        "nef8-listfile-entry-repack",
        "writable-vfs-write-back",
    ])
    .notes("Unreal-like/Explorer-like Asset Browser read-model/control plane. It composes VFS listings, FileTypeRegistry descriptors and AssetManager NEF8 ListFile repack/write-back; domain semantics remain behind engine.* gateways.");

    JsonServiceRouter::with_state(
        ASSET_BROWSER_SERVICE_ID,
        AssetBrowserRuntimeState::new(host, client),
    )
    .describe_json(&description)
    .info(asset_browser_service_info)
    .get_json_result(asset_browser_method::SNAPSHOT_V1, snapshot)
    .get_json_result("engine.assets.browser.snapshot", snapshot)
    .post_json_result::<AssetBrowserListRequest, AssetBrowserListResponse, _>(asset_browser_method::LIST_V1, list_vfs)
    .post_json_result::<AssetBrowserListRequest, AssetBrowserListResponse, _>("engine.assets.browser.list", list_vfs)
    .post_json_result::<AssetBrowserOpenRequest, AssetBrowserListResponse, _>(asset_browser_method::OPEN_V1, open_target)
    .post_json_result::<AssetBrowserOpenRequest, AssetBrowserListResponse, _>("engine.assets.browser.open", open_target)
    .post_json_result::<AssetBrowserListRequest, AssetBrowserListResponse, _>(asset_browser_method::REFRESH_V1, list_vfs)
    .post_json::<AssetBrowserEntryMutationRequest, AssetBrowserMutationResponse, _>(asset_browser_method::ENTRY_UPDATE_V1, entry_update)
    .post_json::<AssetBrowserEntryMutationRequest, AssetBrowserMutationResponse, _>(asset_browser_method::ENTRY_DELETE_V1, entry_delete)
    .post_json::<AssetBrowserRebuildRequest, AssetBrowserMutationResponse, _>(asset_browser_method::REBUILD_V1, rebuild_listfile)
    .blob(asset_browser_method::INVOKE_JSON, invoke_json)
    .blob(asset_browser_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
    .into_service_v1()
}

pub fn register_asset_browser_gateway_best_effort(
    host: HostApiV1,
    client: AssetServiceClient,
) -> bool {
    register_engine_owned_gateway_service_dynamic_best_effort(EngineOwnedGatewayDeclDynamic {
        gateway: ENGINE_ASSETS_BROWSER_SERVICE_ID,
        service_kind: "assets.browser",
        provider_service: ASSET_BROWSER_SERVICE_ID,
        capability: ASSET_BROWSER_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ASSET_BROWSER_GATEWAY_OWNER,
        service: asset_browser_gateway_service(host, client),
    })
}

fn invoke_json(state: &mut AssetBrowserRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or(asset_browser_method::SNAPSHOT_V1);
    let request_value = value.get("request").cloned().unwrap_or_default();
    match method {
        asset_browser_method::SNAPSHOT_V1 => match snapshot(state) {
            Ok(response) => ok_json(response),
            Err(e) => RResult::RErr(RString::from(e)),
        },
        asset_browser_method::LIST_V1 | asset_browser_method::REFRESH_V1 => {
            let request = serde_json::from_value::<AssetBrowserListRequest>(request_value).unwrap_or_default();
            match list_vfs(state, request) {
                Ok(response) => ok_json(response),
                Err(e) => RResult::RErr(RString::from(e)),
            }
        }
        asset_browser_method::OPEN_V1 => {
            let request = serde_json::from_value::<AssetBrowserOpenRequest>(request_value).unwrap_or_default();
            match open_target(state, request) {
                Ok(response) => ok_json(response),
                Err(e) => RResult::RErr(RString::from(e)),
            }
        }
        asset_browser_method::ENTRY_UPDATE_V1 => {
            let request = serde_json::from_value::<AssetBrowserEntryMutationRequest>(request_value).unwrap_or_default();
            ok_json(entry_update(state, request))
        }
        asset_browser_method::ENTRY_DELETE_V1 => {
            let request = serde_json::from_value::<AssetBrowserEntryMutationRequest>(request_value).unwrap_or_default();
            ok_json(entry_delete(state, request))
        }
        asset_browser_method::REBUILD_V1 => {
            let request = serde_json::from_value::<AssetBrowserRebuildRequest>(request_value).unwrap_or_default();
            ok_json(rebuild_listfile(state, request))
        }
        other => RResult::RErr(RString::from(format!("engine.assets.browser: unknown invoke_json method '{other}'"))),
    }
}

fn snapshot(state: &mut AssetBrowserRuntimeState) -> Result<AssetBrowserSnapshotResponse, String> {
    let mut root = list_vfs(state, AssetBrowserListRequest::default())?;
    root.sources = sources_array(state);
    let (file_type_manifest, file_type_warnings) = file_type_manifest_value(state);
    let (formats, format_warnings) = formats_value(state);
    let sources = root.sources.clone();
    let mut warnings = Vec::new();
    warnings.extend(file_type_warnings);
    warnings.extend(format_warnings);
    Ok(AssetBrowserSnapshotResponse {
        ok: true,
        root,
        sources,
        file_type_manifest,
        formats,
        warnings,
        ..Default::default()
    })
}

fn list_vfs(
    state: &mut AssetBrowserRuntimeState,
    request: AssetBrowserListRequest,
) -> Result<AssetBrowserListResponse, String> {
    if let Some(entry) = request.entry.as_deref().map(str::trim).filter(|it| !it.is_empty()) {
        return open_entry(state, &request.logical_path, entry);
    }

    let logical_path = normalize_path(&request.logical_path);
    let listing = state.client.vfs_list_json_v1(&logical_path)?;
    let descriptors = descriptor_map(state);
    let sources = sources_array(state);
    let mut response = AssetBrowserListResponse {
        ok: true,
        location: AssetBrowserLocation { logical_path: logical_path.clone(), entry: None, location_kind: "vfs_directory".to_owned() },
        breadcrumbs: breadcrumbs(&logical_path),
        sources,
        warnings: value_warnings(&listing),
        ..Default::default()
    };

    for entry in listing.get("entries").and_then(|v| v.as_array()).into_iter().flatten() {
        let mut node = node_from_vfs_entry(entry, &descriptors);
        if !request.include_hidden && node.name.starts_with('.') {
            continue;
        }
        if !request.query.trim().is_empty() && !node.name.to_ascii_lowercase().contains(&request.query.to_ascii_lowercase()) {
            continue;
        }
        if node.node_kind == "directory" {
            response.folders.push(node);
        } else {
            annotate_listfile_entry_count(state, &mut node, request.include_listfile_entries);
            response.assets.push(node);
        }
    }
    Ok(response)
}

fn open_target(
    state: &mut AssetBrowserRuntimeState,
    request: AssetBrowserOpenRequest,
) -> Result<AssetBrowserListResponse, String> {
    let target = normalize_path(&request.target_ref);
    let location = AssetBrowserLocation::from_ref(&target);
    if let Some(entry) = location.entry.as_deref() {
        return open_entry(state, &location.logical_path, entry);
    }

    let descriptors = descriptor_map(state);
    let descriptor = extension_of(&location.logical_path).and_then(|ext| descriptors.get(&ext).cloned());
    let mode = request.mode.trim();
    let should_open_as_listfile = mode == "listFile"
        || mode == "listfile"
        || descriptor.as_ref().map(is_listfile_descriptor).unwrap_or(false);

    if should_open_as_listfile {
        match open_listfile_manifest(state, &location.logical_path) {
            Ok(response) => return Ok(response),
            Err(e) if mode.eq_ignore_ascii_case("auto") || mode.is_empty() => {
                let mut request = AssetBrowserListRequest::default();
                request.logical_path = location.logical_path;
                let mut response = list_vfs(state, request)?;
                response.warnings.push(format!("open as ListFile failed: {e}"));
                return Ok(response);
            }
            Err(e) => return Err(e),
        }
    }

    let mut request = AssetBrowserListRequest::default();
    request.logical_path = location.logical_path;
    list_vfs(state, request)
}

fn open_entry(
    state: &mut AssetBrowserRuntimeState,
    logical_path: &str,
    entry: &str,
) -> Result<AssetBrowserListResponse, String> {
    let parent = open_listfile_manifest(state, logical_path)?;
    let entry_ref = format!("{}@{}", normalize_path(logical_path), entry.trim());
    let mut filtered = parent;
    filtered.location = AssetBrowserLocation { logical_path: normalize_path(logical_path), entry: Some(entry.trim().to_owned()), location_kind: "listfile_entry".to_owned() };
    filtered.entries.retain(|node| node.entry_ref.as_deref() == Some(entry_ref.as_str()) || node.name == entry.trim());
    if filtered.entries.is_empty() {
        filtered.warnings.push(format!("ListFile entry not found: {entry_ref}"));
    }
    Ok(filtered)
}

fn open_listfile_manifest(
    state: &mut AssetBrowserRuntimeState,
    logical_path: &str,
) -> Result<AssetBrowserListResponse, String> {
    let logical_path = normalize_path(logical_path);
    let request = AssetDecodeRequest {
        logical_path: logical_path.clone(),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: Value::Null,
    };
    let bytes = state.client.decode_v1(&request)?;
    let manifest: AssetFileManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("ListFile manifest parse failed for '{}': {e}", logical_path))?;
    Ok(response_from_manifest(state, logical_path, manifest))
}

fn response_from_manifest(
    state: &mut AssetBrowserRuntimeState,
    logical_path: String,
    manifest: AssetFileManifest,
) -> AssetBrowserListResponse {
    let descriptors = descriptor_map(state);
    let mut response = AssetBrowserListResponse {
        ok: true,
        location: AssetBrowserLocation { logical_path: logical_path.clone(), entry: None, location_kind: "listfile".to_owned() },
        breadcrumbs: breadcrumbs(&logical_path),
        sources: sources_array(state),
        warnings: manifest.warnings.clone(),
        ..Default::default()
    };
    if let Some(desc) = extension_of(&logical_path).and_then(|ext| descriptors.get(&ext).cloned()) {
        response.assets.push(node_from_descriptor_file(&logical_path, &desc));
    }
    for entry in manifest.entries {
        response.entries.push(node_from_manifest_entry(&logical_path, entry));
    }
    response
}

fn node_from_vfs_entry(
    value: &Value,
    descriptors: &BTreeMap<String, AssetFileTypeDescriptor>,
) -> AssetBrowserNode {
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let logical_path = value.get("path").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    let kind = value.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
    let mut node = AssetBrowserNode {
        name,
        logical_path: logical_path.clone(),
        node_kind: if kind == "directory" { "directory".to_owned() } else { "asset".to_owned() },
        asset_kind: if kind == "directory" { "folder".to_owned() } else { "asset".to_owned() },
        byte_len: value.get("byte_len").and_then(|v| v.as_u64()),
        source_kind: value.get("source_kind").and_then(|v| v.as_str()).map(str::to_owned),
        source_index: value.get("source_index").and_then(|v| v.as_u64()).map(|v| v as usize),
        mount: value.get("mount").and_then(|v| v.as_str()).map(str::to_owned),
        priority: value.get("priority").and_then(|v| v.as_i64()).map(|v| v as i32),
        has_children: kind == "directory",
        can_preview: kind != "directory",
        can_rebuild: false,
        ..Default::default()
    };
    if let Some(ext) = extension_of(&logical_path) {
        node.extension = Some(ext.clone());
        if let Some(desc) = descriptors.get(&ext) {
            apply_descriptor(&mut node, desc);
        }
    }
    node
}

fn node_from_descriptor_file(logical_path: &str, desc: &AssetFileTypeDescriptor) -> AssetBrowserNode {
    let name = logical_path.rsplit('/').next().unwrap_or(logical_path).to_owned();
    let mut node = AssetBrowserNode {
        name,
        logical_path: logical_path.to_owned(),
        node_kind: "asset".to_owned(),
        asset_kind: desc.asset_kind.clone(),
        extension: Some(desc.extension.clone()),
        semantic_gateway: Some(desc.semantic_gateway.clone()),
        handler_service: Some(desc.handler_service.clone()),
        has_children: is_listfile_descriptor(desc) || desc.allow_nested_assets,
        can_open: true,
        can_preview: true,
        can_rebuild: is_listfile_descriptor(desc),
        ..Default::default()
    };
    node.metadata.insert("container".to_owned(), desc.container.clone());
    node.metadata.insert("codec_type".to_owned(), desc.codec_type.clone());
    node
}

fn node_from_manifest_entry(logical_path: &str, entry: AssetEntryManifest) -> AssetBrowserNode {
    let entry_ref = if entry.entry_ref.trim().is_empty() {
        format!("{}@{}", logical_path, entry.name)
    } else {
        entry.entry_ref.clone()
    };
    let mut metadata = entry.metadata.clone();
    if !entry.stable_id.trim().is_empty() {
        metadata.insert("stable_id".to_owned(), entry.stable_id.clone());
    }
    if !entry.dependencies.is_empty() {
        metadata.insert("dependency_count".to_owned(), entry.dependencies.len().to_string());
    }
    AssetBrowserNode {
        name: entry.name,
        logical_path: logical_path.to_owned(),
        entry_ref: Some(entry_ref),
        node_kind: "listfile_entry".to_owned(),
        asset_kind: entry.asset_kind,
        route_gateway: Some(entry.route.gateway),
        route_method: Some(entry.route.method),
        semantic_gateway: Some(entry.route.semantic_owner),
        has_children: false,
        can_open: true,
        can_preview: true,
        can_rename: true,
        can_delete: true,
        can_update: true,
        can_rebuild: true,
        metadata,
        warnings: vec!["entry mutation routes to AssetManager NEF8 ListFile repack/write-back; read-only container-backed sources reject destructive writes until a package writer is active".to_owned()],
        ..Default::default()
    }
}

fn apply_descriptor(node: &mut AssetBrowserNode, desc: &AssetFileTypeDescriptor) {
    node.asset_kind = desc.asset_kind.clone();
    node.semantic_gateway = Some(desc.semantic_gateway.clone());
    node.handler_service = Some(desc.handler_service.clone());
    node.has_children = node.has_children || is_listfile_descriptor(desc) || desc.allow_nested_assets;
    node.can_rebuild = is_listfile_descriptor(desc);
    node.metadata.insert("container".to_owned(), desc.container.clone());
    node.metadata.insert("codec_type".to_owned(), desc.codec_type.clone());
    if let Some(selector) = desc.selector_syntax.as_ref() {
        node.metadata.insert("selector_syntax".to_owned(), selector.clone());
    }
}

fn is_listfile_descriptor(desc: &AssetFileTypeDescriptor) -> bool {
    desc.codec_type == newengine_assets_api::codec_type::LIST_FILE
        || desc.codec_type == newengine_assets_api::codec_type::LIST
        || desc.selector_syntax.as_deref().map(|it| it.contains('@')).unwrap_or(false)
}

fn annotate_listfile_entry_count(
    state: &mut AssetBrowserRuntimeState,
    node: &mut AssetBrowserNode,
    include_entries: bool,
) {
    if !include_entries || !node.has_children || node.node_kind == "directory" {
        return;
    }
    if let Ok(manifest) = decode_listfile_manifest_value(state, &node.logical_path) {
        let count = manifest.entries.len();
        node.metadata.insert("entry_count".to_owned(), count.to_string());
    }
}

fn decode_listfile_manifest_value(
    state: &mut AssetBrowserRuntimeState,
    logical_path: &str,
) -> Result<AssetFileManifest, String> {
    let request = AssetDecodeRequest {
        logical_path: normalize_path(logical_path),
        output_kind: ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
        selector: Value::Null,
    };
    let bytes = state.client.decode_v1(&request)?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn descriptor_map(state: &mut AssetBrowserRuntimeState) -> BTreeMap<String, AssetFileTypeDescriptor> {
    let (manifest, _) = file_type_manifest_value(state);
    serde_json::from_value::<AssetFileTypeManifest>(manifest)
        .map(|manifest| {
            manifest
                .formats
                .into_iter()
                .map(|desc| (desc.extension.clone(), desc))
                .collect()
        })
        .unwrap_or_default()
}

fn file_type_manifest_value(state: &mut AssetBrowserRuntimeState) -> (Value, Vec<String>) {
    let result = (state.host.call_service_v1)(
        RString::from(ENGINE_ASSET_FILE_TYPES_SERVICE_ID),
        MethodName::from(file_type_method::MANIFEST_JSON_V1),
        Blob::from(Vec::<u8>::new()),
    );
    match result.into_result() {
        Ok(blob) => match serde_json::from_slice::<Value>(blob.as_slice()) {
            Ok(value) => (value, Vec::new()),
            Err(e) => (Value::Null, vec![format!("file type manifest parse failed: {e}")]),
        },
        Err(e) => (Value::Null, vec![format!("file type manifest unavailable: {e}")]),
    }
}

fn formats_value(state: &mut AssetBrowserRuntimeState) -> (Value, Vec<String>) {
    match state.client.formats_json_v1() {
        Ok(value) => (value, Vec::new()),
        Err(e) => (Value::Null, vec![format!("asset formats unavailable: {e}")]),
    }
}

fn sources_array(state: &mut AssetBrowserRuntimeState) -> Vec<Value> {
    state
        .client
        .sources_json_v1()
        .ok()
        .and_then(|value| value.get("sources").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default()
}

fn value_warnings(value: &Value) -> Vec<String> {
    value
        .get("warnings")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))
        .collect()
}

fn entry_update(state: &mut AssetBrowserRuntimeState, mut request: AssetBrowserEntryMutationRequest) -> AssetBrowserMutationResponse {
    if request.operation.trim().is_empty() {
        request.operation = "update".to_owned();
    }
    repack_via_asset_manager(state, serde_json::to_value(request).unwrap_or_default(), "update")
}

fn entry_delete(state: &mut AssetBrowserRuntimeState, mut request: AssetBrowserEntryMutationRequest) -> AssetBrowserMutationResponse {
    if request.operation.trim().is_empty() {
        request.operation = "delete".to_owned();
    }
    repack_via_asset_manager(state, serde_json::to_value(request).unwrap_or_default(), "delete")
}

fn rebuild_listfile(state: &mut AssetBrowserRuntimeState, request: AssetBrowserRebuildRequest) -> AssetBrowserMutationResponse {
    let mut value = serde_json::to_value(request).unwrap_or_default();
    if let Some(obj) = value.as_object_mut() {
        obj.insert("operation".to_owned(), Value::String("rebuild".to_owned()));
    }
    repack_via_asset_manager(state, value, "rebuild")
}

fn repack_via_asset_manager(
    state: &mut AssetBrowserRuntimeState,
    payload: Value,
    default_operation: &str,
) -> AssetBrowserMutationResponse {
    match state.client.list_file_repack_json_v1(payload.clone()) {
        Ok(value) => serde_json::from_value::<AssetBrowserMutationResponse>(value).unwrap_or_else(|e| {
            mutation_error_response(
                payload_target_ref(&payload),
                default_operation,
                format!("AssetManager repack response did not match AssetBrowserMutationResponse: {e}"),
            )
        }),
        Err(e) => mutation_error_response(payload_target_ref(&payload), default_operation, e),
    }
}

fn mutation_error_response(target_ref: String, operation: &str, error: String) -> AssetBrowserMutationResponse {
    let target_ref = normalize_path(&target_ref);
    AssetBrowserMutationResponse {
        ok: false,
        accepted: true,
        applied: false,
        target_ref: target_ref.clone(),
        logical_path: target_ref.split('@').next().unwrap_or_default().to_owned(),
        entry: target_ref.split_once('@').map(|(_, entry)| entry.to_owned()),
        operation: operation.to_owned(),
        transaction_id: format!("asset-browser-repack-error:{}", stable_hash(&target_ref)),
        message: error,
        warnings: vec!["Asset Browser did not mutate local UI state; source bytes remain authoritative in AssetManager VFS.".to_owned()],
        ..Default::default()
    }
}

fn payload_target_ref(value: &Value) -> String {
    value
        .get("target_ref")
        .or_else(|| value.get("logical_path"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn breadcrumbs(path: &str) -> Vec<AssetBrowserLocation> {
    let path = normalize_path(path);
    let mut out = vec![AssetBrowserLocation::default()];
    let mut cursor = String::new();
    for part in path.split('/').filter(|it| !it.is_empty()) {
        if !cursor.is_empty() {
            cursor.push('/');
        }
        cursor.push_str(part);
        out.push(AssetBrowserLocation { logical_path: cursor.clone(), entry: None, location_kind: "vfs_directory".to_owned() });
    }
    out
}

fn extension_of(path: &str) -> Option<String> {
    path
        .split('@')
        .next()
        .unwrap_or(path)
        .rsplit('.')
        .next()
        .map(str::trim)
        .filter(|ext| !ext.is_empty() && !ext.contains('/'))
        .map(|ext| ext.to_ascii_lowercase())
}

fn normalize_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
