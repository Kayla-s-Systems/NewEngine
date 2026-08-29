#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_animation_api::{
    animation_method, AnimationDescribeGraphsRequestV1, AnimationDescribeGraphsResponseV1,
    AnimationGraphDescriptorV1, AnimationGraphRef, AnimationIntentDtoV1, AnimationIntentKind,
    AnimationPlanRequestV1, AnimationPlanResponseV1, AnimationServiceInfoV1,
    AnimationValidateIntentRequestV1, AnimationValidateIntentResponseV1,
    ANIMATION_BACKEND_CAPABILITY_ID, ANIMATION_SERVICE_ID, ANIMATION_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use newengine_tags_api::TagId;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

pub const PROVIDER_ROUTE: &str = "engine.animation.foundation";
const OWNER: &str = "newengine-animation-foundation-runtime.foundation-provider";
const CONFIG_JSON: &str = include_str!("../../../config/gameplay/gameplay_foundation.v1.json");

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    animation_graphs: Vec<RawGraph>,
}

#[derive(Debug, Deserialize)]
struct RawGraph {
    graph: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    states: Vec<String>,
}

#[derive(Default)]
struct AnimationState {
    graphs: Vec<AnimationGraphDescriptorV1>,
    config_diagnostics: Vec<String>,
}

#[inline]
fn canonical_text(value: &str) -> String {
    value.trim().to_owned()
}

fn canonical_tags(tags: Vec<String>, diagnostics: &mut Vec<String>, graph: &str) -> Vec<TagId> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in tags {
        let tag = canonical_text(&raw);
        if tag.is_empty() {
            diagnostics.push(format!(
                "animation graph '{graph}' contains an empty tag declaration"
            ));
            continue;
        }
        if seen.insert(tag.clone()) {
            out.push(TagId::new(tag));
        }
    }
    out
}

fn canonical_states(
    states: Vec<String>,
    diagnostics: &mut Vec<String>,
    graph: &str,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in states {
        let state = canonical_text(&raw);
        if state.is_empty() {
            diagnostics.push(format!(
                "animation graph '{graph}' contains an empty state declaration"
            ));
            continue;
        }
        let key = state.to_ascii_lowercase();
        if !seen.insert(key) {
            diagnostics.push(format!(
                "animation graph '{graph}' contains duplicate state '{state}'"
            ));
            continue;
        }
        out.push(state);
    }
    out
}

impl AnimationState {
    fn load() -> Self {
        let config: Config = match serde_json::from_str(CONFIG_JSON) {
            Ok(config) => config,
            Err(error) => {
                return Self {
                    graphs: Vec::new(),
                    config_diagnostics: vec![format!(
                        "animation foundation config decode failed: {error}"
                    )],
                };
            }
        };

        let mut diagnostics = Vec::new();
        let mut graph_keys = HashSet::new();
        let mut graphs = Vec::with_capacity(config.animation_graphs.len());
        for raw in config.animation_graphs {
            let graph = canonical_text(&raw.graph);
            if graph.is_empty() {
                diagnostics.push("animation graph declaration has an empty graph id".to_owned());
                continue;
            }
            let graph_key = graph.to_ascii_lowercase();
            if !graph_keys.insert(graph_key) {
                diagnostics.push(format!("duplicate animation graph id '{graph}'"));
                continue;
            }

            let tags = canonical_tags(raw.tags, &mut diagnostics, &graph);
            let states = canonical_states(raw.states, &mut diagnostics, &graph);
            let display_name = {
                let value = canonical_text(&raw.display_name);
                if value.is_empty() {
                    graph.clone()
                } else {
                    value
                }
            };
            graphs.push(AnimationGraphDescriptorV1 {
                graph: AnimationGraphRef(graph),
                display_name,
                tags,
                states,
            });
        }

        if graphs.is_empty() {
            diagnostics.push("animation foundation contains no usable animation graphs".to_owned());
        }
        Self {
            graphs,
            config_diagnostics: diagnostics,
        }
    }

    fn info(&self) -> AnimationServiceInfoV1 {
        let mut info = AnimationServiceInfoV1::default();
        info.features.extend([
            "graph-validation".to_owned(),
            "graph-tag-filter".to_owned(),
            "intent-normalization".to_owned(),
            "atomic-plan-validation".to_owned(),
        ]);
        info
    }

    fn graph(&self, graph: &str) -> Option<&AnimationGraphDescriptorV1> {
        self.graphs
            .iter()
            .find(|candidate| candidate.graph.0.eq_ignore_ascii_case(graph))
    }

