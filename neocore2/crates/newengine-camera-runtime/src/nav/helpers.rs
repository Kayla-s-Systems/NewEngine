use newengine_camera::{auto_near_far_from_sphere, Perspective, Projection};
use newengine_ecs::{EntityId, World};
use newengine_math::Vec3;
use newengine_sim::{
    follow_params_from_pose, CameraRigComp, FollowTargetCameraController, FollowTargetCameraMotor,
};
use newengine_transform_api::{read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain, Transform};

use super::BoundsSphere;
use newengine_camera::CameraRig;

#[inline]
pub(crate) fn ensure_camera_rig(world: &mut World, cam_id: EntityId) -> CameraRig {
    if let Some(r) = world.get::<CameraRigComp>(cam_id).copied() {
        return r.0;
    }

    let rig = read_entity_world_pose_local_chain(world, cam_id)
        .map(|(pos, rot)| CameraRig {
            position: pos,
            rotation: rot,
        })
        .unwrap_or_default();

    let _ = world.insert(cam_id, CameraRigComp(rig));
    rig
}

#[inline]
pub(crate) fn persist_camera_pose(world: &mut World, cam_id: EntityId, rig: &CameraRig) {
    write_entity_local_from_world_pose_local_chain(world, cam_id, rig.position, rig.rotation);
}

#[inline]
pub(crate) fn retarget_follow_to_rig(
    world: &mut World,
    cam_id: EntityId,
    mut follow: FollowTargetCameraController,
    rig: &CameraRig,
) -> FollowTargetCameraController {
    if let Some((target_pos, target_rot)) = read_entity_world_pose_local_chain(world, follow.target) {
        follow.follow_rotation = true;

        let (offset_ls, rot_offset) =
            follow_params_from_pose(target_pos, target_rot, rig.position, rig.rotation);

        follow.offset_ls = offset_ls;
        follow.rot_offset = rot_offset;

        let _ = world.insert(cam_id, follow);
        let _ = world.insert(
            cam_id,
            FollowTargetCameraMotor {
                vel_ws: Vec3::ZERO,
            },
        );
    }
    follow
}

#[inline]
pub(crate) fn compute_user_busy(
    last_bounds_center: Vec3,
    last_bounds_radius: f32,
    input_look_drag: bool,
    input_pan_drag: bool,
    input_move_mask: u64,
    input_ui_busy: bool,
    bounds: BoundsSphere,
) -> bool {
    let bounds_center_delta = (bounds.center - last_bounds_center).length();
    let bounds_radius_delta = (bounds.radius - last_bounds_radius).abs();
    let eps = bounds.radius.max(0.001) * 0.0005;
    let bounds_changed = bounds_center_delta > eps || bounds_radius_delta > eps;

    input_look_drag || input_pan_drag || input_move_mask != 0 || input_ui_busy || bounds_changed
}

#[inline]
pub(crate) fn compute_projection(rig: &CameraRig, bounds: BoundsSphere, aspect: f32) -> Projection {
    let fovy = 60.0f32.to_radians();
    let cam_dist = (rig.position - bounds.center).length().max(0.01);
    let (near, far) = auto_near_far_from_sphere(cam_dist, bounds.radius);
    Projection::Perspective(Perspective::new(fovy, aspect, near, far))
}

#[inline]
pub(crate) fn target_has_transform(world: &World, target: EntityId) -> bool {
    world.get::<Transform>(target).is_some()
}