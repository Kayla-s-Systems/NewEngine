/// Emit equipment-presentation selection diagnostics only when the authored selection changes.
/// Keeping this outside the animation evaluator avoids mixing trace-policy mutation with pose math.
fn trace_equipment_pose_selection(
    player: newengine_ecs::EntityId,
    binding: &mut PlayerAnimationRuntimeBinding,
    equipment_presentation_active: bool,
    equipment_pose_family: Option<&str>,
    equipment_stance: EquipmentPresentationStance,
) {
    let equipment_trace_changed = binding.equipment_trace_active != equipment_presentation_active
        || binding.equipment_trace_stance != equipment_stance
        || binding.equipment_trace_family.as_deref() != equipment_pose_family;
    if equipment_trace_changed {
        let selected_clip = equipment_presentation_active
            .then(|| {
                select_equipment_pose_set(
                    &binding.equipment_default_pose_set,
                    &binding.equipment_pose_sets,
                    equipment_pose_family,
                )
                .and_then(|set| match equipment_stance {
                    EquipmentPresentationStance::Ready => set.ready.as_ref(),
                    EquipmentPresentationStance::Aim => set.aim.as_ref(),
                    EquipmentPresentationStance::Reload => set.reload.as_ref(),
                    EquipmentPresentationStance::None => None,
                })
                .map(|clip| clip.clip_ref.clone())
            })
            .flatten();
        let stance = match equipment_stance {
            EquipmentPresentationStance::None => "none",
            EquipmentPresentationStance::Ready => "ready",
            EquipmentPresentationStance::Aim => "aim",
            EquipmentPresentationStance::Reload => "reload",
        };
        newengine_ulog_api::ulog::info!(
            "game-ready: equipment pose selected player={} active={} family='{}' stance='{}' clip='{}' policy='weapon class selects character-owned authored capability; no cross-family fallback'",
            player.stable_u64(),
            equipment_presentation_active,
            equipment_pose_family.unwrap_or("<generic>"),
            stance,
            selected_clip.as_deref().unwrap_or("<none>"),
        );
        binding.equipment_trace_active = equipment_presentation_active;
        binding.equipment_trace_family = equipment_pose_family.map(str::to_owned);
        binding.equipment_trace_stance = equipment_stance;
    }
}

#[derive(Clone, Copy, Debug)]
struct EquipmentDirectionalBlend {
    a: EquipmentAimDirection,
    b: EquipmentAimDirection,
    blend_to_b: f32,
}

#[inline]
fn equipment_pose_body_stance(
    locomotion: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    look_context: newengine_engine_runtime::gameplay::PlayerLookContext,
) -> EquipmentPoseBodyStance {
    use newengine_engine_runtime::gameplay::{
        PlayerLocomotionAnimation as L, PlayerLookContext as C,
    };
    if look_context == C::Prone {
        EquipmentPoseBodyStance::Prone
    } else if matches!(locomotion, L::CrouchIdle | L::CrouchWalk) {
        EquipmentPoseBodyStance::Crouch
    } else {
        EquipmentPoseBodyStance::Stand
    }
}

