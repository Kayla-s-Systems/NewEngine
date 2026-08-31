#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: std::sync::Arc<AnimationClip>,
    binding: AnimationClipBinding,
    event_cursor: AnimationEventCursor,
}

#[derive(Clone, Copy, Debug)]
struct PlayerFootJointBinding {
    left: usize,
    right: usize,
}

#[derive(Clone, Copy, Debug)]
struct ResolvedJointBlendRule {
    joint_index: usize,
    joint_tag: u32,
    weight: f32,
    channels: newengine_engine_runtime::gameplay::PlayerJointChannels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnInPlaceSlot {
    Left45,
    Right45,
    Left90,
    Right90,
    Left135,
    Right135,
    Left180,
    Right180,
}

impl TurnInPlaceSlot {
    #[inline]
    fn signed_yaw_radians(self) -> f32 {
        match self {
            Self::Left45 => 45.0_f32.to_radians(),
            Self::Right45 => -45.0_f32.to_radians(),
            Self::Left90 => 90.0_f32.to_radians(),
            Self::Right90 => -90.0_f32.to_radians(),
            Self::Left135 => 135.0_f32.to_radians(),
            Self::Right135 => -135.0_f32.to_radians(),
            Self::Left180 => core::f32::consts::PI,
            Self::Right180 => -core::f32::consts::PI,
        }
    }

    #[inline]
    fn angle_degrees(self) -> f32 {
        self.signed_yaw_radians().abs().to_degrees()
    }
}

#[derive(Clone, Copy, Debug)]
struct TurnInPlaceRuntimeState {
    slot: TurnInPlaceSlot,
    elapsed_seconds: f32,
    /// Wrapped world yaw observed when the authored step started.
    start_body_yaw: f32,
    /// Last wrapped simulation yaw used to accumulate a continuous turn angle across +/-PI.
    last_body_yaw: f32,
    /// Authoritative physical yaw already accepted by simulation since this step started.
    applied_yaw_radians: f32,
}

const TURN_IN_PLACE_MAX_STEP_RADIANS: f32 = core::f32::consts::PI / 30.0; // 6 degrees
const TURN_IN_PLACE_FINISH_EPSILON_RADIANS: f32 = core::f32::consts::PI / 240.0; // 0.75 degree

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
            joint_tag: skeleton.joints[index].tag,
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
                .replace('.', "_")
                .replace(':', "_")
                .replace('-', "_");
            patterns.iter().any(|pattern| {
                name == *pattern || name.starts_with(pattern) || name.ends_with(pattern)
            })
        })
    }

    let left = find_joint(skeleton, &skeleton.anchors.left_foot, true)?;
    let right = find_joint(skeleton, &skeleton.anchors.right_foot, false)?;
    (left != right).then_some(PlayerFootJointBinding { left, right })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FallPresentationBand {
    Low,
    Medium,
    High,
}

#[inline]
pub(super) fn select_fall_presentation_band(
    distance: f32,
    low_available: bool,
    medium_available: bool,
    high_available: bool,
    medium_min_distance: f32,
    high_min_distance: f32,
) -> Option<FallPresentationBand> {
    let distance = if distance.is_finite() {
        distance.max(0.0)
    } else {
        0.0
    };
    let medium_min = if medium_min_distance.is_finite() {
        medium_min_distance.max(0.0)
    } else {
        0.0
    };
    let high_min = if high_min_distance.is_finite() {
        high_min_distance.max(medium_min)
    } else {
        medium_min
    };

    // Severity is authoritative. Missing authored data never substitutes a different animation
    // band: the caller holds the last visible pose instead of presenting a semantically unrelated
    // low/medium/high performance.
    if high_min > 0.0 && distance >= high_min {
        return high_available.then_some(FallPresentationBand::High);
    }
    if medium_min > 0.0 && distance >= medium_min {
        return medium_available.then_some(FallPresentationBand::Medium);
    }
    low_available.then_some(FallPresentationBand::Low)
}

