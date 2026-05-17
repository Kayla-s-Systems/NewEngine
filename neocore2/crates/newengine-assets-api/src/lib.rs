#![forbid(unsafe_op_in_unsafe_fn)]

use core::time::Duration;
use std::time::Instant;

mod asset_error;
pub use asset_error::*;

/// Engine-facing asset service gateway id.
///
/// Runtime and provider plugins must request assets through this stable host-owned
/// gateway, not through a concrete AssetManager/provider service id. The host
/// resolves this gateway to the active asset backend by declared capability.
pub const ENGINE_ASSET_SERVICE_ID: &str = "engine.assets";

/// Canonical client-facing service id for asset access.
pub const ASSET_SERVICE_ID: &str = ENGINE_ASSET_SERVICE_ID;

/// Default provider service id used by the first-party AssetManager backend.
///
/// This is provider-owned identity, not the id consumers should call. Third-party
/// providers may register a different service id as long as they declare
/// `asset_manager.backend`; the engine gateway still resolves them.
pub const ASSET_PROVIDER_SERVICE_ID: &str = "asset_manager.api";

/// Backend capability declared by plugins that provide an asset service backend.
pub const ASSET_BACKEND_CAPABILITY_ID: &str = "asset_manager.backend";

/// Wire method namespace for asset-domain service calls.
pub const ASSET_METHOD_PREFIX: &str = "asset.";

/// Generic host/plugin backend declaration for the asset service family.
pub const ASSET_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets",
        ENGINE_ASSET_SERVICE_ID,
        ASSET_PROVIDER_SERVICE_ID,
        ASSET_BACKEND_CAPABILITY_ID,
    );

/// Canonical AssetManager v1 method names.
///
/// There is one supported runtime contract: explicit `*_v1` entry points for
/// import/pump/state/text/texture access. Older alias pairs such as
/// `asset.load`, `asset.pump`, and `asset.load_text_v1` are intentionally not
/// part of this surface.
pub mod method {
    /// Standard service-framework metadata method.
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    /// Standard service-framework JSON control invocation method.
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;

    pub const RELOAD_V1: &str = "asset.reload_v1";
    pub const INFO_JSON_V1: &str = "asset.info_json_v1";
    pub const STATE_JSON_V1: &str = "asset.state_json_v1";
    /// Current AssetStatus row by id or logical path. Payload accepts utf8 id_hex32 or logical path.
    pub const STATUS_JSON_V1: &str = "asset.status_json_v1";
    /// Full AssetStatus graph by id or logical path. Payload accepts utf8 id_hex32 or logical path.
    pub const STATUS_GRAPH_JSON_V1: &str = "asset.status_graph_json_v1";
    /// Validated lifecycle projection hook. Payload is JSON with owner/domain/logical_path/stage/proof.
    pub const PROJECT_STATUS_JSON_V1: &str = "asset.project_status_json_v1";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";
    /// Runtime-ready RGBA8 texture packet by asset id. AssetManager validates/parses importer metadata.
    pub const TEXTURE_RGBA8_V1: &str = "asset.texture_rgba8_v1";
    /// Runtime-ready RGBA8 texture selected from a .neytd dictionary. Payload: JSON { dictionary_path, texture_name | texture_hash }.
    pub const TEXTURE_DICTIONARY_RGBA8_V1: &str = "asset.texture_dictionary_rgba8_v1";
    /// Runtime-ready GPU-native texture selected from a .neytd dictionary.
    /// Returns NTRT v2 with format + complete mip chain. BC1/BC3/BC5/BC7 stay compressed.
    pub const TEXTURE_DICTIONARY_RUNTIME_V1: &str = "asset.texture_dictionary_runtime_v1";
    /// Explicit BCn aliases for callers that want to assert a compressed format class.
    pub const TEXTURE_BC1_V1: &str = "asset.texture_bc1_v1";
    pub const TEXTURE_BC3_V1: &str = "asset.texture_bc3_v1";
    pub const TEXTURE_BC5_V1: &str = "asset.texture_bc5_v1";
    pub const TEXTURE_BC7_V1: &str = "asset.texture_bc7_v1";

