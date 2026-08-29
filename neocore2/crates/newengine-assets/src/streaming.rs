#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{AssetServiceClient, AssetStreamingPinClassV1, AssetStreamingPinRequestV1};

/// RAII residency reference for mission/cutscene/interior/runtime consumers.
///
/// Acquiring the lease increments exactly one `(class, owner)` reference in
/// `engine.assets.streaming`; dropping/releasing it decrements the same reference.
/// This prevents lifecycle owners from forgetting to unpin on scene teardown.
pub struct AssetStreamingPinLease {
    client: AssetServiceClient,
    request: AssetStreamingPinRequestV1,
    released: bool,
}

impl core::fmt::Debug for AssetStreamingPinLease {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AssetStreamingPinLease")
            .field("logical_path", &self.request.logical_path)
            .field("owner", &self.request.owner)
            .field("class", &self.request.class)
            .field("released", &self.released)
            .finish()
    }
}

impl AssetStreamingPinLease {
    pub fn acquire(
        client: AssetServiceClient,
        logical_path: impl Into<String>,
        owner: impl Into<String>,
        class: AssetStreamingPinClassV1,
    ) -> Result<Self, String> {
        let request = AssetStreamingPinRequestV1 {
            logical_path: logical_path.into(),
            owner: owner.into(),
            class,
        };
        client.streaming_pin_v1(&request)?;
        Ok(Self {
            client,
            request,
            released: false,
        })
    }

    #[inline]
    pub fn logical_path(&self) -> &str {
        &self.request.logical_path
    }

    #[inline]
    pub fn owner(&self) -> &str {
        &self.request.owner
    }

    #[inline]
    pub fn class(&self) -> AssetStreamingPinClassV1 {
        self.request.class
    }

    pub fn release(&mut self) -> Result<bool, String> {
        if self.released {
            return Ok(false);
        }
        let response = self.client.streaming_unpin_v1(&self.request)?;
        let released = response
            .get("released")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        self.released = true;
        Ok(released)
    }
}

impl Drop for AssetStreamingPinLease {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.client.streaming_unpin_v1(&self.request);
            self.released = true;
        }
    }
}
