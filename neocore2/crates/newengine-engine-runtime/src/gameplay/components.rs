pub use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};

use newengine_bounds::{Aabb, Bounds};
use newengine_ecs::EntityId;
use newengine_input_actions_api::GameplayActionFrame;
use newengine_math::Vec3;
use newengine_transform::Transform;
use std::sync::Arc;

mod display;
mod fps;
mod physics;
mod player;
mod run_mode;
mod scene;

pub use display::{DisplayMode, DisplayVisibility};
pub use fps::{
    FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules, FpsDemoState, FpsDemoTarget,
    FpsPlayerTuning, GameReadyWorldLaunchGate, GameReadyWorldLaunchGatePhase,
};
pub use physics::{PhysicsSurface, StaticMeshCollider};
pub use player::{
    PlayerCommandFrame, PlayerController, PlayerControllerKind, PlayerEvent, PlayerEventBus,
    PlayerEventKind, PlayerGroundState, PlayerLocomotionState, PlayerModelBinding,
    PlayerStanceKind, PlayerStanceState, PlayerViewVisibility, PlayerViewVisibilityPolicy,
    PlayerVisualKind, PlayerVisualPart,
};
pub use run_mode::GameRunMode;
pub use scene::{
    attach_scene_element_core, attach_scene_object_core, scene_entity_by_role, GameplayActor,
    PlayerActor, SceneAnchorFollow, SceneEntityAnchor, SceneEntityRole,
};