    /// Stable v1 import entry point.
    pub const IMPORT_V1: &str = "asset.import_v1";
    /// Stable v1 pump entry point.
    pub const PUMP_V1: &str = "asset.pump_v1";
    /// Raw VFS bytes by logical path. This bypasses importers but still resolves exclusively through AssetManager mounts.
    pub const RAW_BYTES_V1: &str = "asset.raw_bytes_v1";
    /// Raw UTF-8 text by logical path resolved through AssetManager mounts.
    pub const TEXT_V1: &str = "asset.text_v1";
    // Fast-path / batch APIs.
    pub const PRELOAD_MANY_V1: &str = "asset.preload_many_v1";
    pub const GET_STATE_V1: &str = "asset.get_state_v1";

    pub const FORMATS_JSON_V1: &str = "asset.formats_json_v1";
    pub const SOURCES_JSON_V1: &str = "asset.sources_json_v1";
    pub const VERIFY_ASSETS_JSON_V1: &str = "asset.verify_assets_json_v1";
    pub const SOURCE_KINDS_JSON_V1: &str = "asset.source_kinds_json_v1";
    pub const MOUNT_SOURCE_JSON_V1: &str = "asset.mount_source_json_v1";

    // Debug/diagnostics.
    pub const RESOLVE_TRACE_JSON_V1: &str = "asset.resolve_trace_json_v1";

    // Generic lifecycle hook understood by the plugin host.
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

    #[cfg(feature = "legacy_asset_api_compat")]
    pub mod legacy {
        pub const INFO_JSON: &str = "asset.info_json";
        pub const STATE_JSON: &str = "asset.state_json";
        pub const MARK_STATUS_JSON_V1: &str = "asset.mark_status_json_v1";
        pub const FORMATS_JSON: &str = "asset.formats_json";
        pub const SOURCES_JSON: &str = "asset.sources_json";
        pub const VERIFY_ASSETS_JSON: &str = "asset.verify_assets_json";
        pub const SOURCE_KINDS_JSON: &str = "asset.source_kinds_json";
        pub const MOUNT_PAK: &str = "asset.mount_pak";
        pub const MOUNT_DIR: &str = "asset.mount_dir";
        pub const MOUNT_PAK_PRIO: &str = "asset.mount_pak_prio";
        pub const MOUNT_DIR_PRIO: &str = "asset.mount_dir_prio";
        pub const MOUNT_HTTP_PRIO: &str = "asset.mount_http_prio";
        pub const RESOLVE_TRACE_JSON: &str = "asset.resolve_trace_json";
    }
}

/// Required runtime methods for AssetManager 0.6+ deployments.
///
/// The engine validates these before scene bootstrap so an old DLL cannot fail
/// later as "unknown method" inside foliage/profile loading.
pub const REQUIRED_RUNTIME_METHODS_V1: &[&str] = &[
    method::INFO_JSON,
    method::INVOKE_JSON,
    method::SHUTDOWN_V1,
    method::RAW_BYTES_V1,
    method::TEXT_V1,
    method::IMPORT_V1,
    method::TEXTURE_RGBA8_V1,
    method::TEXTURE_DICTIONARY_RGBA8_V1,
    method::TEXTURE_DICTIONARY_RUNTIME_V1,
    method::PUMP_V1,
    method::STATUS_JSON_V1,
    method::STATUS_GRAPH_JSON_V1,
    method::PROJECT_STATUS_JSON_V1,
    method::FORMATS_JSON_V1,
];

/// Startup validation contract for the engine-facing asset gateway.
///
/// Validation reads the active backend provider description through the gateway.
pub const ASSET_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ASSET_SERVICE_ID,
        "newengine.assets-api >= 0.8.x",
        REQUIRED_RUNTIME_METHODS_V1,
    );

/// Runtime-ready texture packet returned by AssetManager.
///
/// Important: this is not a decoder contract. The importer pipeline must already
/// have converted the source container (DDS/PNG/JPEG/etc.) into RGBA8 or an
/// explicit renderer-native payload. Runtime code only consumes this normalized
/// packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba8TextureAsset {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Rgba8TextureAsset {
    #[inline]
    pub fn expected_len(width: u32, height: u32) -> usize {
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4)
    }

    #[inline]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err(format!("rgba8 texture has zero extent {width}x{height}"));
        }
        let expected = Self::expected_len(width, height);
        if rgba.len() != expected {
            return Err(format!(
                "rgba8 texture payload size mismatch bytes={} expected={} extent={}x{}",
                rgba.len(), expected, width, height
            ));
        }
        Ok(Self { width, height, rgba })
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeTextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Bc1RgbaUnorm,
    Bc1RgbaSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaSrgb,
    Bc5RgUnorm,
    Bc7RgbaUnorm,
    Bc7RgbaSrgb,
}