fn equipment_directional_blend(
    space: &EquipmentAimPoseSpace,
    aim_velocity_local: Vec3,
) -> Option<EquipmentDirectionalBlend> {
    const IDLE_SPEED_MPS: f32 = 0.08;
    let planar = Vec3::new(aim_velocity_local.x, 0.0, aim_velocity_local.z);
    let speed = planar.length();
    if !speed.is_finite() || speed < IDLE_SPEED_MPS || space.movement.is_empty() {
        return None;
    }
    let angle = planar.x.atan2(-planar.z).rem_euclid(std::f32::consts::TAU);
    let position = angle / std::f32::consts::FRAC_PI_4;
    let lower_base = position.floor() as i32;
    let upper_base = position.ceil() as i32;
    let dir_at = |index: i32| EquipmentAimDirection::ALL[index.rem_euclid(8) as usize];

    let mut lower = None;
    for step in 0..8 {
        let raw = lower_base - step;
        let direction = dir_at(raw);
        if space.movement.contains_key(&direction) {
            let distance = (position - raw as f32).rem_euclid(8.0);
            lower = Some((direction, distance));
            break;
        }
    }
    let mut upper = None;
    for step in 0..8 {
        let raw = upper_base + step;
        let direction = dir_at(raw);
        if space.movement.contains_key(&direction) {
            let distance = (raw as f32 - position).rem_euclid(8.0);
            upper = Some((direction, distance));
            break;
        }
    }
    match (lower, upper) {
        (Some((a, _)), Some((b, _))) if a == b => Some(EquipmentDirectionalBlend {
            a,
            b,
            blend_to_b: 0.0,
        }),
        (Some((a, da)), Some((b, db))) => {
            let total = da + db;
            Some(EquipmentDirectionalBlend {
                a,
                b,
                blend_to_b: if total > 1.0e-6 { da / total } else { 0.0 },
            })
        }
        (Some((a, _)), None) => Some(EquipmentDirectionalBlend {
            a,
            b: a,
            blend_to_b: 0.0,
        }),
        (None, Some((b, _))) => Some(EquipmentDirectionalBlend {
            a: b,
            b,
            blend_to_b: 0.0,
        }),
        (None, None) => None,
    }
}

#[inline]
fn equipment_loop_phase(duration_seconds: f32, elapsed_seconds: f32) -> f32 {
    let duration = duration_seconds.max(1.0 / 30.0);
    (elapsed_seconds.rem_euclid(duration) / duration).clamp(0.0, 1.0)
}

fn equipment_transition_clip<'a>(
    set: &'a EquipmentPoseSet,
    kind: EquipmentTransitionKind,
) -> Option<&'a PlayerAnimationRuntimeClip> {
    match kind {
        EquipmentTransitionKind::ReadyToAim => set.transitions.ready_to_aim.as_ref(),
        EquipmentTransitionKind::AimToReady => set.transitions.aim_to_ready.as_ref(),
    }
}

fn begin_or_advance_equipment_transition(
    binding: &mut PlayerAnimationRuntimeBinding,
    equipment_pose_family: Option<&str>,
    current_stance: EquipmentPresentationStance,
    dt: f32,
) {
    let transition_kind = match (binding.equipment_previous_stance, current_stance) {
        (EquipmentPresentationStance::Ready, EquipmentPresentationStance::Aim) => {
            Some(EquipmentTransitionKind::ReadyToAim)
        }
        (EquipmentPresentationStance::Aim, EquipmentPresentationStance::Ready) => {
            Some(EquipmentTransitionKind::AimToReady)
        }
        _ => None,
    };
    if let Some(kind) = transition_kind {
        let authored = select_equipment_pose_set(
            &binding.equipment_default_pose_set,
            &binding.equipment_pose_sets,
            equipment_pose_family,
        )
        .and_then(|set| equipment_transition_clip(set, kind))
        .is_some();
        binding.equipment_transition = authored.then_some(EquipmentTransitionRuntimeState {
            kind,
            elapsed_seconds: 0.0,
        });
    } else if !matches!(
        current_stance,
        EquipmentPresentationStance::Ready | EquipmentPresentationStance::Aim
    ) {
        binding.equipment_transition = None;
    }
    binding.equipment_previous_stance = current_stance;

    let Some(mut transition) = binding.equipment_transition else {
        return;
    };
    transition.elapsed_seconds += dt.max(0.0);
    let duration = select_equipment_pose_set(
        &binding.equipment_default_pose_set,
        &binding.equipment_pose_sets,
        equipment_pose_family,
    )
    .and_then(|set| equipment_transition_clip(set, transition.kind))
    .map(|clip| clip.clip.duration_seconds.max(1.0 / 30.0));
    if duration.is_none_or(|duration| transition.elapsed_seconds >= duration) {
        binding.equipment_transition = None;
    } else {
        binding.equipment_transition = Some(transition);
    }
}

