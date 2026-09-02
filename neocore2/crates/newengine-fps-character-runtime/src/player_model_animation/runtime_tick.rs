#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EquipmentPresentationStance {
    #[default]
    None,
    Ready,
    Aim,
    Reload,
}

const EQUIPMENT_SUPPORT_IK_RESIDUAL_WARN_THRESHOLD_M: f32 = 0.025;
const EQUIPMENT_SOCKET_ANGULAR_WARN_THRESHOLD_DEG: f32 = 1.0;
const EQUIPMENT_SUPPORT_IK_RESIDUAL_DIAG_INTERVAL_SECONDS: f32 = 2.0;

#[inline]
fn semantic_f32(state: &ResolvedAnimationSemanticState, key: &str, fallback: f32) -> f32 {
    state
        .parameters
        .get(key)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

#[inline]
fn semantic_u64(state: &ResolvedAnimationSemanticState, key: &str, fallback: u64) -> u64 {
    state
        .parameters
        .get(key)
        .and_then(|value| value.as_u64())
        .unwrap_or(fallback)
}

fn locomotion_from_semantic_target(
    target: &str,
) -> Option<newengine_engine_runtime::gameplay::PlayerLocomotionAnimation> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match target.trim().to_ascii_lowercase().as_str() {
        "locomotion.idle" => Some(L::Idle),
        "locomotion.walk" => Some(L::Walk),
        "locomotion.run" => Some(L::Run),
        "locomotion.sprint" => Some(L::Sprint),
        "locomotion.crouch_idle" => Some(L::CrouchIdle),
        "locomotion.crouch_walk" => Some(L::CrouchWalk),
        "locomotion.jump" => Some(L::Jump),
        "locomotion.fall" => Some(L::Fall),
        "none" => None,
        _ => None,
    }
}

fn look_context_from_semantic_target(
    target: &str,
) -> Option<newengine_engine_runtime::gameplay::PlayerLookContext> {
    use newengine_engine_runtime::gameplay::PlayerLookContext as C;
    match target.trim().to_ascii_lowercase().as_str() {
        "look.auto" => Some(C::Standard),
        "look.context.cover_low_left" => Some(C::CoverLowLeft),
        "look.context.cover_low_right" => Some(C::CoverLowRight),
        "look.context.prone" => Some(C::Prone),
        "look.context.supine" => Some(C::Supine),
        "look.context.rope" => Some(C::Rope),
        "look.context.ladder" => Some(C::Ladder),
        "look.context.swim_idle" => Some(C::SwimIdle),
        "look.context.injured" => Some(C::Injured),
        "look.context.relaxed_injured" => Some(C::RelaxedInjured),
        "none" => None,
        _ => None,
    }
}

fn equipment_stance_from_semantic_target(target: &str) -> EquipmentPresentationStance {
    match target.trim().to_ascii_lowercase().as_str() {
        "equipment.ready" => EquipmentPresentationStance::Ready,
        "equipment.aim" => EquipmentPresentationStance::Aim,
        "equipment.reload" => EquipmentPresentationStance::Reload,
        _ => EquipmentPresentationStance::None,
    }
}