impl RuntimeTextureFormat {
    #[inline]
    pub const fn as_wire_id(self) -> u16 {
        match self {
            Self::Rgba8Unorm => 1,
            Self::Rgba8Srgb => 2,
            Self::Bc1RgbaUnorm => 101,
            Self::Bc1RgbaSrgb => 102,
            Self::Bc3RgbaUnorm => 103,
            Self::Bc3RgbaSrgb => 104,
            Self::Bc5RgUnorm => 105,
            Self::Bc7RgbaUnorm => 106,
            Self::Bc7RgbaSrgb => 107,
        }
    }

    #[inline]
    pub const fn from_wire_id(id: u16) -> Option<Self> {
        match id {
            1 => Some(Self::Rgba8Unorm),
            2 => Some(Self::Rgba8Srgb),
            101 => Some(Self::Bc1RgbaUnorm),
            102 => Some(Self::Bc1RgbaSrgb),
            103 => Some(Self::Bc3RgbaUnorm),
            104 => Some(Self::Bc3RgbaSrgb),
            105 => Some(Self::Bc5RgUnorm),
            106 => Some(Self::Bc7RgbaUnorm),
            107 => Some(Self::Bc7RgbaSrgb),
            _ => None,
        }
    }

    #[inline]
    pub fn from_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "RGBA8_UNORM" | "RGBA8" => Some(Self::Rgba8Unorm),
            "RGBA8_SRGB" => Some(Self::Rgba8Srgb),
            "BC1_RGBA_UNORM" | "BC1_UNORM" | "BC1" => Some(Self::Bc1RgbaUnorm),
            "BC1_RGBA_SRGB" | "BC1_SRGB" => Some(Self::Bc1RgbaSrgb),
            "BC3_RGBA_UNORM" | "BC3_UNORM" | "BC3" => Some(Self::Bc3RgbaUnorm),
            "BC3_RGBA_SRGB" | "BC3_SRGB" => Some(Self::Bc3RgbaSrgb),
            "BC5_RG_UNORM" | "BC5_UNORM" | "BC5" => Some(Self::Bc5RgUnorm),
            "BC7_RGBA_UNORM" | "BC7_UNORM" | "BC7" => Some(Self::Bc7RgbaUnorm),
            "BC7_RGBA_SRGB" | "BC7_SRGB" => Some(Self::Bc7RgbaSrgb),
            _ => None,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "RGBA8_UNORM",
            Self::Rgba8Srgb => "RGBA8_SRGB",
            Self::Bc1RgbaUnorm => "BC1_RGBA_UNORM",
            Self::Bc1RgbaSrgb => "BC1_RGBA_SRGB",
            Self::Bc3RgbaUnorm => "BC3_RGBA_UNORM",
            Self::Bc3RgbaSrgb => "BC3_RGBA_SRGB",
            Self::Bc5RgUnorm => "BC5_RG_UNORM",
            Self::Bc7RgbaUnorm => "BC7_RGBA_UNORM",
            Self::Bc7RgbaSrgb => "BC7_RGBA_SRGB",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureMip {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTextureAsset {
    pub width: u32,
    pub height: u32,
    pub format: RuntimeTextureFormat,
    pub mips: Vec<RuntimeTextureMip>,
}

impl RuntimeTextureAsset {
    #[inline]
    pub fn concatenated_payload_and_layout(&self) -> (Vec<u8>, Vec<RuntimeTextureMipLayout>) {
        let mut data = Vec::new();
        let mut layout = Vec::with_capacity(self.mips.len());
        for mip in &self.mips {
            let offset = data.len() as u64;
            data.extend_from_slice(&mip.bytes);
            layout.push(RuntimeTextureMipLayout {
                level: mip.level,
                width: mip.width,
                height: mip.height,
                offset,
                byte_len: mip.bytes.len() as u64,
            });
        }
        (data, layout)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeTextureMipLayout {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub offset: u64,
    pub byte_len: u64,
}

pub mod texture_wire {
    pub const MAGIC: [u8; 4] = *b"NTRT";
    pub const VERSION_RGBA8_V1: u16 = 1;
    pub const VERSION_RUNTIME_V2: u16 = 2;
    pub const HEADER_LEN: usize = 20;
    pub const RUNTIME_HEADER_LEN: usize = 32;
    pub const RUNTIME_MIP_RECORD_LEN: usize = 20;
}

/// Asset lifecycle state as observed through an AssetManager-like service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed,
    Unknown,
}

/// Residency domain. Stages are meaningful only inside a domain: VFS bytes, CPU-imported payloads, and GPU resources are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetResidencyDomain {
    Vfs,
    Cpu,
    Gpu,
    Unknown,
}

impl AssetResidencyDomain {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vfs => "vfs",
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Unknown => "unknown",
        }
    }
}

