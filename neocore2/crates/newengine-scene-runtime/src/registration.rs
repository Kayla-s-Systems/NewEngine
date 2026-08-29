use std::sync::Arc;

use newengine_scene_io::{
    method as scene_method, ENGINE_SCENE_SERVICE_ID, SCENE_BACKEND_CAPABILITY_ID,
};
use newengine_service_kit::{
    ok_empty_blob, register_engine_gateway_provider_service, EngineGatewayProviderDecl,
    JsonServiceRouter,
};

use crate::constants::{authored_scene_method, SCENE_GATEWAY_OWNER, SCENE_SERVICE_METHODS};
use crate::{EngineSceneGatewayService, SceneBridge, SceneGatewayAssetMounts};

pub fn scene_gateway_service(
    scene: Arc<SceneBridge>,
    asset_mounts: Option<SceneGatewayAssetMounts>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = match asset_mounts {
        Some(mounts) => EngineSceneGatewayService::with_asset_mounts(scene, mounts),
        None => EngineSceneGatewayService::new(scene),
    };
    let description = serde_json::json!({
        "id": ENGINE_SCENE_SERVICE_ID,
        "version": 1,
        "contract": "newengine.scene gateway >= 0.1.x",
        "origin": "engine-runtime",
        "owner": SCENE_GATEWAY_OWNER,
        "capability": SCENE_BACKEND_CAPABILITY_ID,
        "methods": SCENE_SERVICE_METHODS,
    });

    let formats_service = service.clone();
    let load_service = service.clone();
    let save_service = service.clone();
    let graph_service = service.clone();
    let archetype_service = service.clone();
    let placement_service = service.clone();
    let prefab_service = service.clone();
    let archetype_instantiate_service = service;

    JsonServiceRouter::new(ENGINE_SCENE_SERVICE_ID)
        .describe_json(&description)
        .blob(scene_method::FORMATS_JSON, move |_unit, _payload| {
            formats_service.formats_json()
        })
        .blob(scene_method::LOAD_JSON_V1, move |_unit, payload| {
            load_service.load_json_v1(payload)
        })
        .blob(scene_method::SAVE_JSON_V1, move |_unit, payload| {
            save_service.save_json_v1(payload)
        })
        .blob(
            authored_scene_method::GRAPH_JSON_V1,
            move |_unit, _payload| graph_service.graph_json_v1(),
        )
        .blob(
            authored_scene_method::ARCHETYPE_GRAPH_JSON_V1,
            move |_unit, _payload| archetype_service.archetype_graph_json_v1(),
        )
        .blob(
            authored_scene_method::PLACEMENTS_JSON_V1,
            move |_unit, _payload| placement_service.placements_json_v1(),
        )
        .blob(
            authored_scene_method::INSTANTIATE_PREFAB_JSON_V1,
            move |_unit, payload| prefab_service.instantiate_prefab_json_v1(payload),
        )
        .blob(
            authored_scene_method::INSTANTIATE_ARCHETYPE_JSON_V1,
            move |_unit, payload| {
                archetype_instantiate_service.instantiate_archetype_json_v1(payload)
            },
        )
        .blob(scene_method::SHUTDOWN_V1, |_unit, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_scene_gateway_best_effort(
    scene: Arc<SceneBridge>,
    asset_mounts: Option<SceneGatewayAssetMounts>,
) {
    if newengine_plugin_host::has_service(ENGINE_SCENE_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.scene gateway registration skipped; service already available"
        );
        return;
    }

    let service = scene_gateway_service(scene, asset_mounts);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_SCENE_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Scene,
        provider_service: ENGINE_SCENE_SERVICE_ID,
        provider_route: "engine.scene.foundation",
        capability: SCENE_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: SCENE_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.scene gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_SCENE_SERVICE_ID,
            SCENE_BACKEND_CAPABILITY_ID,
            SCENE_GATEWAY_OWNER
        ),
        Err(error) => newengine_ulog_api::ulog::error!(
            "engine.scene gateway registration failed id='{}' err='{}'",
            ENGINE_SCENE_SERVICE_ID,
            error
        ),
    }
}
