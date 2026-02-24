#![forbid(unsafe_op_in_unsafe_fn)]

mod character_motor;
mod camera_follow;

pub use camera_follow::{follow_params_from_pose, step_follow_camera, FollowCameraStep};
pub use character_motor::{step_character_motor, CharacterMotorStep};
