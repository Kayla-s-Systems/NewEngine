use super::*;

pub(super) fn submit_state_patch(patch: UiStatePatch) -> bool {
    let payload = match serde_json::to_vec(&patch) {
        Ok(payload) => payload,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: state patch encode failed contract='{}' err='{}'",
                ASSET_INSPECTOR_STATE_CONTRACT,
                error
            );
            return false;
        }
    };
    match newengine_core::call_service_v1_optional(
        ENGINE_UI_SERVICE_ID,
        UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1,
        &payload,
    ) {
        Ok(Some(_)) => true,
        Ok(None) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: engine.ui route unavailable source='{}'",
                ASSET_INSPECTOR_STATE_SOURCE
            );
            false
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "asset inspector: state patch failed source='{}' err='{}'",
                ASSET_INSPECTOR_STATE_SOURCE,
                error
            );
            false
        }
    }
}
