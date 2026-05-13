#![forbid(unsafe_op_in_unsafe_fn)]

use core::time::Duration;
use std::time::Instant;

/// Default service id for the AssetManager service.
///
/// This id is part of the stable engine/plugin contract and must be imported
/// by clients and providers instead of being duplicated in plugin crates.
pub const ASSET_SERVICE_ID: &str = "asset.manager";

/// Canonical AssetManager v1 method names.
///
/// There is one supported runtime contract: explicit `*_v1` entry points for
/// import/pump/state/text/texture access. Older alias pairs such as
/// `asset.load`, `asset.pump`, and `asset.load_text_v1` are intentionally not
/// part of this surface.
pub mod method {
    pub const RELOAD_V1: &str = "asset.reload_v1";
    pub const INFO_JSON: &str = "asset.info_json";
    pub const STATE_JSON: &str = "asset.state_json";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";
    /// Runtime-ready RGBA8 texture packet by asset id. AssetManager validates/parses importer metadata.
    pub const TEXTURE_RGBA8_V1: &str = "asset.texture_rgba8_v1";

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

    pub const FORMATS_JSON: &str = "asset.formats_json";
    pub const SOURCES_JSON: &str = "asset.sources_json";
    pub const VERIFY_ASSETS_JSON: &str = "asset.verify_assets_json";
    pub const SOURCE_KINDS_JSON: &str = "asset.source_kinds_json";
    pub const MOUNT_PAK: &str = "asset.mount_pak";
    pub const MOUNT_DIR: &str = "asset.mount_dir";

    pub const MOUNT_PAK_PRIO: &str = "asset.mount_pak_prio";
    pub const MOUNT_DIR_PRIO: &str = "asset.mount_dir_prio";
    pub const MOUNT_HTTP_PRIO: &str = "asset.mount_http_prio";
    pub const MOUNT_SOURCE_V1: &str = "asset.mount_source_v1";

    // Debug/diagnostics.
    pub const RESOLVE_TRACE_JSON: &str = "asset.resolve_trace_json";

    // Generic lifecycle hook understood by the plugin host.
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
}

/// Required runtime methods for AssetManager 0.6+ deployments.
///
/// The engine validates these before scene bootstrap so an old DLL cannot fail
/// later as "unknown method" inside foliage/profile loading.
pub const REQUIRED_RUNTIME_METHODS_V1: &[&str] = &[
    method::RAW_BYTES_V1,
    method::TEXT_V1,
    method::IMPORT_V1,
    method::TEXTURE_RGBA8_V1,
    method::PUMP_V1,
    method::FORMATS_JSON,
];


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

pub mod texture_wire {
    pub const MAGIC: [u8; 4] = *b"NTRT";
    pub const VERSION_RGBA8_V1: u16 = 1;
    pub const HEADER_LEN: usize = 20;
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
}

/// Extended contract surface.
///
/// Keep this trait small and data-oriented; higher-level systems can build their own
/// caches and decoders above these primitives.
pub trait AssetService: AssetAccess {
    /// Reload/reimport asset by logical path through the stable v1 reload method.
    fn reload(&self, logical_path: &str) -> Result<String, String>;

    /// Query extended info by logical path.
    fn info_json(&self, logical_path: &str) -> Result<serde_json::Value, String>;

    /// List known formats.
    fn formats_json(&self) -> Result<serde_json::Value, String>;

    /// List mounted sources.
    fn sources_json(&self) -> Result<serde_json::Value, String>;

    /// Mount a `.pak` at runtime (if supported by the service).
    fn mount_pak(&self, path_to_pak: &str) -> Result<(), String>;

    /// Mount a directory at runtime (if supported by the service).
    fn mount_dir(&self, path_to_dir: &str) -> Result<(), String>;

    /// Mount a `.pak` layer with an explicit priority.
    fn mount_pak_prio(&self, path_to_pak: &str, priority: i32) -> Result<(), String>;

    /// Mount a directory layer with an explicit priority.
    fn mount_dir_prio(&self, path_to_dir: &str, priority: i32) -> Result<(), String>;

    /// Returns a deterministic trace describing which sources contain the asset.
    fn resolve_trace_json(&self, logical_path: &str) -> Result<serde_json::Value, String>;
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