fn apply_layered_equipment_aim_pose(
    pose_set: &EquipmentPoseSet,
    body_stance: EquipmentPoseBodyStance,
    aim_velocity_local: Vec3,
    elapsed_seconds: f32,
    aim_alpha: f32,
    obstruction_alpha: f32,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch_a: &mut Vec<JointLocalPose>,
    scratch_b: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    root_joint: Option<usize>,
    weights: &[ResolvedJointBlendRule],
) -> Result<bool, String> {
    let space = pose_set.pose_space(body_stance);
    if !space.any() {
        return Ok(false);
    }
    let aim_alpha = aim_alpha.clamp(0.0, 1.0);
    let directional = equipment_directional_blend(space, aim_velocity_local);
    let mut base_applied = false;
    if let Some(blend) = directional {
        let clip_a = space.movement.get(&blend.a);
        let clip_b = space.movement.get(&blend.b);
        let duration_a = clip_a.map(|clip| clip.clip.duration_seconds).unwrap_or(0.0);
        let duration_b = clip_b
            .map(|clip| clip.clip.duration_seconds)
            .unwrap_or(duration_a);
        let duration = duration_a + (duration_b - duration_a) * blend.blend_to_b;
        let phase = equipment_loop_phase(duration, elapsed_seconds);
        base_applied = apply_equipment_full_body_directional_pose(
            clip_a,
            clip_b,
            blend.blend_to_b,
            animation_runtime,
            scratch_a,
            scratch_b,
            target,
            phase,
            root_joint,
            aim_alpha,
        )?;
    }
    if !base_applied {
        let base = space.idle.as_ref().or(pose_set.aim.as_ref());
        let phase = base
            .map(|clip| equipment_loop_phase(clip.clip.duration_seconds, elapsed_seconds))
            .unwrap_or(0.0);
        let _ = apply_equipment_full_body_directional_pose(
            base,
            base,
            0.0,
            animation_runtime,
            scratch_a,
            scratch_b,
            target,
            phase,
            root_joint,
            aim_alpha,
        )?;
    }

    // Grip composition follows the authored contract literally:
    //   directional locomotion -> absolute reference -> absolute arms/prop-hand frames
    //   -> anatomical finger articulation -> additive delta.
    // The source `*-hands` layer owns prop attachment frames, not phalanges. Finger articulation is
    // a separate compact authored domain projected onto the character's anatomical finger joints.
    // `*-add` clips are already stored around identity (zero translations / delta quaternions);
    // deriving another delta against `ref` double-subtracts the torso reference and twists the pose.
    for clip in [
        space.grip.reference.as_ref(),
        space.grip.arms.as_ref(),
        space.grip.hands.as_ref(),
    ] {
        apply_equipment_rotation_overlay(
            clip,
            animation_runtime,
            scratch_a,
            target,
            0.0,
            weights,
            aim_alpha,
        )?;
    }
    let finger_phase = space
        .grip
        .fingers
        .as_ref()
        .map(|clip| equipment_loop_phase(clip.clip.duration_seconds, elapsed_seconds))
        .unwrap_or(0.0);
    apply_equipment_owned_rotation_overlay(
        space.grip.fingers.as_ref(),
        animation_runtime,
        scratch_a,
        target,
        finger_phase,
        aim_alpha,
    )?;

    apply_equipment_additive_overlay(
        space.grip.additive.as_ref(),
        animation_runtime,
        scratch_a,
        target,
        0.0,
        weights,
        aim_alpha,
    )?;

    // A paired blocked `sub/add` contract is evaluated as a relative pose delta: subtract the
    // authored reference and add the blocked target. This preserves the current firing-hand root;
    // weapon/contact IK remains the final authority after animation evaluation.
    let blocked_weight = obstruction_alpha.clamp(0.0, 1.0) * aim_alpha;
    if blocked_weight > 1.0e-5 {
        apply_equipment_relative_delta_overlay(
            space.blocked_subtractive.as_ref(),
            space.blocked_additive.as_ref(),
            animation_runtime,
            scratch_a,
            scratch_b,
            target,
            0.0,
            weights,
            blocked_weight,
        )?;
    }
    Ok(true)
}
