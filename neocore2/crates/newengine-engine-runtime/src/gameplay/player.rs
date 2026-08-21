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
    ensure_player_inventory, CharacterBody, CharacterMotionTuning, CollisionShapeDesc, DisplayMode,
    DisplayVisibility, GameplayActor, Health, PhysicsBodyDesc, PhysicsSurface, PlayerActor,
    PlayerAnimationState, PlayerCommandFrame, PlayerController, PlayerEventKind, PlayerGroundState,
    PlayerLocomotionAnimation, PlayerLocomotionState, PlayerModelAssignment, PlayerModelBinding,
    PlayerStanceKind, PlayerStanceState, PlayerViewVisibility, PlayerVisualKind, PlayerVisualPart,
};

#[path = "player/animation.rs"]
mod animation;
#[path = "player/camera.rs"]
mod camera;
#[path = "player/input.rs"]
mod input;
#[path = "player/model_assignment.rs"]
mod model_assignment;
#[path = "player/spawn.rs"]
mod spawn;
#[path = "player/stance.rs"]
mod stance;

pub use animation::update_player_animation_states;
pub use camera::{
    attach_active_camera_to_player, detach_active_camera_from_player, display_visible_in_mode,
};
pub use input::{
    apply_player_command_frame, apply_player_input, clear_player_input,
    consume_player_transient_input, first_player, is_player_controller_enabled,
};
pub use model_assignment::{clear_player_model_assignment, set_player_model_assignment};
pub use spawn::{
    ensure_physics_body, remove_physics_body, spawn_default_player, spawn_player_controller,
};
pub use stance::{apply_player_stance_geometry, update_player_stance_camera};

#[cfg(test)]
include!("player/tests.rs");
