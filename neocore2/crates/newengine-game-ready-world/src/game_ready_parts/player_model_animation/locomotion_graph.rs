const GAME_READY_WORLD_RUNTIME_DATA_OWNER: &str = "engine.game-ready.world";
const LOCOMOTION_GRAPH_RUNTIME_DATA_KEY: &str = "locomotion_graph";

#[inline]
fn graph_alias_fallback_slots(
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> &'static [usize] {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match state {
        L::Idle => &[0],
        L::Walk => &[1, 0],
        L::Run => &[2, 1, 0],
        L::Sprint => &[3, 2, 1, 0],
        L::CrouchIdle => &[4, 0],
        L::CrouchWalk => &[5, 4, 1, 0],
        L::Jump => &[6, 2, 0],
        L::Fall => &[7, 6, 2, 0],
    }
}

fn resolve_locomotion_slot(
    clips: &[Option<PlayerAnimationRuntimeClip>; 8],
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> usize {
    graph_alias_fallback_slots(state)
        .iter()
        .copied()
        .find(|slot| clips[*slot].is_some())
        .unwrap_or(0)
}

/// Runtime state selection is strict: an unavailable authored state is unsupported, not an
/// invitation to play another locomotion clip. Graph aliases retain deterministic compile-time
/// completion only because the static graph declares all eight slots.
fn resolve_runtime_locomotion_slot(
    clips: &[Option<PlayerAnimationRuntimeClip>; 8],
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> Option<usize> {
    let slot = locomotion_slot(state);
    clips[slot].as_ref().map(|_| slot)
}

#[inline]
fn locomotion_state_for_slot(slot: usize) -> &'static str {
    match slot {
        0 => "Idle",
        1 => "Walk",
        2 => "Run",
        3 => "Sprint",
        4 => "CrouchIdle",
        5 => "CrouchWalk",
        6 => "Jump",
        7 => "Fall",
        _ => "Idle",
    }
}

fn locomotion_state_for_alias(
    reference: &str,
) -> Result<newengine_engine_runtime::gameplay::PlayerLocomotionAnimation, String> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    let alias = reference.trim().replace('\\', "/").to_ascii_lowercase();
    match alias.as_str() {
        "slot://idle" => Ok(L::Idle),
        "slot://walk" => Ok(L::Walk),
        "slot://run" => Ok(L::Run),
        "slot://sprint" => Ok(L::Sprint),
        "slot://crouch_idle" => Ok(L::CrouchIdle),
        "slot://crouch_walk" => Ok(L::CrouchWalk),
        "slot://jump" => Ok(L::Jump),
        "slot://fall" => Ok(L::Fall),
        _ => Err(format!(
            "humanoid locomotion graph contains unsupported clip alias '{reference}'"
        )),
    }
}

fn locomotion_graph_binding_variant_key(clips: &[Option<PlayerAnimationRuntimeClip>; 8]) -> u64 {
    let mut hasher = blake3::Hasher::new();
    for state in [
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Idle,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Walk,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Run,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchIdle,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchWalk,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Jump,
        newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Fall,
    ] {
        let slot = resolve_locomotion_slot(clips, state);
        if let Some(clip) = clips[slot].as_ref() {
            hasher.update(clip.clip_ref.as_bytes());
            hasher.update(&[0]);
            hasher.update(&(std::sync::Arc::as_ptr(&clip.clip) as usize as u64).to_le_bytes());
        }
        hasher.update(&[0xff]);
    }
    let digest = hasher.finalize();
    u64::from_le_bytes(
        digest.as_bytes()[..8]
            .try_into()
            .expect("blake3 digest width"),
    )
}

