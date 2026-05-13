#![forbid(unsafe_op_in_unsafe_fn)]

use core::time::Duration;
use std::time::Instant;

/// Default service id for the AssetManager service.
///
/// This id is part of the stable engine/plugin contract and must be imported
/// by clients and providers instead of being duplicated in plugin crates.
pub const ASSET_SERVICE_ID: &str = "asset.manager";

/// Canonical AssetManager method names.
///
/// Keep compatibility aliases here as the single source of truth. Runtime code
/// should validate against the `*_V1` aliases, while older call sites may keep
/// using the legacy names during migration.
pub mod method {
    pub const LOAD: &str = "asset.load";
    pub const RELOAD: &str = "asset.reload";
    pub const PUMP: &str = "asset.pump";
    pub const INFO_JSON: &str = "asset.info_json";
    pub const STATE_JSON: &str = "asset.state_json";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";

    /// Stable v1 import entry point. Alias-compatible with `asset.load`.
    pub const IMPORT_V1: &str = "asset.import_v1";
    /// Stable v1 pump entry point. Alias-compatible with `asset.pump`.
    pub const PUMP_V1: &str = "asset.pump_v1";
    /// Raw VFS bytes by logical path. This bypasses importers but still resolves exclusively through AssetManager mounts.
    pub const RAW_BYTES_V1: &str = "asset.raw_bytes_v1";
    /// Raw UTF-8 text by logical path resolved through AssetManager mounts.
    pub const TEXT_V1: &str = "asset.text_v1";
    /// Compatibility text-load alias used by older runtime/editor call sites.
    pub const LOAD_TEXT_V1: &str = "asset.load_text_v1";

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
    method::LOAD_TEXT_V1,
    method::IMPORT_V1,
    method::PUMP_V1,
];

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
    /// Enqueue asset load by logical path. Returns an opaque stable id (hex32 string).
    fn load(&self, logical_path: &str) -> Result<String, String>;

    /// Progress background work.
    fn pump(&self);

    /// Query current state for an enqueued asset.
    fn state(&self, id_hex32: &str) -> Result<AssetState, String>;

    /// Read asset payload using a stable wire format.
    ///
    /// Returns `(meta_json, payload_bytes)`.
    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String>;
}

/// Extended contract surface.
///
/// Keep this trait small and data-oriented; higher-level systems can build their own
/// caches and decoders above these primitives.
pub trait AssetService: AssetAccess {
    /// Reload asset by logical path (implementation-defined cache invalidation).
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