fn turn_slot_from_semantic_target(target: &str) -> Option<TurnInPlaceSlot> {
    match target.trim().to_ascii_lowercase().as_str() {
        "turn.left.45" => Some(TurnInPlaceSlot::Left45),
        "turn.right.45" => Some(TurnInPlaceSlot::Right45),
        "turn.left.90" => Some(TurnInPlaceSlot::Left90),
        "turn.right.90" => Some(TurnInPlaceSlot::Right90),
        "turn.left.135" => Some(TurnInPlaceSlot::Left135),
        "turn.right.135" => Some(TurnInPlaceSlot::Right135),
        "turn.left.180" => Some(TurnInPlaceSlot::Left180),
        "turn.right.180" => Some(TurnInPlaceSlot::Right180),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct SemanticLandingImpact {
    sequence: u64,
    distance: f32,
    downward_speed: f32,
    horizontal_speed: f32,
}

#[derive(Clone, Copy, Debug)]
struct PlayerAnimationSemanticFrameState {
    animation_state: newengine_engine_runtime::gameplay::PlayerAnimationState,
    look_context: newengine_engine_runtime::gameplay::PlayerLookContext,
    view_yaw: Option<f32>,
    view_pitch: Option<f32>,
    noclip_enabled: bool,
    fall_active: bool,
    fall_distance: f32,
    equipment_stance: EquipmentPresentationStance,
    aim_alpha: f32,
    reload_progress: Option<f32>,
    recoil_alpha: f32,
    recoil_yaw_radians: f32,
    obstruction_alpha: f32,
    unarmed_ready: bool,
    unarmed_attack_sequence: u64,
    landing: Option<SemanticLandingImpact>,
    max_pulse_sequence: u64,
}

fn semantic_frame_state(
    binding: &PlayerAnimationRuntimeBinding,
) -> PlayerAnimationSemanticFrameState {
    let locomotion_state = binding.semantic_input.state("character.locomotion");
    let locomotion = locomotion_state
        .and_then(|state| locomotion_from_semantic_target(&state.target))
        .unwrap_or(binding.active_state);
    let mut animation_state = newengine_engine_runtime::gameplay::PlayerAnimationState {
        locomotion,
        ..newengine_engine_runtime::gameplay::PlayerAnimationState::default()
    };
    if let Some(state) = locomotion_state {
        animation_state.normalized_speed =
            semantic_f32(state, "normalized_speed", 0.0).clamp(0.0, 2.0);
        animation_state.cycle_phase = semantic_f32(state, "cycle_phase", 0.0).rem_euclid(1.0);
        animation_state.transition_alpha =
            semantic_f32(state, "transition_alpha", 1.0).clamp(0.0, 1.0);
        animation_state.revision = semantic_u64(state, "revision", state.sequence).max(1);
    }

    let look_context = binding
        .semantic_input
        .state("character.look.context")
        .and_then(|state| look_context_from_semantic_target(&state.target))
        .unwrap_or_default();
    let look_view = binding.semantic_input.state("character.look.view");
    let view_yaw = look_view.map(|state| semantic_f32(state, "yaw", 0.0));
    let view_pitch = look_view.map(|state| semantic_f32(state, "pitch", 0.0));

    let traversal = binding.semantic_input.state("character.traversal");
    let noclip_enabled =
        traversal.is_some_and(|state| state.target.eq_ignore_ascii_case("traversal.noclip"));
    let fall = binding.semantic_input.state("character.fall");
    let fall_active = fall.is_some_and(|state| state.target.eq_ignore_ascii_case("fall.auto"));
    let fall_distance = fall
        .map(|state| semantic_f32(state, "distance", 0.0).max(0.0))
        .unwrap_or(0.0);

    let equipment = binding.semantic_input.state("character.equipment");
    let equipment_stance = equipment
        .map(|state| equipment_stance_from_semantic_target(&state.target))
        .unwrap_or_default();
    let aim_alpha = equipment
        .map(|state| semantic_f32(state, "aim_alpha", 0.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let reload_progress = (equipment_stance == EquipmentPresentationStance::Reload).then(|| {
        equipment
            .map(|state| semantic_f32(state, "reload_progress", 0.0).clamp(0.0, 1.0))
            .unwrap_or(0.0)
    });
    let recoil_alpha = equipment
        .map(|state| semantic_f32(state, "recoil_alpha", 0.0).max(0.0))
        .unwrap_or(0.0);
    let recoil_yaw_radians = equipment
        .map(|state| semantic_f32(state, "recoil_yaw_radians", 0.0))
        .unwrap_or(0.0);
    let obstruction_alpha = equipment
        .map(|state| semantic_f32(state, "obstruction_alpha", 0.0).clamp(0.0, 1.0))
        .unwrap_or(0.0);

    let weapon_mode = binding.semantic_input.state("character.weapon.mode");
    let unarmed_ready =
        weapon_mode.is_some_and(|state| state.target.eq_ignore_ascii_case("unarmed.ready"));
    let attack = binding
        .semantic_input
        .latest_pulse_target("unarmed.attack")
        .filter(|pulse| pulse.sequence > binding.consumed_pulse_sequence);
    let landing_pulse = binding
        .semantic_input
        .latest_pulse_target("landing.auto")
        .filter(|pulse| pulse.sequence > binding.consumed_pulse_sequence);
    let landing = landing_pulse.map(|pulse| SemanticLandingImpact {
        sequence: pulse.sequence,
        distance: semantic_f32(pulse, "distance", 0.0).max(0.0),
        downward_speed: semantic_f32(pulse, "downward_speed", 0.0).max(0.0),
        horizontal_speed: semantic_f32(pulse, "horizontal_speed", 0.0).max(0.0),
    });
    let unarmed_attack_sequence = attack.map(|pulse| pulse.sequence).unwrap_or(0);
    let turn_request = binding
        .semantic_input
        .pulses
        .iter()
        .rev()
        .filter(|pulse| pulse.sequence > binding.consumed_pulse_sequence)
        .find_map(|pulse| {
            turn_slot_from_semantic_target(&pulse.target).map(|slot| (pulse.sequence, slot))
        });
    let max_pulse_sequence = landing
        .map(|impact| impact.sequence)
        .into_iter()
        .chain(attack.map(|pulse| pulse.sequence))
        .chain(turn_request.map(|(sequence, _)| sequence))
        .max()
        .unwrap_or(binding.consumed_pulse_sequence);

    PlayerAnimationSemanticFrameState {
        animation_state,
        look_context,
        view_yaw,
        view_pitch,
        noclip_enabled,
        fall_active,
        fall_distance,
        equipment_stance,
        aim_alpha,
        reload_progress,
        recoil_alpha,
        recoil_yaw_radians,
        obstruction_alpha,
        unarmed_ready,
        unarmed_attack_sequence,
        landing,
        max_pulse_sequence,
    }
}

fn resolve_authored_look_state(
    locomotion: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    equipment: EquipmentPresentationStance,
    context: newengine_engine_runtime::gameplay::PlayerLookContext,
) -> AuthoredLookState {
    use newengine_engine_runtime::gameplay::{
        PlayerLocomotionAnimation as L, PlayerLookContext as C,
    };
    match context {
        C::CoverLowLeft => AuthoredLookState::CoverLowLeft,
        C::CoverLowRight => AuthoredLookState::CoverLowRight,
        C::Prone => AuthoredLookState::Prone,
        C::Supine => AuthoredLookState::Supine,
        C::Rope => AuthoredLookState::Rope,
        C::Ladder => AuthoredLookState::Ladder,
        C::SwimIdle => AuthoredLookState::SwimIdle,
        C::Injured => AuthoredLookState::Injured,
        C::RelaxedInjured => AuthoredLookState::RelaxedInjured,
        C::Standard => {
            if matches!(locomotion, L::CrouchIdle | L::CrouchWalk) {
                AuthoredLookState::Crouch
            } else if matches!(
                equipment,
                EquipmentPresentationStance::Ready | EquipmentPresentationStance::Aim
            ) {
                AuthoredLookState::Tense
            } else {
                AuthoredLookState::Relaxed
            }
        }
    }
}

#[inline]
fn authoritative_fall_presentation_requested(
    noclip_enabled: bool,
    fall_event_active: bool,
    locomotion: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> bool {
    !noclip_enabled
        && fall_event_active
        && locomotion == newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Fall
}

pub(crate) fn tick_player_skin_animation(
    world: &mut newengine_ecs::World,
    dt: f32,
    frame_index: u64,
) {
    let tick_started = std::time::Instant::now();
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };

    let query_started = std::time::Instant::now();
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    let query_ms = query_started.elapsed().as_secs_f32() * 1000.0;
    let player_count = players.len();

    let mut prepare_ms = 0.0_f32;
    let mut presentation_core_ms = 0.0_f32;
    let mut finalize_ms = 0.0_f32;
    let mut finalize_pose_copy_ms = 0.0_f32;
    let mut finalize_look_ms = 0.0_f32;
    let mut finalize_support_ik_ms = 0.0_f32;
    let mut finalize_continuity_eye_ms = 0.0_f32;
    let mut finalize_palette_ms = 0.0_f32;
    let mut finalize_joint_frames_ms = 0.0_f32;
    let mut finalize_braid_ms = 0.0_f32;
    let mut finalize_validation_ms = 0.0_f32;
    let mut finalize_overhead_ms = 0.0_f32;
    let mut evaluate_overhead_ms = 0.0_f32;
    let mut commit_ms = 0.0_f32;

    for player in players {
        let phase_started = std::time::Instant::now();
        let Some(frame) = prepare_player_animation_frame(world, player) else {
            prepare_ms += phase_started.elapsed().as_secs_f32() * 1000.0;
            continue;
        };
        prepare_ms += phase_started.elapsed().as_secs_f32() * 1000.0;

        let phase_started = std::time::Instant::now();
        let output = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            evaluate_player_animation_presentation(player, binding, dt, &frame)
        };
        let evaluate_total_ms = phase_started.elapsed().as_secs_f32() * 1000.0;
        let Some(output) = output else {
            evaluate_overhead_ms += evaluate_total_ms;
            continue;
        };
        presentation_core_ms += output.presentation_core_ms;
        finalize_ms += output.finalize_ms;
        finalize_pose_copy_ms += output.finalize_timing.pose_copy_ms;
        finalize_look_ms += output.finalize_timing.look_ms;
        finalize_support_ik_ms += output.finalize_timing.support_ik_ms;
        finalize_continuity_eye_ms += output.finalize_timing.continuity_eye_ms;
        finalize_palette_ms += output.finalize_timing.palette_ms;
        finalize_joint_frames_ms += output.finalize_timing.joint_frames_ms;
        finalize_braid_ms += output.finalize_timing.braid_ms;
        finalize_validation_ms += output.finalize_timing.validation_ms;
        finalize_overhead_ms += output.finalize_timing.overhead_ms;
        evaluate_overhead_ms +=
            (evaluate_total_ms - output.presentation_core_ms - output.finalize_ms).max(0.0);

        let phase_started = std::time::Instant::now();
        commit_player_animation_frame(world, player, dt, output);
        commit_ms += phase_started.elapsed().as_secs_f32() * 1000.0;
    }

    let total_ms = tick_started.elapsed().as_secs_f32() * 1000.0;
    if total_ms >= 1.0 || frame_index.is_multiple_of(120) {
        let payload = serde_json::json!({
            "schema": "newengine.diagnostics.profiler.sample.v1",
            "category": "animation.skin",
            "source": "newengine-fps-character-runtime",
            "name": "player skin animation frame",
            "lane": "animation",
            "priority": "interactive",
            "dependency_group": format!("animation.skin.frame.{frame_index}"),
            "frame_index": frame_index,
            "elapsed_ms": total_ms,
            "budget_ms": 1.0,
            "slow": total_ms >= 1.0,
            "player_count": player_count,
            "query_ms": query_ms,
            "prepare_ms": prepare_ms,
            "presentation_core_ms": presentation_core_ms,
            "finalize_ms": finalize_ms,
            "finalize_pose_copy_ms": finalize_pose_copy_ms,
            "finalize_look_ms": finalize_look_ms,
            "finalize_support_ik_ms": finalize_support_ik_ms,
            "finalize_continuity_eye_ms": finalize_continuity_eye_ms,
            "finalize_palette_ms": finalize_palette_ms,
            "finalize_joint_frames_ms": finalize_joint_frames_ms,
            "finalize_braid_ms": finalize_braid_ms,
            "finalize_validation_ms": finalize_validation_ms,
            "finalize_overhead_ms": finalize_overhead_ms,
            "evaluate_overhead_ms": evaluate_overhead_ms,
            "commit_ms": commit_ms,
        });
        if let Ok(bytes) = serde_json::to_vec(&payload) {
            let _ = newengine_plugin_host::emit_plugin_event(
                "newengine.diagnostics.profiler.sample.v1",
                &bytes,
            );
        }
    }
}