fn compile_game_ready_locomotion_graph(
    clips: &[Option<PlayerAnimationRuntimeClip>; 8],
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<std::sync::Arc<CompiledAnimationGraph>, String> {
    let startup = newengine_core::startup::last_startup_config()
        .ok_or_else(|| "GameReady locomotion graph requires startup configuration".to_owned())?;
    let graph_ref = newengine_core::plugin_runtime_data_value(
        startup,
        GAME_READY_WORLD_RUNTIME_DATA_OWNER,
        LOCOMOTION_GRAPH_RUNTIME_DATA_KEY,
    )?
    .to_owned();
    let (_, graph_bytes) = newengine_core::read_plugin_runtime_data_bytes(
        startup,
        GAME_READY_WORLD_RUNTIME_DATA_OWNER,
        LOCOMOTION_GRAPH_RUNTIME_DATA_KEY,
    )?;
    let variant_key = locomotion_graph_binding_variant_key(clips);
    global_compiled_animation_graph_store().load_or_compile_with_variant(
        &graph_ref,
        animation_runtime,
        variant_key,
        {
            let graph_ref = graph_ref.clone();
            move |logical_path| {
                if logical_path.eq_ignore_ascii_case(&graph_ref) {
                    Ok(graph_bytes.clone())
                } else {
                    Err(format!(
                        "unexpected GameReady locomotion graph asset path '{logical_path}'"
                    ))
                }
            }
        },
        |reference| {
            let state = locomotion_state_for_alias(reference)?;
            let slot = resolve_locomotion_slot(clips, state);
            clips[slot]
                .as_ref()
                .map(|clip| clip.clip.clone())
                .ok_or_else(|| {
                    format!(
                        "GameReady locomotion graph alias '{reference}' resolved to empty slot={slot}"
                    )
                })
        },
    )
}

fn physical_locomotion_clip_ref_for_alias<'a>(
    clips: &'a [Option<PlayerAnimationRuntimeClip>; 8],
    reference: &str,
) -> Option<&'a str> {
    let state = locomotion_state_for_alias(reference).ok()?;
    let slot = resolve_locomotion_slot(clips, state);
    clips[slot].as_ref().map(|clip| clip.clip_ref.as_str())
}
#[inline]
fn locomotion_playback_rate(
    animation_state: newengine_engine_runtime::gameplay::PlayerAnimationState,
) -> f32 {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match animation_state.locomotion {
        L::Walk => (animation_state.normalized_speed / 0.40).clamp(0.65, 1.45),
        L::Run => (animation_state.normalized_speed / 0.85).clamp(0.75, 1.45),
        L::Sprint => animation_state.normalized_speed.clamp(1.0, 1.65),
        L::CrouchWalk => (animation_state.normalized_speed / 0.333_333_34).clamp(0.70, 1.25),
        _ => 1.0,
    }
}

fn graph_intent(
    entity: newengine_ecs::EntityId,
    graph: &CompiledAnimationGraph,
    kind: newengine_animation_api::AnimationIntentKind,
    parameters: serde_json::Value,
) -> newengine_animation_api::AnimationIntentDtoV1 {
    newengine_animation_api::AnimationIntentDtoV1 {
        entity: entity.into(),
        intent: kind,
        graph: Some(newengine_animation_api::AnimationGraphRef(
            graph.name().to_owned(),
        )),
        clip: None,
        task: None,
        tags: Vec::new(),
        parameters,
    }
}

fn apply_locomotion_graph_parameters(
    entity: newengine_ecs::EntityId,
    graph: &CompiledAnimationGraph,
    instance: &mut AnimationGraphInstance,
    normalized_speed: f32,
) -> Result<(), String> {
    let intent = graph_intent(
        entity,
        graph,
        newengine_animation_api::AnimationIntentKind::SetParameter,
        serde_json::json!({ "normalized_speed": normalized_speed }),
    );
    apply_animation_intent_to_graph_instance(graph, instance, &intent).map(|_| ())
}

fn blend_locomotion_graph_to_state(
    entity: newengine_ecs::EntityId,
    graph: &CompiledAnimationGraph,
    instance: &mut AnimationGraphInstance,
    state: &str,
) -> Result<(), String> {
    let intent = graph_intent(
        entity,
        graph,
        newengine_animation_api::AnimationIntentKind::BlendToState,
        serde_json::json!({
            "state": state,
            "blend_seconds": 0.12
        }),
    );
    apply_animation_intent_to_graph_instance(graph, instance, &intent).map(|_| ())
}

fn collect_locomotion_graph_events(
    entity: newengine_ecs::EntityId,
    graph: &CompiledAnimationGraph,
    evaluation: &AnimationGraphEvaluation,
    clips: &[Option<PlayerAnimationRuntimeClip>; 8],
    out: &mut Vec<newengine_animation_api::AnimationTimelineEventV1>,
) -> Result<usize, String> {
    let mut emitted = 0usize;
    for occurrence in evaluation.events.iter().copied() {
        let clip = graph.clip(occurrence.clip_index).ok_or_else(|| {
            format!(
                "locomotion graph event references invalid clip index {}",
                occurrence.clip_index
            )
        })?;
        let graph_ref = graph.clip_reference(occurrence.clip_index).ok_or_else(|| {
            format!(
                "locomotion graph event has no clip reference index={}",
                occurrence.clip_index
            )
        })?;
        let physical_ref =
            physical_locomotion_clip_ref_for_alias(clips, graph_ref).unwrap_or(graph_ref);
        let occurrence = AnimationEventOccurrence {
            event_index: occurrence.event_index,
            playback_time_seconds: occurrence.playback_time_seconds,
            loop_index: occurrence.loop_index,
        };
        if let Some(event) = crate::animation_events::timeline_event(
            entity,
            physical_ref,
            "character.locomotion",
            clip,
            occurrence,
        ) {
            out.push(event);
            emitted += 1;
        }
    }
    Ok(emitted)
}