#[derive(Clone, Debug)]
pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    active_slot: usize,
    locomotion_graph: std::sync::Arc<CompiledAnimationGraph>,
    locomotion_graph_instance: AnimationGraphInstance,
    locomotion_graph_evaluation: AnimationGraphEvaluation,
    skeleton: ModelSkeletonMetadata,
    animation_runtime: AnimationSkeletonRuntime,
    /// Pose currently visible on the character. This is preserved when a new
    /// locomotion state interrupts an in-flight cross-fade.
    current_locals: Vec<JointLocalPose>,
    sampled_target_locals: Vec<JointLocalPose>,
    palette_scratch: Vec<Mat4>,
    /// Absolute bind joint frames in baked model space. Current animated frames are derived as
    /// `skin_palette * bind_frame`, after all pose/follower corrections but before braid solve.
    bind_joint_frames: Vec<Mat4>,
    joint_frames_scratch: Vec<Mat4>,
    foot_joints: Option<PlayerFootJointBinding>,
    turn_root_joint: Option<usize>,
    turn_45_left_pose: Option<PlayerAnimationRuntimeClip>,
    turn_45_right_pose: Option<PlayerAnimationRuntimeClip>,
    turn_90_left_pose: Option<PlayerAnimationRuntimeClip>,
    turn_90_right_pose: Option<PlayerAnimationRuntimeClip>,
    turn_135_left_pose: Option<PlayerAnimationRuntimeClip>,
    turn_135_right_pose: Option<PlayerAnimationRuntimeClip>,
    turn_180_left_pose: Option<PlayerAnimationRuntimeClip>,
    turn_180_right_pose: Option<PlayerAnimationRuntimeClip>,
    turn_in_place: Option<TurnInPlaceRuntimeState>,
    turn_sequence: u64,
    pose_continuity: PoseContinuityBridge,
    /// Original-content authored look-at pose spaces. Range geometry, active joints and turn
    /// hand-off are derived from the selected native base/range clips, never from character constants.
    authored_look: AuthoredLookRuntimeBinding,
    braid_secondary_motion: Option<AbbyBraidRuntime>,
    /// Definition-authored local-pose copy rules resolved to this skeleton.
    helper_pose_copies: Vec<ResolvedJointCopyRule>,
    /// Imported Rigify control/face branches need the authored constraint order restored:
    /// deform body -> animated neck/head controls -> face/eyes deform branches.
    eye_contract: Option<EyeRuntimeContract>,
    head_follow: Option<DetachedHeadFollowRig>,
    noclip_pose: Option<PlayerAnimationRuntimeClip>,
    noclip_time_seconds: f32,
    noclip_active: bool,
    fall_low_pose: Option<PlayerAnimationRuntimeClip>,
    fall_medium_pose: Option<PlayerAnimationRuntimeClip>,
    fall_high_pose: Option<PlayerAnimationRuntimeClip>,
    landing_soft_pose: Option<PlayerAnimationRuntimeClip>,
    landing_medium_pose: Option<PlayerAnimationRuntimeClip>,
    landing_hard_pose: Option<PlayerAnimationRuntimeClip>,
    landing_hard_run_pose: Option<PlayerAnimationRuntimeClip>,
    landing_active_band: Option<FallPresentationBand>,
    landing_active_run: bool,
    landing_time_seconds: f32,
    landing_last_revision: u64,
    fall_medium_min_distance: f32,
    fall_high_min_distance: f32,
    fall_active_band: Option<FallPresentationBand>,
    fall_time_seconds: f32,
    equipment_ready_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_aim_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_reload_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_ready_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_sequence: u64,
    unarmed_attack_time_seconds: f32,
    equipment_ready_sample_phase: f32,
    equipment_time_seconds: f32,
    equipment_reload_active: bool,
    equipment_ready_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_aim_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_reload_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_overlay_locals: Vec<JointLocalPose>,
    equipment_ik: Option<WeaponArmIkRig>,
    /// Torso-owned, reach-fitted weapon root before secondary dynamics. Render consumes this exact root.
    equipment_resolved_weapon_root: Option<crate::weapon_grip::WeaponRootTransform>,
}

