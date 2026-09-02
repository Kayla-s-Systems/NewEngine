use newengine_ecs::{EntityId, World};
use newengine_input_actions_api::{move_mask as input_move, ActionCommandFrame};
use newengine_math::{Quat, Vec2, Vec3};
use newengine_primitives::{builtins as prim_builtins, Primitive};
use newengine_scene::components::Name;
use newengine_sim::{
    CameraRigComp, CharacterMotor, FollowTargetCameraController, FollowTargetCameraMotor,
    MotorInput, Velocity,
};
use newengine_transform::{set_parent, Transform};

use super::listeners::emit_player_event;
use super::{
    ensure_player_inventory, AIController, CharacterBody, CharacterControlState,
    CharacterExertionState, CharacterLifeState, CharacterMotionTuning, CollisionShapeDesc,
    DisplayMode, DisplayVisibility, GameplayActor, Health, PhysicsBodyDesc, PhysicsSurface,
    PlayerActor, PlayerAnimationState, PlayerCommandFrame, PlayerController, PlayerEventKind,
    PlayerFallState, PlayerFirstPersonBodyBarrierProfile, PlayerFixedPoseHistory,
    PlayerGroundState, PlayerLocomotionAnimation, PlayerLocomotionState, PlayerModelAssignment,
    PlayerModelBinding, PlayerMovementSpeeds, PlayerRenderPose, PlayerStanceKind,
    PlayerStanceState, PlayerViewState, PlayerViewVisibility, PlayerViewVisibilityPolicy,
    PlayerVisualKind, PlayerVisualPart, Stamina, StaminaTuning,
};

#[path = "player/animation.rs"]
mod animation;
#[path = "player/camera.rs"]
mod camera;
#[path = "player/input.rs"]
mod input;
#[path = "player/model_assignment.rs"]
mod model_assignment;
#[path = "player/presentation.rs"]
mod presentation;
#[path = "player/spawn.rs"]
mod spawn;
#[path = "player/stance.rs"]
mod stance;

pub use animation::{
    player_fall_is_confirmed, update_character_animation_states, update_player_animation_states,
};
pub use camera::{
    attach_active_camera_to_player, detach_active_camera_from_player,
    display_shadow_caster_visible_in_mode, display_visible_in_mode,
};
pub use input::{
    apply_player_command_frame, apply_player_input, clear_player_input,
    consume_player_transient_input, first_player, is_player_controller_enabled,
};
pub use model_assignment::{clear_player_model_assignment, set_player_model_assignment};
pub use presentation::{
    capture_player_fixed_poses, player_render_model_matrix, publish_player_render_poses,
};
pub use spawn::{
    ensure_physics_body, remove_physics_body, spawn_default_player, spawn_player_controller,
};
pub use stance::{apply_player_stance_geometry, update_player_stance_camera};

#[cfg(test)]
include!("player/tests.rs");
