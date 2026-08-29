use newengine_plugin_api::MethodName;

use super::AssetServiceClient;
use crate::{
    method, AssetStreamingCleanupRequestV1, AssetStreamingCleanupResponseV1,
    AssetStreamingPinRequestV1, AssetStreamingRequestV1, AssetStreamingStatsV1,
    AssetStreamingTouchRequestV1, ENGINE_ASSETS_STREAMING_SERVICE_ID,
};

impl AssetServiceClient {
    pub fn streaming_request_v1(
        &self,
        request: &AssetStreamingRequestV1,
    ) -> Result<serde_json::Value, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_REQUEST_V1),
            request,
            "asset.streaming.request_v1",
        )
    }

    pub fn streaming_pin_v1(
        &self,
        request: &AssetStreamingPinRequestV1,
    ) -> Result<serde_json::Value, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_PIN_V1),
            request,
            "asset.streaming.pin_v1",
        )
    }

    pub fn streaming_unpin_v1(
        &self,
        request: &AssetStreamingPinRequestV1,
    ) -> Result<serde_json::Value, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_UNPIN_V1),
            request,
            "asset.streaming.unpin_v1",
        )
    }

    pub fn streaming_touch_v1(
        &self,
        request: &AssetStreamingTouchRequestV1,
    ) -> Result<serde_json::Value, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_TOUCH_V1),
            request,
            "asset.streaming.touch_v1",
        )
    }

    pub fn streaming_cleanup_v1(
        &self,
        request: &AssetStreamingCleanupRequestV1,
    ) -> Result<AssetStreamingCleanupResponseV1, String> {
        self.call_service_json_typed(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_CLEANUP_V1),
            request,
            "asset.streaming.cleanup_v1",
        )
    }

    pub fn streaming_compact_v1(&self) -> Result<serde_json::Value, String> {
        let bytes = self.call_service(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_COMPACT_V1),
            Vec::new(),
        )?;
        Self::decode_ok_json(bytes)
    }

    pub fn streaming_stats_v1(&self) -> Result<AssetStreamingStatsV1, String> {
        let bytes = self.call_service(
            ENGINE_ASSETS_STREAMING_SERVICE_ID,
            MethodName::from(method::STREAMING_STATS_V1),
            Vec::new(),
        )?;
        Self::decode_json(bytes, "asset.streaming.stats_v1")
    }
}