#[inline]
const fn locomotion_slot(
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> usize {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match state {
        L::Idle => 0,
        L::Walk => 1,
        L::Run => 2,
        L::Sprint => 3,
        L::CrouchIdle => 4,
        L::CrouchWalk => 5,
        L::Jump => 6,
        L::Fall => 7,
    }
}

impl PlayerAnimationRuntimeBinding {
    pub(super) fn authored_capabilities(
        &self,
    ) -> newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities {
        newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities {
            unarmed_ready: self.unarmed_ready_pose.is_some(),
            unarmed_attack: self.unarmed_attack_pose.is_some(),
            equipment_ready: self.equipment_ready_pose.is_some(),
            equipment_aim: self.equipment_aim_pose.is_some(),
            equipment_reload: self.equipment_reload_pose.is_some(),
            noclip: self.noclip_pose.is_some(),
        }
    }

    pub(super) fn initial_palette(&self) -> Vec<Mat4> {
        self.palette_scratch.clone()
    }

    pub(super) fn consume_landing_revision_baseline(&mut self, revision: u64) {
        self.landing_last_revision = revision;
        self.landing_active_band = None;
        self.landing_active_run = false;
        self.landing_time_seconds = 0.0;
    }

    pub(super) fn skeleton_joint_count(&self) -> usize {
        self.skeleton.joints.len()
    }

    pub(super) fn supplemental_palette_joint_count(&self) -> usize {
        0
    }

    pub(super) fn expected_palette_joints(&self) -> usize {
        self.skeleton_joint_count()
    }

    pub(super) fn clip_refs_csv(&self) -> String {
        self.clips
            .iter()
            .filter_map(|clip| clip.as_ref().map(|clip| clip.clip_ref.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn resolve_slot(
        &self,
        state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    ) -> Option<usize> {
        resolve_runtime_locomotion_slot(&self.clips, state)
    }

    fn turn_clip(&self, slot: TurnInPlaceSlot) -> Option<&PlayerAnimationRuntimeClip> {
        match slot {
            TurnInPlaceSlot::Left45 => self.turn_45_left_pose.as_ref(),
            TurnInPlaceSlot::Right45 => self.turn_45_right_pose.as_ref(),
            TurnInPlaceSlot::Left90 => self.turn_90_left_pose.as_ref(),
            TurnInPlaceSlot::Right90 => self.turn_90_right_pose.as_ref(),
            TurnInPlaceSlot::Left135 => self.turn_135_left_pose.as_ref(),
            TurnInPlaceSlot::Right135 => self.turn_135_right_pose.as_ref(),
            TurnInPlaceSlot::Left180 => self.turn_180_left_pose.as_ref(),
            TurnInPlaceSlot::Right180 => self.turn_180_right_pose.as_ref(),
        }
    }

    fn turn_clip_mut(&mut self, slot: TurnInPlaceSlot) -> Option<&mut PlayerAnimationRuntimeClip> {
        match slot {
            TurnInPlaceSlot::Left45 => self.turn_45_left_pose.as_mut(),
            TurnInPlaceSlot::Right45 => self.turn_45_right_pose.as_mut(),
            TurnInPlaceSlot::Left90 => self.turn_90_left_pose.as_mut(),
            TurnInPlaceSlot::Right90 => self.turn_90_right_pose.as_mut(),
            TurnInPlaceSlot::Left135 => self.turn_135_left_pose.as_mut(),
            TurnInPlaceSlot::Right135 => self.turn_135_right_pose.as_mut(),
            TurnInPlaceSlot::Left180 => self.turn_180_left_pose.as_mut(),
            TurnInPlaceSlot::Right180 => self.turn_180_right_pose.as_mut(),
        }
    }

    fn minimum_turn_step_radians(&self) -> Option<f32> {
        [
            TurnInPlaceSlot::Left45,
            TurnInPlaceSlot::Right45,
            TurnInPlaceSlot::Left90,
            TurnInPlaceSlot::Right90,
            TurnInPlaceSlot::Left135,
            TurnInPlaceSlot::Right135,
            TurnInPlaceSlot::Left180,
            TurnInPlaceSlot::Right180,
        ]
        .into_iter()
        .filter(|slot| self.turn_clip(*slot).is_some())
        .map(|slot| slot.signed_yaw_radians().abs())
        .min_by(|a, b| a.total_cmp(b))
    }
}

#[inline]
fn blend_joint_translation_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = Vec3::new(dst.translation[0], dst.translation[1], dst.translation[2]);
    let to = Vec3::new(src.translation[0], src.translation[1], src.translation[2]);
    let translation = from.lerp(to, weight);
    dst.translation = [translation.x, translation.y, translation.z];
}

#[inline]
fn blend_joint_rotation_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = Quat::from_xyzw(
        dst.rotation[0],
        dst.rotation[1],
        dst.rotation[2],
        dst.rotation[3],
    )
    .normalize_or_identity();
    let mut to = Quat::from_xyzw(
        src.rotation[0],
        src.rotation[1],
        src.rotation[2],
        src.rotation[3],
    )
    .normalize_or_identity();
    if from.dot(to) < 0.0 {
        to = Quat::from_xyzw(-to.x, -to.y, -to.z, -to.w);
    }
    let rotation = from.slerp(to, weight).normalize_or_identity();
    dst.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
}

#[inline]
fn blend_joint_scale_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = dst.scale.unwrap_or([1.0, 1.0, 1.0]);
    let to = src.scale.unwrap_or([1.0, 1.0, 1.0]);
    dst.scale = Some([
        from[0] + (to[0] - from[0]) * weight,
        from[1] + (to[1] - from[1]) * weight,
        from[2] + (to[2] - from[2]) * weight,
    ]);
}

fn apply_character_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
) -> Result<(), String> {
    let Some(clip) = clip else {
        return Ok(());
    };
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sample_time =
        (clip.clip.duration_seconds * phase).clamp(0.0, clip.clip.duration_seconds.max(0.0));
    clip.clip
        .sample_local_pose_bound(sample_time, animation_runtime, &clip.binding, scratch)?;
    for (index, (dst, src)) in target.iter_mut().zip(scratch.iter()).enumerate() {
        let Some(joint) = skeleton.joints.get(index) else {
            continue;
        };
        // Untracked clip channels are bind-pose completion, not authored overlay data. Preserve
        // the current base locomotion pose unless this clip explicitly owns the joint tag.
        if clip.clip.joint_tags.contains(&joint.tag) {
            blend_joint_rotation_only(dst, src, 1.0);
        }
    }
    Ok(())
}

