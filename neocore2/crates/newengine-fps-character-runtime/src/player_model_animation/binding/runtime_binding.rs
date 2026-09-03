#[derive(Clone, Copy, Debug, Default)]
struct EquipmentRelativeAdsState {
    view_rotation_model_at_entry: Option<Quat>,
    sight_forward_model_at_entry: Option<Vec3>,
}

impl EquipmentRelativeAdsState {
    #[inline]
    fn update_activation(&mut self, active: bool, view_rotation_model: Option<Quat>) {
        if !active {
            *self = Self::default();
            return;
        }
        if self.view_rotation_model_at_entry.is_none() {
            self.view_rotation_model_at_entry = view_rotation_model
                .filter(|rotation| rotation.is_finite())
                .map(|rotation| rotation.normalize_or_identity());
        }
    }

    #[inline]
    fn capture_entry_sight_if_unset(&mut self, sight_forward_model: Option<Vec3>) {
        if self.sight_forward_model_at_entry.is_some()
            || self.view_rotation_model_at_entry.is_none()
        {
            return;
        }
        self.sight_forward_model_at_entry = sight_forward_model
            .filter(|forward| forward.is_finite())
            .map(Vec3::normalize_or_zero)
            .filter(|forward| forward.length_squared() > 1.0e-8);
    }

    #[inline]
    fn relative_sight_target(
        &mut self,
        current_view_rotation_model: Quat,
        current_sight_forward_model: Vec3,
    ) -> Option<Vec3> {
        let entry_view = self.view_rotation_model_at_entry?;
        if !current_view_rotation_model.is_finite() || !current_sight_forward_model.is_finite() {
            return None;
        }
        let current_sight = current_sight_forward_model.normalize_or_zero();
        if current_sight.length_squared() <= 1.0e-8 {
            return None;
        }
        let entry_sight = match self.sight_forward_model_at_entry {
            Some(forward) => forward,
            None => {
                self.sight_forward_model_at_entry = Some(current_sight);
                current_sight
            }
        };
        let view_delta = (current_view_rotation_model.normalize_or_identity()
            * entry_view.inverse())
        .normalize_or_identity();
        let target = (view_delta * entry_sight).normalize_or_zero();
        (target.is_finite() && target.length_squared() > 1.0e-8).then_some(target)
    }
}

pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    animation_event_bindings: std::collections::BTreeMap<String, String>,
    semantic_input: PlayerAnimationSemanticInput,
    consumed_pulse_sequence: u64,
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
    skeletal_secondary_motion: Option<SkeletalSecondaryMotionRuntime>,
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
    landing_active_distance: f32,
    landing_active_downward_speed: f32,
    landing_active_horizontal_speed: f32,
    landing_last_revision: u64,
    fall_medium_min_distance: f32,
    fall_high_min_distance: f32,
    fall_active_band: Option<FallPresentationBand>,
    fall_time_seconds: f32,
    /// Generic `equipment.ready/aim/reload` compatibility fallback.
    equipment_default_pose_set: EquipmentPoseSet,
    /// Open-ended project-authored family sets selected by the equipped item's `weapon_class`.
    /// Keys are normalized class ids such as `pistol` or `rifle`; runtime never enumerates them.
    equipment_pose_sets: std::collections::BTreeMap<String, EquipmentPoseSet>,
    unarmed_ready_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_sequence: u64,
    unarmed_attack_time_seconds: f32,
    equipment_ready_sample_phase: f32,
    equipment_time_seconds: f32,
    equipment_reload_active: bool,
    equipment_previous_stance: EquipmentPresentationStance,
    equipment_transition: Option<EquipmentTransitionRuntimeState>,
    /// Last published equipment selection diagnostic. This is transition state only; it never
    /// participates in pose selection and exists to make live capability routing auditable.
    equipment_trace_active: bool,
    equipment_trace_family: Option<String>,
    equipment_trace_stance: EquipmentPresentationStance,
    equipment_ready_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_aim_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_reload_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_overlay_locals: Vec<JointLocalPose>,
    equipment_overlay_locals_b: Vec<JointLocalPose>,
    equipment_ik: Option<WeaponArmIkRig>,
    /// Cooldown for significant support-IK residual diagnostics. The solve still runs every frame,
    /// but a persistent authored-contact problem must not flood the runtime log.
    equipment_ik_residual_diag_cooldown: f32,
    /// Relative RMB/ADS anchor. Entry view+sight are captured once so the complete rifle chain follows
    /// mouse/view deltas from its current pose instead of snapping to absolute camera-forward.
    equipment_relative_ads: EquipmentRelativeAdsState,
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

