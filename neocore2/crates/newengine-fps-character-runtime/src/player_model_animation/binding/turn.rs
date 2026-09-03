#[inline]
fn turn_in_place_target_yaw(slot: TurnInPlaceSlot, elapsed_seconds: f32, duration: f32) -> f32 {
    let duration = if duration.is_finite() && duration > 1.0e-6 {
        duration
    } else {
        1.0
    };
    let phase = if elapsed_seconds.is_finite() {
        (elapsed_seconds / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Ease-in/out follows authored weight transfer instead of imposing a linear rigid-body spin.
    let eased = phase * phase * (3.0 - 2.0 * phase);
    slot.signed_yaw_radians() * eased
}

#[inline]
fn accumulate_turn_in_place_yaw(applied: f32, previous_wrapped: f32, current_wrapped: f32) -> f32 {
    if !applied.is_finite() || !previous_wrapped.is_finite() || !current_wrapped.is_finite() {
        return applied;
    }
    applied + newengine_math::wrap_pi(current_wrapped - previous_wrapped)
}

#[inline]
fn bounded_turn_in_place_step(yaw_error: f32) -> f32 {
    if !yaw_error.is_finite() {
        return 0.0;
    }
    yaw_error.clamp(
        -TURN_IN_PLACE_MAX_STEP_RADIANS,
        TURN_IN_PLACE_MAX_STEP_RADIANS,
    )
}

#[inline]
fn live_view_residual_requires_turn_replan(
    active: TurnInPlaceSlot,
    residual_yaw: f32,
    hysteresis_radians: f32,
) -> bool {
    if !residual_yaw.is_finite() || !hysteresis_radians.is_finite() {
        return false;
    }
    if residual_yaw.abs() <= hysteresis_radians.max(0.0) {
        return false;
    }
    residual_yaw.signum() != active.signed_yaw_radians().signum()
}

#[inline]
fn compensate_turn_root_yaw(
    pose: &mut [JointLocalPose],
    root_joint: Option<usize>,
    applied_world_yaw: f32,
) {
    if !applied_world_yaw.is_finite() || applied_world_yaw.abs() <= 1.0e-6 {
        return;
    }
    let Some(root) = root_joint.and_then(|index| pose.get_mut(index)) else {
        return;
    };
    let authored = Quat::from_xyzw(
        root.rotation[0],
        root.rotation[1],
        root.rotation[2],
        root.rotation[3],
    )
    .normalize_or_identity();
    // Physical yaw is extracted into PlayerActor. Counter-rotate the sampled root by the exact
    // accepted amount so feet/pelvis keep the authored performance and never double-spin.
    let compensated =
        (Quat::from_rotation_y(-applied_world_yaw) * authored).normalize_or_identity();
    root.rotation = [compensated.x, compensated.y, compensated.z, compensated.w];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PoseContinuityKey {
    clip_hash: u64,
    turn_sequence: u64,
    unarmed_attack_sequence: u64,
    equipment_stance: u8,
}

#[derive(Clone, Debug)]
struct PoseContinuityBridge {
    initialized: bool,
    key: PoseContinuityKey,
    from_pose: Vec<JointLocalPose>,
    last_visible_pose: Vec<JointLocalPose>,
    elapsed_seconds: f32,
    duration_seconds: f32,
}

impl PoseContinuityBridge {
    fn new(initial_pose: &[JointLocalPose]) -> Self {
        Self {
            initialized: false,
            key: PoseContinuityKey::default(),
            from_pose: initial_pose.to_vec(),
            last_visible_pose: initial_pose.to_vec(),
            elapsed_seconds: 0.0,
            duration_seconds: 0.12,
        }
    }

    fn apply(&mut self, key: PoseContinuityKey, target_pose: &mut [JointLocalPose], dt: f32) {
        if self.last_visible_pose.len() != target_pose.len() || target_pose.is_empty() {
            self.initialized = true;
            self.key = key;
            self.elapsed_seconds = self.duration_seconds;
            self.last_visible_pose.clear();
            self.last_visible_pose.extend_from_slice(target_pose);
            self.from_pose.clone_from(&self.last_visible_pose);
            return;
        }
        if !self.initialized {
            self.initialized = true;
            self.key = key;
            self.elapsed_seconds = self.duration_seconds;
            return;
        }
        if self.key != key {
            self.key = key;
            self.from_pose.clone_from(&self.last_visible_pose);
            self.elapsed_seconds = 0.0;
        }
        if self.elapsed_seconds >= self.duration_seconds {
            return;
        }

        // Advance before sampling the blend weight. A source change therefore begins blending on
        // the same rendered frame instead of inserting a zero-weight/frozen transition frame.
        self.elapsed_seconds = (self.elapsed_seconds + dt.max(0.0)).min(self.duration_seconds);
        let phase = if self.duration_seconds > 1.0e-6 {
            (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let weight = phase * phase * (3.0 - 2.0 * phase);
        for (target, from) in target_pose.iter_mut().zip(self.from_pose.iter()) {
            let mut blended = *from;
            blend_joint_translation_only(&mut blended, target, weight);
            blend_joint_rotation_only(&mut blended, target, weight);
            blend_joint_scale_only(&mut blended, target, weight);
            *target = blended;
        }
    }

    fn restore_last_visible_pose(&self, target: &mut Vec<JointLocalPose>) -> bool {
        if self.last_visible_pose.len() != target.len() || target.is_empty() {
            return false;
        }
        target.clone_from(&self.last_visible_pose);
        true
    }

    fn commit_visible_pose(&mut self, visible_pose: &[JointLocalPose]) {
        if self.last_visible_pose.len() == visible_pose.len() {
            self.last_visible_pose.clone_from_slice(visible_pose);
        } else {
            self.last_visible_pose.clear();
            self.last_visible_pose.extend_from_slice(visible_pose);
        }
    }
}

#[inline]
fn animation_source_hash(value: &str) -> u64 {
    // Stable allocation-free FNV-1a. This is an animation source discriminator only, not a
    // security hash; it keeps the continuity hot path independent of randomized std hashers.
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
fn nearest_turn_in_place_slot(
    yaw_delta: f32,
    mut available: impl FnMut(TurnInPlaceSlot) -> bool,
) -> Option<TurnInPlaceSlot> {
    if !yaw_delta.is_finite() || yaw_delta.abs() <= 1.0e-5 {
        return None;
    }
    let left = yaw_delta > 0.0;
    let candidates = if left {
        [
            TurnInPlaceSlot::Left45,
            TurnInPlaceSlot::Left90,
            TurnInPlaceSlot::Left135,
            TurnInPlaceSlot::Left180,
        ]
    } else {
        [
            TurnInPlaceSlot::Right45,
            TurnInPlaceSlot::Right90,
            TurnInPlaceSlot::Right135,
            TurnInPlaceSlot::Right180,
        ]
    };
    candidates
        .into_iter()
        .filter(|slot| available(*slot))
        .min_by(|a, b| {
            let da = (yaw_delta.abs().to_degrees() - a.angle_degrees()).abs();
            let db = (yaw_delta.abs().to_degrees() - b.angle_degrees()).abs();
            da.total_cmp(&db)
        })
}

fn resolve_joint_blend_rules(
    skeleton: &ModelSkeletonMetadata,
    rules: &[newengine_engine_runtime::gameplay::PlayerJointRotationWeight],
) -> Result<Vec<ResolvedJointBlendRule>, String> {
    let mut resolved = Vec::with_capacity(rules.len());
    for rule in rules {
        let joint_name = rule.joint.trim();
        let index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == joint_name)
            .ok_or_else(|| {
                format!(
                    "authored animation layer joint is absent from skeleton joint='{joint_name}'"
                )
            })?;
        if !rule.weight.is_finite() || !(0.0..=1.0).contains(&rule.weight) || !rule.channels.any() {
            return Err(format!(
                "authored animation layer rule is invalid joint='{joint_name}' weight={} channels={:?}",
                rule.weight, rule.channels
            ));
        }
        resolved.push(ResolvedJointBlendRule {
            joint_index: index,
            weight: rule.weight,
            channels: rule.channels,
        });
    }
    Ok(resolved)
}

fn resolve_foot_joint_binding(skeleton: &ModelSkeletonMetadata) -> Option<PlayerFootJointBinding> {
    fn find_joint(skeleton: &ModelSkeletonMetadata, authored: &str, left: bool) -> Option<usize> {
        let root = skeleton.anchors.root.as_str();
        let hips = skeleton.anchors.hips.as_str();
        if !authored.trim().is_empty() && authored != root && authored != hips {
            if let Some(index) = skeleton
                .joints
                .iter()
                .position(|joint| joint.name == authored)
            {
                return Some(index);
            }
        }
        let patterns: &[&str] = if left {
            &[
                "left_foot",
                "foot_l",
                "l_foot",
                "leftfoot",
                "left_ankle",
                "ankle_l",
                "l_ankle",
            ]
        } else {
            &[
                "right_foot",
                "foot_r",
                "r_foot",
                "rightfoot",
                "right_ankle",
                "ankle_r",
                "r_ankle",
            ]
        };
        skeleton.joints.iter().position(|joint| {
            let name = joint
                .name
                .to_ascii_lowercase()
                .replace(['.', ':', '-'], "_");
            patterns.iter().any(|pattern| {
                name == *pattern || name.starts_with(pattern) || name.ends_with(pattern)
            })
        })
    }

    let left = find_joint(skeleton, &skeleton.anchors.left_foot, true)?;
    let right = find_joint(skeleton, &skeleton.anchors.right_foot, false)?;
    (left != right).then_some(PlayerFootJointBinding { left, right })
}