fn apply_equipment_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    weights: &[ResolvedJointBlendRule],
    weight_scale: f32,
) -> Result<(), String> {
    let Some(clip) = clip else {
        return Ok(());
    };
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sample_time =
        (clip.clip.duration_seconds * phase).clamp(0.0, clip.clip.duration_seconds.max(0.0));
    clip.clip
        .sample_local_pose_bound(sample_time, animation_runtime, &clip.binding, scratch)?;
    for rule in weights {
        // Sampling returns a complete pose by filling absent clip channels from bind pose. That
        // fallback is not layer-authored data. A project layer may only modify a joint when the
        // selected clip explicitly owns that joint tag; otherwise the live base pose is preserved.
        if !clip.clip.joint_tags.contains(&rule.joint_tag) {
            continue;
        }
        if let (Some(dst), Some(src)) = (
            target.get_mut(rule.joint_index),
            scratch.get(rule.joint_index),
        ) {
            let effective_weight = (rule.weight * weight_scale).clamp(0.0, 1.0);
            if rule.channels.translation {
                blend_joint_translation_only(dst, src, effective_weight);
            }
            if rule.channels.rotation {
                blend_joint_rotation_only(dst, src, effective_weight);
            }
            if rule.channels.scale {
                blend_joint_scale_only(dst, src, effective_weight);
            }
        }
    }
    Ok(())
}