impl core::fmt::Display for AssetResidencyDomain {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}


/// Stable, high-resolution asset lifecycle stage used by tooling, loading screens and render gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetStatusStage {
    Declared,
    Requested,
    Resolving,
    Queued,
    Reading,
    Importing,
    Imported,
    UploadQueued,
    Uploading,
    Resident,
    Failed,
    Stale,
    Unknown,
}

impl AssetStatusStage {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Requested => "requested",
            Self::Resolving => "resolving",
            Self::Queued => "queued",
            Self::Reading => "reading",
            Self::Importing => "importing",
            Self::Imported => "imported",
            Self::UploadQueued => "upload_queued",
            Self::Uploading => "uploading",
            Self::Resident => "resident",
            Self::Failed => "failed",
            Self::Stale => "stale",
            Self::Unknown => "unknown",
        }
    }
}

impl core::fmt::Display for AssetStatusStage {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// AssetStatus is the canonical read-model row for one asset graph node.
///
/// The service serializes the same shape as JSON via `asset.status_json_v1`.
/// Runtime systems may keep richer local states, but they should be projected
/// from this model instead of inventing incompatible lifecycle enums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStatus {
    pub id_hex32: String,
    pub logical_path: Option<String>,
    pub state: AssetState,
    pub domain: AssetResidencyDomain,
    pub stage: AssetStatusStage,
    pub source: Option<String>,
    pub importer_id: Option<String>,
    pub type_id: Option<String>,
    pub format: Option<String>,
    pub bytes: Option<u64>,
    pub error: Option<String>,
    pub detail: Option<String>,
    pub updated_unix_ms: u64,
}

impl AssetStatus {
    #[inline]
    pub fn unknown(id_hex32: impl Into<String>) -> Self {
        Self {
            id_hex32: id_hex32.into(),
            logical_path: None,
            state: AssetState::Unknown,
            domain: AssetResidencyDomain::Unknown,
            stage: AssetStatusStage::Unknown,
            source: None,
            importer_id: None,
            type_id: None,
            format: None,
            bytes: None,
            error: None,
            detail: Some("AssetManager has no status row for this asset".to_string()),
            updated_unix_ms: 0,
        }
    }
}

/// Minimal engine-facing Asset access surface.
///
/// Implementations may be plugin-backed, filesystem-backed, HTTP-backed, etc.
pub trait AssetAccess {
    /// Enqueue importer-owned asset import by logical path. Returns an opaque stable id (hex32 string).
    fn import_v1(&self, logical_path: &str) -> Result<String, String>;

    /// Progress background AssetManager work through the stable v1 pump method.
    fn pump(&self);

    /// Query current state for an enqueued asset.
    fn state(&self, id_hex32: &str) -> Result<AssetState, String>;

    /// Query the canonical AssetStatus read-model row for an asset id or logical path.
    fn status_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String>;

    /// Query the full AssetStatus graph node for an asset id or logical path.
    fn status_graph_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String>;

    /// Project a validated lifecycle transition from an owning subsystem, e.g. render GPU residency.
    fn project_status_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String>;

    /// Read UTF-8/text asset bytes by logical path through AssetManager/VFS.
    fn text_v1(&self, logical_path: &str) -> Result<Vec<u8>, String>;

    /// Read raw binary asset bytes by logical path through AssetManager/VFS.
    ///
    /// This is still AssetManager-owned VFS access; callers must not use
    /// filesystem paths or bypass mounts.
    fn raw_bytes_v1(&self, logical_path: &str) -> Result<Vec<u8>, String>;

    /// Read asset payload using a stable wire format.
    ///
    /// Returns `(meta_json, payload_bytes)`.
    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String>;

    /// Read a runtime-ready RGBA8 texture packet.
    ///
    /// The implementation must parse/validate importer metadata inside AssetManager.
    /// Runtime callers must not parse image containers or importer metadata.
    fn texture_rgba8_v1(&self, id_hex32: &str) -> Result<Rgba8TextureAsset, String>;

    /// Select and read a runtime-ready RGBA8 texture from a .neytd dictionary.
    fn texture_dictionary_rgba8_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<Rgba8TextureAsset, String>;

