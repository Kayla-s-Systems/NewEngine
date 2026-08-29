pub const DEFAULT_ANIMATION_GRAPH_INTENT_BLEND_SECONDS: f32 = 0.12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationGraphIntentApplyResult {
    SetParameters { count: usize },
    BlendToState {
        state_index: usize,
        blend_seconds: f32,
    },
}

fn validate_intent_graph_ref(
    graph: &CompiledAnimationGraph,
    intent: &newengine_animation_api::AnimationIntentDtoV1,
) -> Result<(), String> {
    let requested = intent
        .graph
        .as_ref()
        .map(|reference| reference.0.trim())
        .filter(|reference| !reference.is_empty())
        .ok_or_else(|| format!("animation graph intent requires graph='{}'", graph.name()))?;
    if !requested.eq_ignore_ascii_case(graph.name()) {
        return Err(format!(
            "animation graph intent targets '{}' but instance owns '{}'",
            requested,
            graph.name()
        ));
    }
    Ok(())
}

fn json_parameter_value(
    graph: &CompiledAnimationGraph,
    parameter_index: usize,
    name: &str,
    value: &serde_json::Value,
) -> Result<AnimationGraphParameterValue, String> {
    match graph.parameters.get(parameter_index).map(|definition| definition.default) {
        Some(AnimationGraphParameterValue::Bool(_)) => value
            .as_bool()
            .map(AnimationGraphParameterValue::Bool)
            .ok_or_else(|| {
                format!(
                    "animation graph '{}' parameter '{}' expects bool",
                    graph.name(),
                    name
                )
            }),
        Some(AnimationGraphParameterValue::Float(_)) => {
            let value = value.as_f64().ok_or_else(|| {
                format!(
                    "animation graph '{}' parameter '{}' expects number",
                    graph.name(),
                    name
                )
            })? as f32;
            if !value.is_finite() {
                return Err(format!(
                    "animation graph '{}' parameter '{}' is non-finite",
                    graph.name(),
                    name
                ));
            }
            Ok(AnimationGraphParameterValue::Float(value))
        }
        None => Err("animation graph parameter/runtime shape mismatch".to_owned()),
    }
}

fn apply_set_parameter_intent(
    graph: &CompiledAnimationGraph,
    instance: &mut AnimationGraphInstance,
    parameters: &serde_json::Value,
) -> Result<AnimationGraphIntentApplyResult, String> {
    let object = parameters.as_object().ok_or_else(|| {
        format!(
            "animation graph '{}' SetParameter payload must be a JSON object",
            graph.name()
        )
    })?;
    if object.is_empty() {
        return Err(format!(
            "animation graph '{}' SetParameter payload is empty",
            graph.name()
        ));
    }

    // Validate the complete batch first. A bad second field must never leave the first field applied.
    let mut pending = Vec::with_capacity(object.len());
    for (name, value) in object {
        let index = graph.parameter_index(name).ok_or_else(|| {
            format!(
                "animation graph '{}' has no parameter '{}'",
                graph.name(),
                name
            )
        })?;
        pending.push((index, json_parameter_value(graph, index, name, value)?));
    }
    for (index, value) in pending.iter().copied() {
        let slot = instance
            .parameters
            .get_mut(index)
            .ok_or_else(|| "animation graph parameter/runtime shape mismatch".to_owned())?;
        *slot = value;
    }
    Ok(AnimationGraphIntentApplyResult::SetParameters {
        count: pending.len(),
    })
}

fn apply_blend_to_state_intent(
    graph: &CompiledAnimationGraph,
    instance: &mut AnimationGraphInstance,
    intent: &newengine_animation_api::AnimationIntentDtoV1,
) -> Result<AnimationGraphIntentApplyResult, String> {
    let state = intent
        .parameters
        .get("state")
        .and_then(serde_json::Value::as_str)
        .or_else(|| intent.clip.as_ref().map(|clip| clip.0.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "animation graph '{}' BlendToState requires parameters.state",
                graph.name()
            )
        })?;
    let blend_seconds = match intent.parameters.get("blend_seconds") {
        Some(value) => value.as_f64().ok_or_else(|| {
            format!(
                "animation graph '{}' BlendToState blend_seconds must be numeric",
                graph.name()
            )
        })? as f32,
        None => DEFAULT_ANIMATION_GRAPH_INTENT_BLEND_SECONDS,
    };
    if !blend_seconds.is_finite() || !(0.0..=60.0).contains(&blend_seconds) {
        return Err(format!(
            "animation graph '{}' BlendToState blend_seconds is invalid value={blend_seconds}",
            graph.name()
        ));
    }
    let state_index = graph.state_index(state).ok_or_else(|| {
        format!("animation graph '{}' has no state '{state}'", graph.name())
    })?;
    instance.blend_to_state(graph, state, blend_seconds)?;
    Ok(AnimationGraphIntentApplyResult::BlendToState {
        state_index,
        blend_seconds,
    })
}

/// Applies provider-normalized semantic intents to one graph instance.
///
/// This bridge deliberately owns no ECS/world access. The caller resolves entity->instance and
/// graph ownership; the generic animation runtime only performs validated graph mutation.
pub fn apply_animation_intent_to_graph_instance(
    graph: &CompiledAnimationGraph,
    instance: &mut AnimationGraphInstance,
    intent: &newengine_animation_api::AnimationIntentDtoV1,
) -> Result<AnimationGraphIntentApplyResult, String> {
    validate_intent_graph_ref(graph, intent)?;
    match intent.intent {
        newengine_animation_api::AnimationIntentKind::SetParameter => {
            apply_set_parameter_intent(graph, instance, &intent.parameters)
        }
        newengine_animation_api::AnimationIntentKind::BlendToState => {
            apply_blend_to_state_intent(graph, instance, intent)
        }
        _ => Err(format!(
            "animation graph '{}' intent bridge does not apply kind '{:?}'",
            graph.name(),
            intent.intent
        )),
    }
}
