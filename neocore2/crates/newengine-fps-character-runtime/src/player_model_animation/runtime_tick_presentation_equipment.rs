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
            "fps-character: equipment pose selected player={} active={} family='{}' stance='{}' clip='{}' policy='weapon class selects character-owned authored capability; no cross-family fallback'",
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

#[inline]
fn equipment_clip_owns_current_weapon_arm_contract(
    clip: &PlayerAnimationRuntimeClip,
    rig: &WeaponArmIkRig,
) -> bool {
    // A native firearm pose is terminal only when it owns the complete continuous chain that
    // determines the prop frame. Shoulder->palm alone is insufficient: leaving the clavicle on a
    // locomotion torso basis or the hand-prop helper on another layer creates a split hierarchy in
    // which a mathematically valid wrist can still appear behind the body. The TLOU-style rifle
    // contract is therefore clavicle -> shoulder -> elbow -> wrist -> palm plus the sibling
    // wrist -> hand_prop -> hand_prop_attachment chain on both sides.
    [
        rig.right_clavicle,
        Some(rig.right_shoulder),
        Some(rig.right_elbow),
        Some(rig.right_wrist),
        Some(rig.right_palm),
        rig.right_prop_helper,
        rig.right_prop_attachment,
        rig.left_clavicle,
        Some(rig.left_shoulder),
        Some(rig.left_elbow),
        Some(rig.left_wrist),
        Some(rig.left_palm),
        rig.left_prop_helper,
        rig.left_prop_attachment,
    ]
    .into_iter()
    .all(|joint| joint.is_some_and(|joint| clip.binding.owns_skeleton_joint(joint)))
}

#[inline]
fn equipment_transition_clip_is_compatible(
    clip: &PlayerAnimationRuntimeClip,
    rig: Option<&WeaponArmIkRig>,
) -> bool {
    let Some(rig) = rig else {
        return true;
    };
    let Some(right_prop) = rig.right_prop_attachment else {
        return true;
    };
    // A partial transition that does not touch the prop domain is harmless. If it *does* claim the
    // current prop socket, it must also own the current anatomical arm chain. This rejects clips
    // imported from an older/reference joint partition whose numeric source indices merely collide
    // with the current attachment tag (the TLOU rifle 1074-node transition is the canonical case).
    !clip.binding.owns_skeleton_joint(right_prop)
        || equipment_clip_owns_current_weapon_arm_contract(clip, rig)
}

fn equipment_aim_base_owns_current_weapon_arm_contract(
    pose_set: &EquipmentPoseSet,
    body_stance: EquipmentPoseBodyStance,
    aim_velocity_local: Vec3,
    rig: Option<&WeaponArmIkRig>,
) -> bool {
    let Some(rig) = rig else {
        return false;
    };
    let space = pose_set.pose_space(body_stance);
    if let Some(blend) = equipment_directional_blend(space, aim_velocity_local) {
        let Some(a) = space.movement.get(&blend.a) else {
            return false;
        };
        let Some(b) = space.movement.get(&blend.b) else {
            return false;
        };
        return equipment_clip_owns_current_weapon_arm_contract(a, rig)
            && equipment_clip_owns_current_weapon_arm_contract(b, rig);
    }
    space
        .idle
        .as_ref()
        .or(pose_set.aim.as_ref())
        .is_some_and(|clip| equipment_clip_owns_current_weapon_arm_contract(clip, rig))
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
        .is_some_and(|clip| {
            equipment_transition_clip_is_compatible(clip, binding.equipment_ik.as_ref())
        });
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

fn apply_equipment_grip_layers(
    space: &EquipmentAimPoseSpace,
    layer_phase: f32,
    finger_phase: f32,
    weight_scale: f32,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    weights: &[ResolvedJointBlendRule],
) -> Result<(), String> {
    // TLOU firearm composition is not `base owns arm => base is terminal`. The native MM base and
    // weapon grip layers share the same current character partition and are authored to compose.
    // The additive layer first corrects torso/arm orientation; absolute arm and prop-hand layers then
    // re-establish the terminal grip frame. Applying ADD last splits the two prop attachments.
    // `reference` is deliberately excluded: it is a composition operand and may belong to a foreign
    // reference rig (the Abby rifle stand-aim ref is a 1074-node domain).
    apply_equipment_additive_overlay(
        space.grip.additive.as_ref(),
        animation_runtime,
        scratch,
        target,
        layer_phase,
        weights,
        weight_scale,
    )?;
    for clip in [space.grip.arms.as_ref(), space.grip.hands.as_ref()] {
        apply_equipment_rotation_overlay(
            clip,
            animation_runtime,
            scratch,
            target,
            layer_phase,
            weights,
            weight_scale,
        )?;
    }
    apply_equipment_owned_rotation_overlay(
        space.grip.fingers.as_ref(),
        animation_runtime,
        scratch,
        target,
        finger_phase,
        weight_scale,
    )?;
    Ok(())
}

fn apply_equipment_ready_pose(
    pose_set: Option<&EquipmentPoseSet>,
    body_stance: EquipmentPoseBodyStance,
    sample_phase: f32,
    equipment_ik: Option<&WeaponArmIkRig>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch_a: &mut Vec<JointLocalPose>,
    scratch_b: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    root_joint: Option<usize>,
    weights: &[ResolvedJointBlendRule],
) -> Result<bool, String> {
    let ready = pose_set.and_then(|set| set.ready.as_ref());
    let self_contained = ready
        .zip(equipment_ik)
        .is_some_and(|(clip, rig)| equipment_clip_owns_current_weapon_arm_contract(clip, rig));
    if self_contained {
        let _ = apply_equipment_full_body_directional_pose(
            ready,
            ready,
            0.0,
            animation_runtime,
            scratch_a,
            scratch_b,
            target,
            sample_phase,
            root_joint,
            1.0,
        )?;
    } else {
        apply_equipment_rotation_overlay(
            ready,
            animation_runtime,
            scratch_a,
            target,
            sample_phase,
            weights,
            1.0,
        )?;
    }

    if let Some(space) = pose_set.map(|set| set.pose_space(body_stance)) {
        apply_equipment_grip_layers(
            space,
            0.0,
            sample_phase,
            1.0,
            animation_runtime,
            scratch_a,
            target,
            weights,
        )?;
    }
    Ok(self_contained)
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

    // Joint ownership is an authority/binding invariant, not a reason to skip authored grip
    // composition. Native TLOU MM rifle bases already own the arm joints but still require the
    // same-partition stand/crouch ADD + ARMS + HANDS layers to produce the final firing grip.
    let finger_phase = space
        .grip
        .fingers
        .as_ref()
        .map(|clip| equipment_loop_phase(clip.clip.duration_seconds, elapsed_seconds))
        .unwrap_or(0.0);
    apply_equipment_grip_layers(
        space,
        0.0,
        finger_phase,
        aim_alpha,
        animation_runtime,
        scratch_a,
        target,
        weights,
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