    fn canonical_graph_ref(&self, graph: &str) -> Result<AnimationGraphRef, String> {
        let graph = canonical_text(graph);
        if graph.is_empty() {
            return Err("animation graph reference is empty".to_owned());
        }
        let descriptor = self
            .graph(&graph)
            .ok_or_else(|| format!("unknown animation graph '{graph}'"))?;
        Ok(descriptor.graph.clone())
    }

    fn canonical_state(&self, graph: &AnimationGraphRef, state: &str) -> Result<String, String> {
        let state = canonical_text(state);
        if state.is_empty() {
            return Err(format!(
                "animation graph '{}' requires a non-empty state",
                graph.0
            ));
        }
        let descriptor = self
            .graph(&graph.0)
            .ok_or_else(|| format!("unknown animation graph '{}'", graph.0))?;
        descriptor
            .states
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(&state))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "animation graph '{}' does not declare state '{}'",
                    graph.0, state
                )
            })
    }

    fn normalize_tags(tags: &mut Vec<TagId>) {
        let mut seen = HashSet::new();
        tags.retain_mut(|tag| {
            tag.0 = canonical_text(&tag.0);
            !tag.0.is_empty() && seen.insert(tag.0.clone())
        });
    }

    fn validate_and_normalize(
        &self,
        mut intent: AnimationIntentDtoV1,
    ) -> Result<AnimationIntentDtoV1, String> {
        if !self.config_diagnostics.is_empty() {
            return Err(format!(
                "animation foundation configuration is invalid: {}",
                self.config_diagnostics.join("; ")
            ));
        }

        if let Some(graph) = intent.graph.as_mut() {
            *graph = self.canonical_graph_ref(&graph.0)?;
        }
        if let Some(clip) = intent.clip.as_mut() {
            clip.0 = canonical_text(&clip.0);
            if clip.0.is_empty() {
                intent.clip = None;
            }
        }
        if let Some(task) = intent.task.as_mut() {
            task.0 = canonical_text(&task.0);
            if task.0.is_empty() {
                intent.task = None;
            }
        }
        Self::normalize_tags(&mut intent.tags);

        match &mut intent.intent {
            AnimationIntentKind::PlayClip => {
                if intent.clip.is_none() {
                    return Err("PlayClip requires a non-empty clip reference".to_owned());
                }
            }
            AnimationIntentKind::Stop => {
                // Missing graph means stop all animation owned by this entity. A supplied
                // graph was canonicalized above and therefore is known to the provider.
            }
            AnimationIntentKind::BlendToState => {
                let graph = intent
                    .graph
                    .as_ref()
                    .ok_or_else(|| "BlendToState requires an animation graph".to_owned())?;
                let requested_state = intent
                    .parameters
                    .get("state")
                    .and_then(Value::as_str)
                    .or_else(|| intent.clip.as_ref().map(|clip| clip.0.as_str()))
                    .ok_or_else(|| {
                        "BlendToState requires parameters.state or a state clip reference"
                            .to_owned()
                    })?;
                let state = self.canonical_state(graph, requested_state)?;
                let object = intent
                    .parameters
                    .as_object_mut()
                    .ok_or_else(|| "BlendToState parameters must be a JSON object".to_owned())?;
                object.insert("state".to_owned(), Value::String(state));
            }
            AnimationIntentKind::SetParameter => {
                if intent.graph.is_none() {
                    return Err("SetParameter requires an animation graph".to_owned());
                }
                if !intent
                    .parameters
                    .as_object()
                    .is_some_and(|parameters| !parameters.is_empty())
                {
                    return Err(
                        "SetParameter requires a non-empty JSON parameter object".to_owned()
                    );
                }
            }
            AnimationIntentKind::AttachTask => {
                if intent.task.is_none() {
                    return Err("AttachTask requires a non-empty task id".to_owned());
                }
            }
            AnimationIntentKind::Custom(name) => {
                *name = canonical_text(name);
                if name.is_empty() {
                    return Err("Custom animation intent requires a non-empty kind".to_owned());
                }
            }
        }

        Ok(intent)
    }

    fn describe(&self, req: AnimationDescribeGraphsRequestV1) -> AnimationDescribeGraphsResponseV1 {
        if !self.config_diagnostics.is_empty() {
            return AnimationDescribeGraphsResponseV1 {
                accepted: false,
                graphs: Vec::new(),
                diagnostics: self.config_diagnostics.clone(),
            };
        }

        let mut tag_filter = req.tag_filter;
        Self::normalize_tags(&mut tag_filter);
        let graphs = self
            .graphs
            .iter()
            .filter(|graph| {
                tag_filter
                    .iter()
                    .all(|required| graph.tags.iter().any(|tag| tag == required))
            })
            .cloned()
            .collect();
        AnimationDescribeGraphsResponseV1 {
            accepted: true,
            graphs,
            diagnostics: Vec::new(),
        }
    }

    fn plan(&self, req: AnimationPlanRequestV1) -> AnimationPlanResponseV1 {
        if !self.config_diagnostics.is_empty() {
            return AnimationPlanResponseV1 {
                accepted: false,
                accepted_intents: Vec::new(),
                diagnostics: self.config_diagnostics.clone(),
            };
        }

        let mut accepted_intents = Vec::with_capacity(req.intents.len());
        let mut diagnostics = Vec::new();
        for (index, intent) in req.intents.into_iter().enumerate() {
            match self.validate_and_normalize(intent) {
                Ok(intent) => accepted_intents.push(intent),
                Err(error) => diagnostics.push(format!("intent[{index}]: {error}")),
            }
        }
        if diagnostics.is_empty() {
            AnimationPlanResponseV1 {
                accepted: true,
                accepted_intents,
                diagnostics,
            }
        } else {
            // Planning is deliberately atomic. Apply stages must never receive a partial
            // animation command batch because cross-layer graph/task ordering is significant.
            AnimationPlanResponseV1 {
                accepted: false,
                accepted_intents: Vec::new(),
                diagnostics,
            }
        }
    }

    fn validate(&self, req: AnimationValidateIntentRequestV1) -> AnimationValidateIntentResponseV1 {
        match self.validate_and_normalize(req.intent) {
            Ok(intent) => AnimationValidateIntentResponseV1 {
                accepted: true,
                normalized: Some(intent),
                diagnostics: Vec::new(),
            },
            Err(error) => AnimationValidateIntentResponseV1 {
                accepted: false,
                normalized: None,
                diagnostics: vec![error],
            },
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

fn invoke(state: &mut AnimationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, animation_method::DESCRIBE_GRAPHS_JSON_V1) {
        Ok(value) => value,
        Err(error) => return RResult::RErr(error),
    };
    let decode = |error: serde_json::Error| RResult::RErr(RString::from(error.to_string()));
    match method.as_str() {
        animation_method::INFO_JSON => ok_json(state.info()),
        animation_method::DESCRIBE_GRAPHS_JSON_V1 => match serde_json::from_value(request) {
            Ok(value) => ok_json(state.describe(value)),
            Err(error) => decode(error),
        },
        animation_method::PLAN_JSON_V1 => match serde_json::from_value(request) {
            Ok(value) => ok_json(state.plan(value)),
            Err(error) => decode(error),
        },
        animation_method::VALIDATE_INTENT_JSON_V1 => match serde_json::from_value(request) {
            Ok(value) => ok_json(state.validate(value)),
            Err(error) => decode(error),
        },
        other => RResult::RErr(RString::from(format!(
            "animation.api: unknown invoke method '{other}'"
        ))),
    }
}

pub fn register_animation_gateway_best_effort() -> bool {
    let description = engine_gateway_provider_service_description(
        ANIMATION_SERVICE_ID,
        PROVIDER_ROUTE,
        ANIMATION_BACKEND_CAPABILITY_ID,
        ANIMATION_SERVICE_METHODS.iter().copied(),
    )
    .gateway("engine.animation")
    .protocol("newengine.animation.foundation/v1")
    .features([
        "single-purpose-provider",
        "replaceable-gateway-route",
        "dto-only-boundary",
        "graph-validation",
        "intent-normalization",
        "atomic-plan-validation",
    ])
    .notes("Owns baseline animation graph/intent semantics and validation only; skeletal clip decoding/evaluation remains in newengine-animation-runtime and apply stages own world mutation.");
    let service = JsonServiceRouter::with_state(ANIMATION_SERVICE_ID, AnimationState::load())
        .describe_json(&description)
        .get_json(animation_method::INFO_JSON, |state| state.info())
        .post_json(animation_method::DESCRIBE_GRAPHS_JSON_V1, |state, req| {
            state.describe(req)
        })
        .post_json(animation_method::PLAN_JSON_V1, |state, req| state.plan(req))
        .post_json(animation_method::VALIDATE_INTENT_JSON_V1, |state, req| {
            state.validate(req)
        })
        .blob(animation_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.animation",
        service_kind: newengine_service_api::EngineServiceKind::Animation,
        provider_service: ANIMATION_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: ANIMATION_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service,
    })
    .is_ok()
}

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.animation",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_animation_api::ANIMATION_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_animation_gateway_best_effort();
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
    use newengine_animation_api::AnimationClipRef;

    fn intent(kind: AnimationIntentKind) -> AnimationIntentDtoV1 {
        AnimationIntentDtoV1 {
            entity: Default::default(),
            intent: kind,
            graph: None,
            clip: None,
            task: None,
            tags: Vec::new(),
            parameters: Value::Object(Default::default()),
        }
    }

    #[test]
    fn state_loads_valid_animation_domain() {
        let state = AnimationState::load();
        assert!(
            state.config_diagnostics.is_empty(),
            "{:?}",
            state.config_diagnostics
        );
        assert!(!state.graphs.is_empty());
    }

    #[test]
    fn describe_graphs_honors_tag_filter() {
        let state = AnimationState::load();
        let response = state.describe(AnimationDescribeGraphsRequestV1 {
            tag_filter: vec![TagId::new("weapon.firearm")],
        });
        assert!(response.accepted);
        assert_eq!(response.graphs.len(), 1);
        assert_eq!(response.graphs[0].graph.0, "humanoid.upper_body");
    }

    #[test]
    fn play_clip_is_normalized_and_unknown_graph_is_rejected() {
        let state = AnimationState::load();
        let mut play = intent(AnimationIntentKind::PlayClip);
        play.graph = Some(AnimationGraphRef("  HUMANOID.LOCOMOTION  ".to_owned()));
        play.clip = Some(AnimationClipRef("  idle  ".to_owned()));
        let valid = state.validate(AnimationValidateIntentRequestV1 { intent: play });
        assert!(valid.accepted, "{:?}", valid.diagnostics);
        let normalized = valid.normalized.expect("normalized intent");
        assert_eq!(normalized.graph.unwrap().0, "humanoid.locomotion");
        assert_eq!(normalized.clip.unwrap().0, "idle");

        let mut invalid = intent(AnimationIntentKind::PlayClip);
        invalid.graph = Some(AnimationGraphRef("missing.graph".to_owned()));
        invalid.clip = Some(AnimationClipRef("idle".to_owned()));
        let invalid = state.validate(AnimationValidateIntentRequestV1 { intent: invalid });
        assert!(!invalid.accepted);
        assert!(invalid.diagnostics[0].contains("unknown animation graph"));
    }

    #[test]
    fn plan_is_atomic_when_any_intent_is_invalid() {
        let state = AnimationState::load();
        let mut valid = intent(AnimationIntentKind::PlayClip);
        valid.graph = Some(AnimationGraphRef("humanoid.locomotion".to_owned()));
        valid.clip = Some(AnimationClipRef("idle".to_owned()));
        let invalid = intent(AnimationIntentKind::PlayClip);
        let response = state.plan(AnimationPlanRequestV1 {
            intents: vec![valid, invalid],
        });
        assert!(!response.accepted);
        assert!(response.accepted_intents.is_empty());
        assert_eq!(response.diagnostics.len(), 1);
        assert!(response.diagnostics[0].starts_with("intent[1]:"));
    }

    #[test]
    fn blend_state_and_task_contracts_are_validated() {
        let state = AnimationState::load();
        let mut blend = intent(AnimationIntentKind::BlendToState);
        blend.graph = Some(AnimationGraphRef("humanoid.upper_body".to_owned()));
        blend.parameters = serde_json::json!({"state":"AIM"});
        let blend = state.validate(AnimationValidateIntentRequestV1 { intent: blend });
        assert!(blend.accepted, "{:?}", blend.diagnostics);
        assert_eq!(
            blend.normalized.unwrap().parameters["state"],
            Value::String("aim".to_owned())
        );

        let mut task = intent(AnimationIntentKind::AttachTask);
        task.task = Some(
            serde_json::from_value(Value::String("  combat.reload  ".to_owned())).expect("task id"),
        );
        let task = state.validate(AnimationValidateIntentRequestV1 { intent: task });
        assert!(task.accepted, "{:?}", task.diagnostics);
        assert_eq!(
            task.normalized.unwrap().task.unwrap().as_str(),
            "combat.reload"
        );
    }
}
