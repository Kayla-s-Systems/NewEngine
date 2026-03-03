#![forbid(unsafe_op_in_unsafe_fn)]

use std::time::{Duration, Instant};

/// Asset lifecycle state as observed through an AssetManager-like service.
///
/// This enum is intentionally minimal and stable; adapters can map richer states.
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
/// `newengine-ui` does not require the AssetManager crate; the host can provide any implementation
/// (plugin-backed, filesystem-backed, HTTP-backed, etc.).
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

#[derive(Debug, Clone)]
pub struct WaitReadyError;

#[inline]
pub fn wait_ready<A: AssetAccess + ?Sized>(
    assets: &A,
    id_hex32: &str,
    timeout: Duration,
) -> Result<(), WaitReadyError> {
    let deadline = Instant::now() + timeout;

    loop {
        assets.pump();

        match assets.state(id_hex32) {
            Ok(AssetState::Ready) => return Ok(()),
            Ok(AssetState::Failed) => return Err(WaitReadyError),
            _ => {}
        }

        if Instant::now() >= deadline {
            return Err(WaitReadyError);
        }

        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(feature = "assets")]
mod assets_adapter {
    use super::*;

    impl From<newengine_assets::AssetState> for AssetState {
        #[inline]
        fn from(v: newengine_assets::AssetState) -> Self {
            match v {
                newengine_assets::AssetState::Unloaded => AssetState::Unloaded,
                newengine_assets::AssetState::Loading => AssetState::Loading,
                newengine_assets::AssetState::Ready => AssetState::Ready,
                newengine_assets::AssetState::Failed => AssetState::Failed,
                newengine_assets::AssetState::Unknown => AssetState::Unknown,
            }
        }
    }

    impl<T: newengine_assets::AssetAccess + ?Sized> AssetAccess for T {
        #[inline]
        fn load(&self, logical_path: &str) -> Result<String, String> {
            newengine_assets::AssetAccess::load(self, logical_path)
        }

        #[inline]
        fn pump(&self) {
            newengine_assets::AssetAccess::pump(self)
        }

        #[inline]
        fn state(&self, id_hex32: &str) -> Result<AssetState, String> {
            newengine_assets::AssetAccess::state(self, id_hex32).map(AssetState::from)
        }

        #[inline]
        fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String> {
            newengine_assets::AssetAccess::blob_wire_v1(self, id_hex32)
        }
    }
}

#[cfg(feature = "assets")]
pub use newengine_assets::{AssetService, AssetServiceClient};
