use newengine_service_kit::{
    engine_gateway_provider_service_description,
    register_engine_gateway_provider_service_dynamic_best_effort, EngineGatewayProviderDeclDynamic,
    JsonServiceRouter,
};
use newengine_ui_api::{
    ui_notify_method, UiNotifyClearRequest, UiNotifyDismissRequest, UiNotifyRequest,
    UiNotifyServiceInfoV1, ENGINE_UI_NOTIFY_SERVICE_ID, UI_NOTIFY_BACKEND_CAPABILITY_ID,
    UI_NOTIFY_PROVIDER_ROUTE, UI_NOTIFY_RUNTIME_CONTRACT, UI_NOTIFY_SERVICE_ID,
    UI_NOTIFY_SERVICE_METHODS,
};

use crate::state::UiNotifyRuntime;

const OWNER: &str = "newengine-ui-notify-runtime.engine-runtime-provider";

fn invoke(
    runtime: &mut UiNotifyRuntime,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let method = request
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(ui_notify_method::SNAPSHOT_V1);
    let payload = request
        .get("request")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    match method {
        ui_notify_method::INFO_JSON => serde_json::to_value(UiNotifyServiceInfoV1::default()),
        ui_notify_method::SNAPSHOT_V1 => serde_json::to_value(runtime.snapshot()),
        ui_notify_method::PUSH_V1 => {
            let request = serde_json::from_value::<UiNotifyRequest>(payload)
                .map_err(|error| format!("ui.notify: invalid push request: {error}"))?;
            serde_json::to_value(runtime.push(request))
        }
        ui_notify_method::DISMISS_V1 => {
            let request = serde_json::from_value::<UiNotifyDismissRequest>(payload)
                .map_err(|error| format!("ui.notify: invalid dismiss request: {error}"))?;
            serde_json::to_value(runtime.dismiss(request))
        }
        ui_notify_method::CLEAR_V1 => {
            let request = serde_json::from_value::<UiNotifyClearRequest>(payload)
                .map_err(|error| format!("ui.notify: invalid clear request: {error}"))?;
            serde_json::to_value(runtime.clear(request))
        }
        other => return Err(format!("ui.notify: unknown invoke method '{other}'")),
    }
    .map_err(|error| error.to_string())
}

fn service(runtime: UiNotifyRuntime) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        UI_NOTIFY_SERVICE_ID,
        OWNER,
        UI_NOTIFY_BACKEND_CAPABILITY_ID,
        UI_NOTIFY_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_UI_NOTIFY_SERVICE_ID)
    .protocol(UI_NOTIFY_RUNTIME_CONTRACT)
    .features([
        "bounded-notification-queue-v1",
        "typed-game-message-subscription-v1",
        "severity-and-ttl-policy-v1",
        "sticky-notifications-v1",
        "stable-id-replacement-v1",
        "provider-neutral-ui-toast-stack-v1",
    ])
    .notes("The runtime owns notification policy and subscribes to GameMessageEnvelope. engine.ui owns presentation; render backends never interpret notification semantics.");

    JsonServiceRouter::with_state(UI_NOTIFY_SERVICE_ID, runtime)
        .describe_json(&description)
        .get_json(ui_notify_method::INFO_JSON, |_runtime| {
            UiNotifyServiceInfoV1::default()
        })
        .post_json(ui_notify_method::PUSH_V1, |runtime, request| {
            runtime.push(request)
        })
        .post_json(ui_notify_method::DISMISS_V1, |runtime, request| {
            runtime.dismiss(request)
        })
        .post_json(ui_notify_method::CLEAR_V1, |runtime, request| {
            runtime.clear(request)
        })
        .get_json(ui_notify_method::SNAPSHOT_V1, |runtime| runtime.snapshot())
        .json_value_result(ui_notify_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1()
}

pub fn register_ui_notify_gateway_best_effort(runtime: UiNotifyRuntime) -> bool {
    register_engine_gateway_provider_service_dynamic_best_effort(EngineGatewayProviderDeclDynamic {
        gateway: ENGINE_UI_NOTIFY_SERVICE_ID,
        service_kind: "ui.notify",
        provider_service: UI_NOTIFY_SERVICE_ID,
        provider_route: UI_NOTIFY_PROVIDER_ROUTE,
        capability: UI_NOTIFY_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(runtime),
    })
}
