pub use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};

use newengine_bounds::{Aabb, Bounds};
use newengine_ecs::EntityId;
use newengine_input_actions_api::ActionCommandFrame;
use newengine_math::Vec3;
use newengine_transform::Transform;
use std::sync::Arc;

mod display;
mod physics;
mod player;
mod render_environment;
mod run_mode;
mod scene;
mod world_activation;
mod world_runtime;

pub use display::{DisplayMode, DisplayVisibility};
pub use physics::{PhysicsSurface, PhysicsWorldSettings, StaticMeshCollider};
pub use player::{
    CharacterBody, CharacterMotionTuning, PlayerAnimationState, PlayerCharacterPresentation,
    PlayerCommandFrame, PlayerController, PlayerControllerKind, PlayerEvent, PlayerEventBus,
    PlayerEventKind, PlayerFirstPersonCameraAnchor, PlayerFixedPoseHistory, PlayerGroundState,
    PlayerJointRotationWeight, PlayerLocomotionAnimation, PlayerLocomotionState,
    PlayerModelAssignment, PlayerModelBinding, PlayerMovementSpeeds, PlayerRenderPose,
    PlayerSkinBinding, PlayerSkinPose, PlayerSkinVertex, PlayerStanceKind, PlayerStanceState,
    PlayerViewState, PlayerViewVisibility, PlayerViewVisibilityPolicy, PlayerVisualKind,
    PlayerVisualPart,
};
pub use render_environment::{
    CloudShadowRenderState, EnvironmentDomeRenderState, EnvironmentPostFxState,
    SkyCloudProfileRenderState, TerrainMaterialLayers, WorldClearColor,
};
pub use run_mode::GameRunMode;
pub use scene::{
    attach_scene_element_core, attach_scene_object_core, scene_entity_by_role,
    AuthoredMapPlacement, AuthoredMapPlacementCloneSource, AuthoredMapPlacementDirty,
    AuthoredMapPlacementReplicaScaleState, AuthoredMapPlacementSource, GameplayActor, PlayerActor,
    SceneAnchorFollow, SceneEntityAnchor, SceneEntityRole,
};
pub use world_activation::{ResidencyProgress, WorldActivationPhase, WorldActivationState};
pub use world_runtime::{
    ModelRenderComponent, PreparedRenderMesh, PrimitiveGpuEvictionQueue, WorldAssemblyProgress,
};
