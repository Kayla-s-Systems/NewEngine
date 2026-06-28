use std::sync::{Arc, OnceLock};

use abi_stable::std_types::{RResult, RString};
use newengine_ai_api::{
    ai_method, AiFrameInputV1, AiFrameOutputV1, AiValidateIntentsRequestV1,
    AI_BACKEND_CAPABILITY_ID, AI_SERVICE_ID, AI_SERVICE_METHODS,
};
use newengine_animation_api::{
    animation_method, AnimationDescribeGraphsRequestV1, AnimationPlanRequestV1,
    AnimationValidateIntentRequestV1, ANIMATION_BACKEND_CAPABILITY_ID, ANIMATION_SERVICE_ID,
    ANIMATION_SERVICE_METHODS,
};
use newengine_navigation_api::{
    navigation_method, NavPlanPathRequestV1, NavProjectPointRequestV1, NavQueryStatusRequestV1,
    NAVIGATION_BACKEND_CAPABILITY_ID, NAVIGATION_SERVICE_ID, NAVIGATION_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_tags_api::{
    tags_method, TagsDescribeRequestV1, TagsResolveRequestV1, TagsSnapshotRequestV1,
    TagsValidateSetRequestV1, TAGS_REGISTRY_CAPABILITY_ID, TAGS_SERVICE_ID, TAGS_SERVICE_METHODS,
};
use newengine_tasks_api::{
    tasks_method, TasksDescribeRequestV1, TasksPlanQueueRequestV1, TasksValidateRequestV1,
    TASKS_BACKEND_CAPABILITY_ID, TASKS_SERVICE_ID, TASKS_SERVICE_METHODS,
};
use parking_lot::Mutex;
use serde_json::Value;

use crate::state::GameplayFoundationState;

static GAMEPLAY_STATE: OnceLock<Arc<Mutex<GameplayFoundationState>>> = OnceLock::new();

fn state() -> Arc<Mutex<GameplayFoundationState>> {
    Arc::clone(
        GAMEPLAY_STATE.get_or_init(|| Arc::new(Mutex::new(GameplayFoundationState::default()))),
    )
}

fn unknown_method(domain: &str, method: &str) -> RResult<Blob, RString> {
    RResult::RErr(RString::from(format!(
        "{domain}: unknown invoke method '{method}'"
    )))
}

fn envelope(payload: &Blob, default_method: &str) -> Result<(String, Value), RString> {
    let value = payload_json(payload).map_err(RString::from)?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(default_method)
        .to_owned();
    let request = value.get("request").cloned().unwrap_or(Value::Null);
    Ok((method, request))
}

fn tags_invoke(state: &mut GameplayFoundationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, tags_method::DESCRIBE_TAGS_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    match method.as_str() {
        tags_method::INFO_JSON => ok_json(state.tags_info()),
        tags_method::DESCRIBE_TAGS_JSON_V1 => {
            match serde_json::from_value::<TagsDescribeRequestV1>(request) {
                Ok(v) => ok_json(state.describe_tags(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        tags_method::RESOLVE_TAG_JSON_V1 => {
            match serde_json::from_value::<TagsResolveRequestV1>(request) {
                Ok(v) => ok_json(state.resolve_tag(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        tags_method::SNAPSHOT_JSON_V1 => {
            match serde_json::from_value::<TagsSnapshotRequestV1>(request) {
                Ok(v) => ok_json(state.tags_snapshot(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        tags_method::VALIDATE_TAG_SET_JSON_V1 => {
            match serde_json::from_value::<TagsValidateSetRequestV1>(request) {
                Ok(v) => ok_json(state.validate_tag_set(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        other => unknown_method("tags.api", other),
    }
}

fn tasks_invoke(state: &mut GameplayFoundationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, tasks_method::DESCRIBE_TASKS_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    match method.as_str() {
        tasks_method::INFO_JSON => ok_json(state.tasks_info()),
        tasks_method::DESCRIBE_TASKS_JSON_V1 => {
            match serde_json::from_value::<TasksDescribeRequestV1>(request) {
                Ok(v) => ok_json(state.describe_tasks(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        tasks_method::VALIDATE_TASK_JSON_V1 => {
            match serde_json::from_value::<TasksValidateRequestV1>(request) {
                Ok(v) => ok_json(state.validate_task(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        tasks_method::PLAN_QUEUE_JSON_V1 => {
            match serde_json::from_value::<TasksPlanQueueRequestV1>(request) {
                Ok(v) => ok_json(state.plan_queue(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        other => unknown_method("tasks.api", other),
    }
}

fn animation_invoke(state: &mut GameplayFoundationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, animation_method::DESCRIBE_GRAPHS_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    match method.as_str() {
        animation_method::INFO_JSON => ok_json(state.animation_info()),
        animation_method::DESCRIBE_GRAPHS_JSON_V1 => {
            match serde_json::from_value::<AnimationDescribeGraphsRequestV1>(request) {
                Ok(v) => ok_json(state.describe_animation_graphs(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        animation_method::PLAN_JSON_V1 => {
            match serde_json::from_value::<AnimationPlanRequestV1>(request) {
                Ok(v) => ok_json(state.plan_animation(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        animation_method::VALIDATE_INTENT_JSON_V1 => {
            match serde_json::from_value::<AnimationValidateIntentRequestV1>(request) {
                Ok(v) => ok_json(state.validate_animation_intent(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        other => unknown_method("animation.api", other),
    }
}

fn navigation_invoke(state: &mut GameplayFoundationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, navigation_method::PLAN_PATH_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    match method.as_str() {
        navigation_method::INFO_JSON => ok_json(state.navigation_info()),
        navigation_method::PLAN_PATH_JSON_V1 => {
            match serde_json::from_value::<NavPlanPathRequestV1>(request) {
                Ok(v) => ok_json(state.plan_path(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        navigation_method::PROJECT_POINT_JSON_V1 => {
            match serde_json::from_value::<NavProjectPointRequestV1>(request) {
                Ok(v) => ok_json(state.project_point(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        navigation_method::QUERY_STATUS_JSON_V1 => {
            match serde_json::from_value::<NavQueryStatusRequestV1>(request) {
                Ok(v) => ok_json(state.query_status(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        other => unknown_method("navigation.api", other),
    }
}

fn ai_invoke(state: &mut GameplayFoundationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, ai_method::FRAME_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    match method.as_str() {
        ai_method::INFO_JSON => ok_json(state.ai_info()),
        ai_method::FRAME_JSON_V1 => match serde_json::from_value::<AiFrameInputV1>(request) {
            Ok(v) => ok_json(state.ai_frame(v)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        },
        ai_method::VALIDATE_INTENTS_JSON_V1 => {
            match serde_json::from_value::<AiValidateIntentsRequestV1>(request) {
                Ok(v) => ok_json(state.validate_ai_intents(v)),
                Err(e) => RResult::RErr(RString::from(e.to_string())),
            }
        }
        ai_method::DECISION_TRACE_JSON_V1 => {
            ok_json(serde_json::json!({ "provider": crate::AI_PROVIDER_ROUTE, "trace": [] }))
        }
        other => unknown_method(AI_SERVICE_ID, other),
    }
}

fn service_description(
    service_id: &'static str,
    provider_route: &'static str,
    gateway: &'static str,
    capability: &'static str,
    methods: &'static [&'static str],
) -> newengine_service_kit::EngineGatewayProviderServiceDescription {
    engine_gateway_provider_service_description(service_id, provider_route, capability, methods.iter().copied())
        .gateway(gateway)
        .protocol("newengine.gameplay-foundation/v1")
        .features(["core-owned-baseline-provider", "replaceable-gateway-route", "dto-only-boundary"])
        .notes("Gameplay foundation provider observes DTOs and returns DTOs; runtime apply stages own mutation.")
}

fn register_tags_gateway() -> bool {
    let service = JsonServiceRouter::with_shared_state(TAGS_SERVICE_ID, state())
        .describe_json(&service_description(
            TAGS_SERVICE_ID,
            crate::TAGS_PROVIDER_ROUTE,
            "engine.tags",
            TAGS_REGISTRY_CAPABILITY_ID,
            TAGS_SERVICE_METHODS,
        ))
        .get_json(tags_method::INFO_JSON, |state| state.tags_info())
        .post_json(tags_method::DESCRIBE_TAGS_JSON_V1, |state, req| {
            state.describe_tags(req)
        })
        .post_json(tags_method::RESOLVE_TAG_JSON_V1, |state, req| {
            state.resolve_tag(req)
        })
        .post_json(tags_method::SNAPSHOT_JSON_V1, |state, req| {
            state.tags_snapshot(req)
        })
        .post_json(tags_method::VALIDATE_TAG_SET_JSON_V1, |state, req| {
            state.validate_tag_set(req)
        })
        .blob(tags_method::INVOKE_JSON, tags_invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.tags",
        service_kind: newengine_service_api::EngineServiceKind::Tags,
        provider_service: TAGS_SERVICE_ID,
        provider_route: crate::TAGS_PROVIDER_ROUTE,
        capability: TAGS_REGISTRY_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service,
    })
    .is_ok()
}

fn register_tasks_gateway() -> bool {
    let service = JsonServiceRouter::with_shared_state(TASKS_SERVICE_ID, state())
        .describe_json(&service_description(
            TASKS_SERVICE_ID,
            crate::TASKS_PROVIDER_ROUTE,
            "engine.tasks",
            TASKS_BACKEND_CAPABILITY_ID,
            TASKS_SERVICE_METHODS,
        ))
        .get_json(tasks_method::INFO_JSON, |state| state.tasks_info())
        .post_json(tasks_method::DESCRIBE_TASKS_JSON_V1, |state, req| {
            state.describe_tasks(req)
        })
        .post_json(tasks_method::VALIDATE_TASK_JSON_V1, |state, req| {
            state.validate_task(req)
        })
        .post_json(tasks_method::PLAN_QUEUE_JSON_V1, |state, req| {
            state.plan_queue(req)
        })
        .blob(tasks_method::INVOKE_JSON, tasks_invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.tasks",
        service_kind: newengine_service_api::EngineServiceKind::Tasks,
        provider_service: TASKS_SERVICE_ID,
        provider_route: crate::TASKS_PROVIDER_ROUTE,
        capability: TASKS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service,
    })
    .is_ok()
}

fn register_animation_gateway() -> bool {
    let service = JsonServiceRouter::with_shared_state(ANIMATION_SERVICE_ID, state())
        .describe_json(&service_description(
            ANIMATION_SERVICE_ID,
            crate::ANIMATION_PROVIDER_ROUTE,
            "engine.animation",
            ANIMATION_BACKEND_CAPABILITY_ID,
            ANIMATION_SERVICE_METHODS,
        ))
        .get_json(animation_method::INFO_JSON, |state| state.animation_info())
        .post_json(animation_method::DESCRIBE_GRAPHS_JSON_V1, |state, req| {
            state.describe_animation_graphs(req)
        })
        .post_json(animation_method::PLAN_JSON_V1, |state, req| {
            state.plan_animation(req)
        })
        .post_json(animation_method::VALIDATE_INTENT_JSON_V1, |state, req| {
            state.validate_animation_intent(req)
        })
        .blob(animation_method::INVOKE_JSON, animation_invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.animation",
        service_kind: newengine_service_api::EngineServiceKind::Animation,
        provider_service: ANIMATION_SERVICE_ID,
        provider_route: crate::ANIMATION_PROVIDER_ROUTE,
        capability: ANIMATION_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service,
    })
    .is_ok()
}

fn register_navigation_gateway() -> bool {
    let service = JsonServiceRouter::with_shared_state(NAVIGATION_SERVICE_ID, state())
        .describe_json(&service_description(
            NAVIGATION_SERVICE_ID,
            crate::NAVIGATION_PROVIDER_ROUTE,
            "engine.navigation",
            NAVIGATION_BACKEND_CAPABILITY_ID,
            NAVIGATION_SERVICE_METHODS,
        ))
        .get_json(navigation_method::INFO_JSON, |state| {
            state.navigation_info()
        })
        .post_json(navigation_method::PLAN_PATH_JSON_V1, |state, req| {
            state.plan_path(req)
        })
        .post_json(navigation_method::PROJECT_POINT_JSON_V1, |state, req| {
            state.project_point(req)
        })
        .post_json(navigation_method::QUERY_STATUS_JSON_V1, |state, req| {
            state.query_status(req)
        })
        .blob(navigation_method::INVOKE_JSON, navigation_invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.navigation",
        service_kind: newengine_service_api::EngineServiceKind::Navigation,
        provider_service: NAVIGATION_SERVICE_ID,
        provider_route: crate::NAVIGATION_PROVIDER_ROUTE,
        capability: NAVIGATION_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service,
    })
    .is_ok()
}

fn register_ai_gateway() -> bool {
    let service = JsonServiceRouter::with_shared_state(AI_SERVICE_ID, state())
        .describe_json(&service_description(
            AI_SERVICE_ID,
            crate::AI_PROVIDER_ROUTE,
            "engine.ai",
            AI_BACKEND_CAPABILITY_ID,
            AI_SERVICE_METHODS,
        ))
        .get_json(ai_method::INFO_JSON, |state| state.ai_info())
        .post_json(ai_method::FRAME_JSON_V1, |state, req| state.ai_frame(req))
        .post_json(ai_method::VALIDATE_INTENTS_JSON_V1, |state, req| {
            state.validate_ai_intents(req)
        })
        .get_json(
            ai_method::DECISION_TRACE_JSON_V1,
            |_state| serde_json::json!({ "provider": crate::AI_PROVIDER_ROUTE, "trace": [] }),
        )
        .blob(ai_method::INVOKE_JSON, ai_invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.ai",
        service_kind: newengine_service_api::EngineServiceKind::Ai,
        provider_service: AI_SERVICE_ID,
        provider_route: crate::AI_PROVIDER_ROUTE,
        capability: AI_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service,
    })
    .is_ok()
}

pub fn register_gameplay_foundation_gateways_best_effort() -> bool {
    let tags = register_tags_gateway();
    let tasks = register_tasks_gateway();
    let animation = register_animation_gateway();
    let navigation = register_navigation_gateway();
    let ai = register_ai_gateway();
    tags && tasks && animation && navigation && ai
}

const _P6_AI_FRAME_OUTPUT_DTO_MARKER: Option<fn() -> Option<AiFrameOutputV1>> = None;
