#![forbid(unsafe_op_in_unsafe_fn)]

use core::time::Duration;
use std::time::Instant;

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
