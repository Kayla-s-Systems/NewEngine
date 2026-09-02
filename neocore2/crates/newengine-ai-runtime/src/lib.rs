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
            let visible_target = agent
                .visible_facts
                .iter()
                .find(|fact| fact.fact_id == "combat.target.visible")
                .and_then(|fact| {
                    let target = fact.value.get("target")?.as_u64()?;
                    let position = fact.value.get("position").cloned().unwrap_or(Value::Null);
                    Some((target, position))
                });
            let combat_memory = agent.blackboard.get("combat");
            let memory_target = combat_memory
                .and_then(|value| value.get("memory_target"))
                .and_then(Value::as_u64);
            let memory_position = combat_memory
                .and_then(|value| value.get("last_known_position"))
                .cloned()
                .unwrap_or(Value::Null);

            let alert = agent.tags.iter().any(|tag| tag.as_str() == "state.alert");
            let idle = agent.tags.iter().any(|tag| tag.as_str() == "state.idle");
            let (kind, task, payload, selected_pattern, score) = if let Some((target, position)) =
                visible_target
            {
                (
                    AiIntentKind::Custom("combat.engage".to_owned()),
                    None,
                    serde_json::json!({
                        "target": target,
                        "target_position": position,
                        "reason": "visible-hostile-target",
                    }),
                    "foundation.combat.engage",
                    1.0,
                )
            } else if let Some(target) = memory_target {
                (
                    AiIntentKind::Custom("combat.investigate".to_owned()),
                    None,
                    serde_json::json!({
                        "target": target,
                        "target_position": memory_position,
                        "reason": "target-memory",
                    }),
                    "foundation.combat.investigate",
                    0.65,
                )
            } else if alert {
                (
                    AiIntentKind::RequestTask,
                    Some(TaskRequestDtoV1 {
                        task: TaskId::new("move_to"),
                        issuer: Some(agent.entity),
                        target: None,
                        priority: 100,
                        parameters: serde_json::json!({"reason":"alert-agent-foundation-intent"}),
                        tags: agent.tags.clone(),
                    }),
                    serde_json::json!({"apply_stage_required":true}),
                    "foundation.alert.request_task",
                    1.0,
                )
            } else {
                (
                    AiIntentKind::Idle,
                    None,
                    serde_json::json!({"apply_stage_required":true}),
                    "foundation.idle",
                    0.1,
                )
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
                payload,
            });
            trace.push(AiDecisionTraceV1 {
                agent: agent.entity,
                selected_pattern: selected_pattern.to_owned(),
                score,
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

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ai_api::{AiAgentSnapshotV1, AiFrameInputV1, AiPerceptionFactV1};

    fn agent(
        visible_facts: Vec<AiPerceptionFactV1>,
        blackboard: serde_json::Value,
    ) -> AiAgentSnapshotV1 {
        AiAgentSnapshotV1 {
            entity: Default::default(),
            agent_id: "test-agent".to_owned(),
            position: None,
            velocity: None,
            tags: Vec::new(),
            current_task: None,
            visible_facts,
            blackboard,
        }
    }

    fn decide(agent: AiAgentSnapshotV1) -> AiIntentKind {
        AiState
            .frame(AiFrameInputV1 {
                frame_id: 1,
                fixed_tick: 1,
                seed: 7,
                agents: vec![agent],
                world_facts: Vec::new(),
            })
            .intents
            .into_iter()
            .next()
            .expect("AI intent")
            .kind
    }

    #[test]
    fn visible_combat_fact_selects_engage() {
        let kind = decide(agent(
            vec![AiPerceptionFactV1 {
                fact_id: "combat.target.visible".to_owned(),
                tags: Vec::new(),
                value: serde_json::json!({
                    "target": 42,
                    "position": [1.0, 2.0, 3.0],
                    "distance": 5.0,
                }),
            }],
            serde_json::json!({}),
        ));
        assert_eq!(kind, AiIntentKind::Custom("combat.engage".to_owned()));
    }

    #[test]
    fn target_memory_without_visibility_selects_investigate() {
        let kind = decide(agent(
            Vec::new(),
            serde_json::json!({
                "combat": {
                    "memory_target": 42,
                    "memory_visible": false,
                    "last_known_position": [4.0, 0.0, -2.0],
                    "seconds_since_seen": 0.4,
                }
            }),
        ));
        assert_eq!(kind, AiIntentKind::Custom("combat.investigate".to_owned()));
    }

    #[test]
    fn no_perception_or_memory_selects_idle() {
        assert_eq!(
            decide(agent(Vec::new(), serde_json::json!({}))),
            AiIntentKind::Idle
        );
    }
}
