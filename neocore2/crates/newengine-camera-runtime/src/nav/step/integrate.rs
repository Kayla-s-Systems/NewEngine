use newengine_camera::{CameraRig, EditorNavController, EditorNavMode};
use newengine_ecs::{EntityId, World};
use newengine_sim::FollowTargetCameraController;

use crate::nav::helpers::retarget_follow_to_rig;
use crate::nav::input::build_camera_input;
use crate::nav::CameraNavInput;

#[inline]
pub(crate) fn integrate_nav(
    world: &mut World,
    cam_id: EntityId,
    input: &CameraNavInput,
    dt: f32,
    mode: EditorNavMode,
    ctrl: &mut EditorNavController,
    rig: &mut CameraRig,
    follow_ctrl: Option<FollowTargetCameraController>,
) {
    let cam_input = build_camera_input(input, mode);
    ctrl.step(rig, cam_input, dt);

    if mode == EditorNavMode::Fly {
        if let Some(follow) = follow_ctrl {
            let _ = retarget_follow_to_rig(world, cam_id, follow, rig);
        }
    }
}