#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerVisualKind {
    #[default]
    RuntimeModelPart,
    FallbackCapsule,
    EquippedWeapon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerVisualPart {
    pub owner: newengine_ecs::EntityId,
    pub part_index: u32,
    pub kind: PlayerVisualKind,
    pub material_slot: String,
}

/// Eight-influence linear blend skinning vertex payload owned by the engine runtime.
/// Joint indices address the stable authored skeleton joint table. The first quartet
/// is backward-compatible with YDD V3; the second is populated by YDD V4 sources.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSkinVertex {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
    pub joints_extra: [u16; 4],
    pub weights_extra: [f32; 4],
}

/// Skin stream attached to one runtime player visual part. The owner points at the
/// PlayerActor that carries the current palette; source_to_model is retained for
/// diagnostics/validation and must match the pose binding used to build the palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSkinBinding {
    pub owner: newengine_ecs::EntityId,
    pub vertices: Vec<PlayerSkinVertex>,
    pub source_to_model: [f32; 16],
}

/// Per-player matrix palette produced once per frame by the animation backend and
/// consumed by every skinned visual part owned by that player.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerSkinPose {
    pub palette: Vec<newengine_math::Mat4>,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerViewVisibilityPolicy {
    #[default]
    AlwaysVisible,
    HideInFirstPerson,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerViewVisibility {
    pub base_mode: DisplayMode,
    pub policy: PlayerViewVisibilityPolicy,
}

/// Optional local-owner geometry pair for full-body first-person presentation.
///
/// The world primitive is restored for third-person presentation. While first person is active,
/// first-person primitive is a derived topology variant that may remove camera-near head/neck
/// triangles and seal the resulting neckline while keeping the same material and skin contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerFirstPersonPrimitiveVariant {
    pub world_primitive: newengine_primitives::PrimitiveId,
    pub first_person_primitive: newengine_primitives::PrimitiveId,
}

/// Presentation signal published by the camera gateway for systems that need to distinguish
/// first-person view-model presentation from world/third-person attachment. This deliberately
/// carries no camera implementation types across the gameplay boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerViewState {
    pub first_person_active: bool,
}

impl Default for PlayerViewState {
    #[inline]
    fn default() -> Self {
        // CameraViewMode defaults to FirstPerson, so startup presentation must agree before the
        // first camera-gateway frame publishes an explicit state.
        Self {
            first_person_active: true,
        }
    }
}

impl PlayerViewVisibility {
    #[inline]
    pub const fn runtime_model_default() -> Self {
        Self {
            base_mode: DisplayMode::GameOnly,
            policy: PlayerViewVisibilityPolicy::AlwaysVisible,
        }
    }

    #[inline]
    pub const fn fallback_capsule_default() -> Self {
        Self {
            base_mode: DisplayMode::GameOnly,
            policy: PlayerViewVisibilityPolicy::HideInFirstPerson,
        }
    }
}

impl Default for PlayerViewVisibility {
    #[inline]
    fn default() -> Self {
        Self::runtime_model_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerEventKind {
    Spawned,
    ModelAssignmentChanged,
    ModelBound,
    AnimationStateChanged,
    Possessed,
    Released,
    InputApplied,
    GroundStateChanged,
    FallStarted,
    FallEnded,
    Footstep,
    Landed,
    StanceChanged,
    StanceBlocked,
    VisualVisibilityChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerEvent {
    pub entity: newengine_ecs::EntityId,
    pub kind: PlayerEventKind,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerEventBus {
    pub events: Vec<PlayerEvent>,
}

impl PlayerEventBus {
    #[inline]
    pub fn emit(
        &mut self,
        entity: newengine_ecs::EntityId,
        kind: PlayerEventKind,
        message: impl Into<String>,
    ) {
        const MAX_RETAINED_EVENTS: usize = 256;
        if self.events.len() >= MAX_RETAINED_EVENTS {
            let overflow = self.events.len() + 1 - MAX_RETAINED_EVENTS;
            self.events.drain(0..overflow);
        }
        self.events.push(PlayerEvent {
            entity,
            kind,
            message: message.into(),
        });
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<PlayerEvent> {
        std::mem::take(&mut self.events)
    }
}