    /// Select and read a runtime-ready GPU-native texture from a .neytd dictionary.
    fn texture_dictionary_runtime_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<RuntimeTextureAsset, String>;
}

/// Extended contract surface.
///
/// Keep this trait small and data-oriented; higher-level systems can build their own
/// caches and decoders above these primitives.
pub trait AssetService: AssetAccess {
    /// Reload/reimport asset by logical path through the stable v1 reload method.
    fn reload(&self, logical_path: &str) -> Result<String, String>;

    /// Query extended info by logical path.
    fn info_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// List known formats.
    fn formats_json_v1(&self) -> Result<serde_json::Value, String>;

    /// List mounted sources.
    fn sources_json_v1(&self) -> Result<serde_json::Value, String>;

    /// Mount one source through the strict v1 JSON source model.
    fn mount_source_json_v1(&self, payload: serde_json::Value) -> Result<(), String>;

    /// Returns a deterministic trace describing which sources contain the asset.
    fn resolve_trace_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// Backward-compatible Rust wrappers for legacy Rust callers.
    ///
    /// These wrappers are compiled only when `legacy_asset_api_compat` is enabled.
    /// Strict builds must use explicit `*_v1` methods so the Rust surface mirrors
    /// the wire-level ABI and cannot hide deprecated entry points.
    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use info_json_v1")]
    #[inline]
    fn info_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.info_json_v1(logical_path)
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use formats_json_v1")]
    #[inline]
    fn formats_json(&self) -> Result<serde_json::Value, String> {
        self.formats_json_v1()
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use sources_json_v1")]
    #[inline]
    fn sources_json(&self) -> Result<serde_json::Value, String> {
        self.sources_json_v1()
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use mount_source_json_v1")]
    #[inline]
    fn mount_pak(&self, path_to_pak: &str) -> Result<(), String> {
        self.mount_source_json_v1(serde_json::json!({
            "kind": "nepak",
            "priority": 100,
            "mount": "",
            "config": { "path": path_to_pak }
        }))
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use mount_source_json_v1")]
    #[inline]
    fn mount_dir(&self, path_to_dir: &str) -> Result<(), String> {
        self.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": 200,
            "mount": "",
            "config": { "root": path_to_dir }
        }))
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use mount_source_json_v1")]
    #[inline]
    fn mount_pak_prio(&self, path_to_pak: &str, priority: i32) -> Result<(), String> {
        self.mount_source_json_v1(serde_json::json!({
            "kind": "nepak",
            "priority": priority,
            "mount": "",
            "config": { "path": path_to_pak }
        }))
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use mount_source_json_v1")]
    #[inline]
    fn mount_dir_prio(&self, path_to_dir: &str, priority: i32) -> Result<(), String> {
        self.mount_source_json_v1(serde_json::json!({
            "kind": "filesystem",
            "priority": priority,
            "mount": "",
            "config": { "root": path_to_dir }
        }))
    }

    #[cfg(feature = "legacy_asset_api_compat")]
    #[deprecated(note = "use resolve_trace_json_v1")]
    #[inline]
    fn resolve_trace_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        self.resolve_trace_json_v1(logical_path)
    }

}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitReadyError {
    Timeout,
    Failed(String),
    Transport(String),
}

/// Wait until the asset reaches `Ready` or `Failed`, periodically calling `pump()`.
///
/// Polling interval is intentionally conservative to avoid busy-waiting.
pub fn wait_ready<A: AssetAccess + ?Sized>(
    assets: &A,
    id_hex32: &str,
    timeout: Duration,
) -> Result<(), WaitReadyError> {
    const SLEEP_MS: u64 = 8;

    let deadline = Instant::now() + timeout;

    loop {
        assets.pump();

        match assets.state(id_hex32) {
            Ok(AssetState::Ready) => return Ok(()),
            Ok(AssetState::Failed) => return Err(WaitReadyError::Failed(id_hex32.to_string())),
            Ok(AssetState::Loading) | Ok(AssetState::Unloaded) => {}
            Ok(AssetState::Unknown) => {
                log::warn!("Unknown asset state id='{}'", id_hex32);
            }
            Err(e) => return Err(WaitReadyError::Transport(e)),
        }

        if Instant::now() >= deadline {
            return Err(WaitReadyError::Timeout);
        }

        std::thread::sleep(Duration::from_millis(SLEEP_MS));
    }
}
