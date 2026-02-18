#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::{Duration, Instant};

/// Asset lifecycle state as observed through the AssetManager service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Unloaded,
    Loading,
    Ready,
    Failed,
}

/// Minimal engine-facing AssetManager access surface.
///
/// This is intentionally protocol-agnostic: the concrete service implementation
/// lives in the AssetManager plugin, while the engine talks via `HostApiV1::call_service_v1`.
pub trait AssetAccess {
    /// Enqueue asset load by logical path. Returns an opaque stable id (hex32 string).
    fn load(&self, logical_path: &str) -> Result<String, String>;

    /// Progress AssetManager background work.
    fn pump(&self);

    /// Query current state for an enqueued asset.
    fn state(&self, id_hex32: &str) -> Result<AssetState, String>;

    /// Read asset payload using a stable wire format.
    ///
    /// Returns `(meta_json, payload_bytes)`.
    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitReadyError {
    Timeout,
    Failed(String),
    Transport(String),
}

/// Wait until the asset reaches `Ready` or `Failed`, periodically calling `pump()`.
pub fn wait_ready<A: AssetAccess>(assets: &A, id_hex32: &str, timeout: Duration) -> Result<(), WaitReadyError> {
    let deadline = Instant::now() + timeout;

    loop {
        assets.pump();

        match assets.state(id_hex32) {
            Ok(AssetState::Ready) => return Ok(()),
            Ok(AssetState::Failed) => return Err(WaitReadyError::Failed(id_hex32.to_string())),
            Ok(AssetState::Loading) | Ok(AssetState::Unloaded) => {}
            Err(e) => return Err(WaitReadyError::Transport(e)),
        }

        if Instant::now() >= deadline {
            return Err(WaitReadyError::Timeout);
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}
