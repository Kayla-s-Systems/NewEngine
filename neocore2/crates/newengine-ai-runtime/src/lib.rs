#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_ai_api::{
    ai_method, AiDecisionTraceV1, AiFrameInputV1, AiFrameOutputV1, AiIntentDtoV1, AiIntentKind,
    AiServiceInfoV1, AiValidateIntentsRequestV1, AiValidateIntentsResponseV1,
    AI_BACKEND_CAPABILITY_ID, AI_SERVICE_ID, AI_SERVICE_METHODS,
};
use newengine_animation_api::{
    AnimationClipRef, AnimationGraphRef, AnimationIntentDtoV1, AnimationIntentKind,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_tasks_api::{TaskId, TaskRequestDtoV1};
use serde_json::Value;

pub const PROVIDER_ROUTE: &str = "engine.ai.foundation";
const OWNER: &str = "newengine-ai-runtime.foundation-provider";
#[derive(Default)]
struct AiState;
impl AiState {
    fn info(&self) -> AiServiceInfoV1 {
        AiServiceInfoV1::default()
    }
    fn frame(&self, input: AiFrameInputV1) -> AiFrameOutputV1 {
        let mut intents = Vec::new();
        let mut trace = Vec::new();
        for agent in input.agents {
            let alert = agent.tags.iter().any(|tag| tag.as_str() == "state.alert");
            let idle = agent.tags.iter().any(|tag| tag.as_str() == "state.idle");
            let kind = if alert {
                AiIntentKind::RequestTask
            } else {
                AiIntentKind::Idle
            };
            let task = if alert {
                Some(TaskRequestDtoV1 {
                    task: TaskId::new("move_to"),
                    issuer: Some(agent.entity),
                    target: None,
                    priority: 100,
                    parameters: serde_json::json!({"reason":"alert-agent-foundation-intent"}),
                    tags: agent.tags.clone(),
                })
            } else {
                None
            };
            intents.push(AiIntentDtoV1 {
                intent_id: format!("ai.intent.{}.{}", input.fixed_tick, agent.agent_id),
                agent: agent.entity,
                kind,
                target_position: agent.position,
                path: None,
                task,
                animation: idle.then(|| AnimationIntentDtoV1 {
                    entity: agent.entity,
                    intent: AnimationIntentKind::PlayClip,
                    graph: Some(AnimationGraphRef("humanoid.locomotion".to_owned())),
                    clip: Some(AnimationClipRef("idle".to_owned())),
                    task: None,
                    tags: agent.tags.clone(),
                    parameters: serde_json::json!({}),
                }),
                tags: agent.tags.clone(),
                payload: serde_json::json!({"apply_stage_required":true}),
            });
            trace.push(AiDecisionTraceV1 {
                agent: agent.entity,
                selected_pattern: if alert {
                    "foundation.alert.request_task".to_owned()
                } else {
                    "foundation.idle".to_owned()
                },
                score: if alert { 1.0 } else { 0.1 },
                notes: vec![
                    "AI emitted intent DTO only; runtime apply stage owns mutation.".to_owned(),
                ],
            });
        }
        AiFrameOutputV1 {
            accepted: true,
            fixed_tick: input.fixed_tick,
            intents,
            decision_trace: trace,
            diagnostics: Vec::new(),
        }
    }
    fn validate(&self, req: AiValidateIntentsRequestV1) -> AiValidateIntentsResponseV1 {
        AiValidateIntentsResponseV1 {
            accepted: true,
            intents: req.intents,
            rejected: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
fn envelope(payload: &Blob, default_method: &str) -> Result<(String, Value), RString> {
    let value = payload_json(payload).map_err(RString::from)?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(default_method)
        .to_owned();
    Ok((method, value.get("request").cloned().unwrap_or(Value::Null)))
}
fn invoke(state: &mut AiState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, ai_method::FRAME_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    let decode = |e: serde_json::Error| RResult::RErr(RString::from(e.to_string()));
    match method.as_str() {
        ai_method::INFO_JSON => ok_json(state.info()),
        ai_method::FRAME_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.frame(v)),
            Err(e) => decode(e),
        },
        ai_method::VALIDATE_INTENTS_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.validate(v)),
            Err(e) => decode(e),
        },
        ai_method::DECISION_TRACE_JSON_V1 => {
            ok_json(serde_json::json!({"provider":PROVIDER_ROUTE,"trace":[]}))
        }
        other => RResult::RErr(RString::from(format!(
            "{AI_SERVICE_ID}: unknown invoke method '{other}'"
        ))),
    }
}
pub fn register_ai_gateway_best_effort() -> bool {
    let description=engine_gateway_provider_service_description(AI_SERVICE_ID,PROVIDER_ROUTE,AI_BACKEND_CAPABILITY_ID,AI_SERVICE_METHODS.iter().copied()).gateway("engine.ai").protocol("newengine.ai.foundation/v1").features(["single-purpose-provider","replaceable-gateway-route","intent-only-boundary"]).notes("Owns only baseline AI intent generation. It never owns world/ECS mutation, navigation, task execution, or animation application.");
    let service = JsonServiceRouter::with_state(AI_SERVICE_ID, AiState)
        .describe_json(&description)
        .get_json(ai_method::INFO_JSON, |state| state.info())
        .post_json(ai_method::FRAME_JSON_V1, |state, req| state.frame(req))
        .post_json(ai_method::VALIDATE_INTENTS_JSON_V1, |state, req| {
            state.validate(req)
        })
        .get_json(
            ai_method::DECISION_TRACE_JSON_V1,
            |_state| serde_json::json!({"provider":PROVIDER_ROUTE,"trace":[]}),
        )
        .blob(ai_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.ai",
        service_kind: newengine_service_api::EngineServiceKind::Ai,
        provider_service: AI_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: AI_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service,
    })
    .is_ok()
}
const _AI_FRAME_OUTPUT_DTO_MARKER: Option<fn() -> Option<AiFrameOutputV1>> = None;

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.ai",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_ai_api::AI_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_ai_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
