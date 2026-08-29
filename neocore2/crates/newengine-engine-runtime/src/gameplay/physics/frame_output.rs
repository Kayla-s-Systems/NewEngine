use std::collections::BTreeMap;

use newengine_ecs::{EntityId, World};
use newengine_physics_api::{
    PhysicsBodyPoseUpdate, PhysicsBodyVelocityUpdate, PhysicsFrameOutput, PhysicsStepReportDto,
};
use newengine_physics_contracts::{PhysicsContactEvent, PhysicsEvent, PhysicsStepReport};
use newengine_sim::{AngularVelocity, CharacterMotor, Velocity};
use newengine_transform::Transform;

use crate::gameplay::GameplayPhysicsQueryProviderRegistry;

use super::util::{arr_to_quat, arr_to_vec3};

#[path = "frame_output/apply.rs"]
mod apply;
#[path = "frame_output/body_updates.rs"]
mod body_updates;
#[path = "frame_output/report.rs"]
mod report;

pub(super) use apply::apply_frame_output;

use body_updates::{apply_pose_update, apply_velocity_update, contact_from_dto};
use report::report_from_dto;
