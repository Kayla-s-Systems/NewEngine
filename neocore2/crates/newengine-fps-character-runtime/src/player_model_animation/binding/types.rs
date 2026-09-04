#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: std::sync::Arc<AnimationClip>,
    binding: AnimationClipBinding,
    event_cursor: AnimationEventCursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum EquipmentAimDirection {
    Forward,
    ForwardRight45,
    Right90,
    BackRight135,
    Back180,
    BackLeft135,
    Left90,
    ForwardLeft45,
}

impl EquipmentAimDirection {
    const ALL: [Self; 8] = [
        Self::Forward,
        Self::ForwardRight45,
        Self::Right90,
        Self::BackRight135,
        Self::Back180,
        Self::BackLeft135,
        Self::Left90,
        Self::ForwardLeft45,
    ];

    #[inline]
    const fn semantic(self) -> &'static str {
        match self {
            Self::Forward => "fw",
            Self::ForwardRight45 => "fw45r",
            Self::Right90 => "r90",
            Self::BackRight135 => "b135r",
            Self::Back180 => "b180",
            Self::BackLeft135 => "b135l",
            Self::Left90 => "l90",
            Self::ForwardLeft45 => "fw45l",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquipmentPoseBodyStance {
    Stand,
    Crouch,
    Prone,
}

impl EquipmentPoseBodyStance {
    #[inline]
    const fn semantic_prefix(self) -> &'static str {
        match self {
            Self::Stand => "",
            Self::Crouch => "crouch.",
            Self::Prone => "prone.",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct EquipmentGripLayerSet {
    reference: Option<PlayerAnimationRuntimeClip>,
    arms: Option<PlayerAnimationRuntimeClip>,
    /// Authored prop-hand frame layer. Despite the source name `hands`, this owns the
    /// l/r_hand_prop attachment domain, not anatomical finger articulation.
    hands: Option<PlayerAnimationRuntimeClip>,
    /// Anatomical finger articulation projected from the character's compact hand domain.
    fingers: Option<PlayerAnimationRuntimeClip>,
    additive: Option<PlayerAnimationRuntimeClip>,
}

impl EquipmentGripLayerSet {
    #[inline]
    fn any(&self) -> bool {
        self.reference.is_some()
            || self.arms.is_some()
            || self.hands.is_some()
            || self.additive.is_some()
    }

    /// TLOU rifle grip authority is established by the terminal prop-domain layer plus the authored
    /// additive correction. `arms` is content/stance-specific: standing Vepr uses ADD+ARMS+HANDS,
    /// while crouch uses ADD+PART, where PART owns the same l/r hand_prop attachment domain.
    /// `reference` is deliberately excluded because Abby's refs live in the foreign 1074-node domain.
    #[inline]
    fn has_prop_socket_contract(&self) -> bool {
        self.hands.is_some() && self.additive.is_some()
    }
}

#[derive(Clone, Debug, Default)]
struct EquipmentAimPoseSpace {
    idle: Option<PlayerAnimationRuntimeClip>,
    movement: std::collections::BTreeMap<EquipmentAimDirection, PlayerAnimationRuntimeClip>,
    grip: EquipmentGripLayerSet,
    blocked_additive: Option<PlayerAnimationRuntimeClip>,
    blocked_subtractive: Option<PlayerAnimationRuntimeClip>,
}

impl EquipmentAimPoseSpace {
    #[inline]
    fn any(&self) -> bool {
        self.idle.is_some()
            || !self.movement.is_empty()
            || self.grip.any()
            || self.blocked_additive.is_some()
            || self.blocked_subtractive.is_some()
    }
}

#[derive(Clone, Debug, Default)]
struct EquipmentTransitionPoseSet {
    ready_to_aim: Option<PlayerAnimationRuntimeClip>,
    aim_to_ready: Option<PlayerAnimationRuntimeClip>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EquipmentTransitionKind {
    ReadyToAim,
    AimToReady,
}

#[derive(Clone, Copy, Debug)]
struct EquipmentTransitionRuntimeState {
    kind: EquipmentTransitionKind,
    elapsed_seconds: f32,
}

#[derive(Clone, Debug, Default)]
struct EquipmentPoseSet {
    /// Legacy/simple family contract retained as a compatibility fallback.
    ready: Option<PlayerAnimationRuntimeClip>,
    aim: Option<PlayerAnimationRuntimeClip>,
    reload: Option<PlayerAnimationRuntimeClip>,
    /// Layered authored weapon pose spaces. The engine knows only generic stance/direction semantics.
    stand: EquipmentAimPoseSpace,
    crouch: EquipmentAimPoseSpace,
    prone: EquipmentAimPoseSpace,
    transitions: EquipmentTransitionPoseSet,
    /// Optional class-specific READY sample phase. `None` inherits the character's generic phase.
    ready_sample_phase: Option<f32>,
}

impl EquipmentPoseSet {
    #[inline]
    fn pose_space(&self, stance: EquipmentPoseBodyStance) -> &EquipmentAimPoseSpace {
        match stance {
            EquipmentPoseBodyStance::Stand => &self.stand,
            EquipmentPoseBodyStance::Crouch => &self.crouch,
            EquipmentPoseBodyStance::Prone => &self.prone,
        }
    }

    #[inline]
    fn pose_space_mut(&mut self, stance: EquipmentPoseBodyStance) -> &mut EquipmentAimPoseSpace {
        match stance {
            EquipmentPoseBodyStance::Stand => &mut self.stand,
            EquipmentPoseBodyStance::Crouch => &mut self.crouch,
            EquipmentPoseBodyStance::Prone => &mut self.prone,
        }
    }

    #[inline]
    fn has_aim(&self) -> bool {
        self.aim.is_some() || self.stand.any() || self.crouch.any() || self.prone.any()
    }

    #[inline]
    fn any(&self) -> bool {
        self.ready.is_some()
            || self.has_aim()
            || self.reload.is_some()
            || self.transitions.ready_to_aim.is_some()
            || self.transitions.aim_to_ready.is_some()
    }
}

#[derive(Clone, Copy, Debug)]
struct PlayerFootJointBinding {
    left: usize,
    right: usize,
}

#[derive(Clone, Copy, Debug)]
struct ResolvedJointBlendRule {
    joint_index: usize,
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
