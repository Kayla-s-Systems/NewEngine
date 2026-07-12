use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    textures_method, ENGINE_ASSETS_TEXTURES_SERVICE_ID, TEXTURES_BACKEND_CAPABILITY_ID,
    TEXTURES_RUNTIME_CONTRACT, TEXTURES_SERVICE_ID, TEXTURES_SERVICE_METHODS,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};

use crate::{
    handlers::{entry_rgba8_blob, entry_runtime_blob, invoke_json, manifest_blob},
    manifest::validate_texture_ref,
    service::{textures_service_info, TEXTURES_GATEWAY_OWNER},
    state::TextureRuntimeState,
    TextureRefRequest, TextureRefValidation,
};

pub fn textures_gateway_service(
    client: AssetServiceClient,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        TEXTURES_SERVICE_ID,
        TEXTURES_GATEWAY_OWNER,
        TEXTURES_BACKEND_CAPABILITY_ID,
        TEXTURES_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_TEXTURES_SERVICE_ID)
    .protocol(TEXTURES_RUNTIME_CONTRACT)
    .features([
        "ytd-manifest",
        "runtime-texture-packet",
        "rgba8-debug-packet",
        "strict-ref-validation",
    ])
    .notes("Engine texture runtime service. .ytd semantics live in engine.assets.textures; VFS/raw bytes/codec dispatch remain in engine.assets.");

    JsonServiceRouter::with_state(TEXTURES_SERVICE_ID, TextureRuntimeState::new(client))
        .describe_json(&description)
        .info(textures_service_info)
        .blob(textures_method::MANIFEST_JSON_V1, manifest_blob)
        .post_json_result::<TextureRefRequest, TextureRefValidation, _>(
            textures_method::VALIDATE_REF_V1,
            validate_texture_ref,
        )
        .post_json_result::<TextureRefRequest, TextureRefValidation, _>(
            textures_method::DESCRIBE_REF_JSON_V1,
            validate_texture_ref,
        )
        .blob(textures_method::ENTRY_RUNTIME_V1, entry_runtime_blob)
        .blob(textures_method::ENTRY_RGBA8_V1, entry_rgba8_blob)
        .blob(textures_method::INVOKE_JSON, invoke_json)
        .blob(textures_method::SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}
