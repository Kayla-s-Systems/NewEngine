use newengine_assets_api::{
    ENGINE_ASSETS_TEXTURES_SERVICE_ID, ENGINE_ASSET_SERVICE_ID, TEXTURES_RUNTIME_CONTRACT,
    TEXTURES_SERVICE_ID, TEXTURES_SERVICE_METHODS,
};

use crate::dto::TexturesServiceInfo;

pub const TEXTURES_GATEWAY_OWNER: &str = "newengine-textures-runtime.semantic-service";
pub(crate) const TEXTURES_PROVIDER_NAME: &str = "NorthStarYtdTextureSemanticService";

pub fn textures_service_info() -> TexturesServiceInfo {
    TexturesServiceInfo {
        id: TEXTURES_SERVICE_ID,
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        provider: TEXTURES_PROVIDER_NAME,
        contract: TEXTURES_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        methods: TEXTURES_SERVICE_METHODS,
        validation_policy:
            "accept .ytd@entry and .ytd@hash:<u64>; reject raw images and .ytd without @entry",
    }
}