#[inline]
fn select_equipment_pose_set<'a>(
    default_set: &'a EquipmentPoseSet,
    pose_sets: &'a std::collections::BTreeMap<String, EquipmentPoseSet>,
    family: Option<&str>,
) -> Option<&'a EquipmentPoseSet> {
    match family {
        Some(family) => pose_sets.get(family),
        None => Some(default_set),
    }
}

#[inline]
fn select_equipment_pose_set_mut<'a>(
    default_set: &'a mut EquipmentPoseSet,
    pose_sets: &'a mut std::collections::BTreeMap<String, EquipmentPoseSet>,
    family: Option<&str>,
) -> Option<&'a mut EquipmentPoseSet> {
    match family {
        Some(family) => pose_sets.get_mut(family),
        None => Some(default_set),
    }
}

#[inline]
fn equipment_ready_sample_phase_for_pose_set(
    set: Option<&EquipmentPoseSet>,
    generic_phase: f32,
) -> f32 {
    set.and_then(|set| set.ready_sample_phase)
        .unwrap_or(generic_phase)
        .clamp(0.0, 1.0)
}

impl PlayerAnimationRuntimeBinding {
    #[inline]
    fn equipment_pose_set(&self, family: Option<&str>) -> Option<&EquipmentPoseSet> {
        select_equipment_pose_set(
            &self.equipment_default_pose_set,
            &self.equipment_pose_sets,
            family,
        )
    }

    #[inline]
    fn has_equipment_pose_for_family(&self, family: Option<&str>) -> bool {
        self.equipment_pose_set(family)
            .is_some_and(EquipmentPoseSet::any)
    }

    pub(super) fn consume_semantic_event(
        &mut self,
        event: &newengine_animation_api::AnimationSemanticEventV1,
    ) -> Result<bool, String> {
        self.semantic_input
            .consume(&self.animation_event_bindings, event)
    }

    pub(super) fn consume_semantic_events<'a>(
        &mut self,
        events: impl IntoIterator<Item = &'a newengine_animation_api::AnimationSemanticEventV1>,
    ) -> Result<usize, String> {
        let mut accepted = 0usize;
        for event in events {
            accepted += usize::from(self.consume_semantic_event(event)?);
        }
        Ok(accepted)
    }

    pub(super) fn seed_semantic_state(
        &mut self,
        events: &[newengine_animation_api::AnimationSemanticEventV1],
    ) -> Result<usize, String> {
        self.consume_semantic_events(events.iter())
    }

    pub(super) fn authored_capabilities(
        &self,
    ) -> newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities {
        newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities {
            unarmed_ready: self.unarmed_ready_pose.is_some(),
            unarmed_attack: self.unarmed_attack_pose.is_some(),
            equipment_ready: self.equipment_default_pose_set.ready.is_some()
                || self
                    .equipment_pose_sets
                    .values()
                    .any(|set| set.ready.is_some()),
            equipment_aim: self.equipment_default_pose_set.has_aim()
                || self
                    .equipment_pose_sets
                    .values()
                    .any(EquipmentPoseSet::has_aim),
            equipment_reload: self.equipment_default_pose_set.reload.is_some()
                || self
                    .equipment_pose_sets
                    .values()
                    .any(|set| set.reload.is_some()),
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
