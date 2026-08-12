use super::*;

#[inline]
pub fn attach_active_camera_to_player(world: &mut World, camera: EntityId, player: EntityId) {
    if !world.exists(camera) || !world.exists(player) {
        return;
    }

    let ctrl = world
        .get::<FollowTargetCameraController>(camera)
        .copied()
        .unwrap_or(FollowTargetCameraController {
            target: player,
            offset_ls: Vec3::new(0.0, 1.6, 4.5),
            rot_offset: Quat::IDENTITY,
            follow_rotation: false,
            smooth_time: 0.08,
            max_speed: 0.0,
        });

    let mut next = ctrl;
    next.target = player;
    let eye_height = world
        .get::<PlayerStanceState>(player)
        .map(|state| state.current_eye_height)
        .or_else(|| {
            world
                .get::<CharacterBody>(player)
                .map(|body| body.standing_eye_height)
        })
        .unwrap_or_else(|| CharacterBody::default().standing_eye_height);
    next.offset_ls = Vec3::new(0.0, eye_height, 0.0);
    next.rot_offset = Quat::IDENTITY;
    next.follow_rotation = true;
    next.smooth_time = 0.0;
    next.max_speed = 0.0;

    let _ = world.insert(camera, next);
    let _ = world.insert(camera, FollowTargetCameraMotor::default());

    if world.get::<CameraRigComp>(camera).is_none() {
        let rig = world
            .get::<Transform>(camera)
            .copied()
            .map(|t| {
                CameraRigComp(newengine_camera::CameraRig {
                    position: t.position,
                    rotation: t.rotation,
                })
            })
            .unwrap_or_default();
        let _ = world.insert(camera, rig);
    }

    emit_player_event(world, player, PlayerEventKind::Possessed, "camera attached");
}

#[inline]
pub fn detach_active_camera_from_player(world: &mut World, camera: EntityId) {
    let target = world
        .get::<FollowTargetCameraController>(camera)
        .map(|ctrl| ctrl.target);
    let _ = world.remove::<FollowTargetCameraController>(camera);
    let _ = world.remove::<FollowTargetCameraMotor>(camera);
    if let Some(player) = target {
        emit_player_event(world, player, PlayerEventKind::Released, "camera detached");
    }
}

#[inline]
pub fn display_visible_in_mode(world: &World, entity: EntityId, runtime: bool) -> bool {
    let vis = world
        .get::<DisplayVisibility>(entity)
        .copied()
        .unwrap_or_default();
    // RuntimeHidden is a hard presentation quarantine. This is important during
    // loading / first-world handoff, where the render controller may still use
    // a non-runtime extraction path while the camera is already first-person.
    // First-person avatar bodies and fallback capsules must not leak as white
    // diagnostic silhouettes in the center of the screen.
    if matches!(vis.mode, DisplayMode::RuntimeHidden) {
        return false;
    }
    if runtime {
        vis.visible_in_game()
    } else {
        vis.visible_in_authoring()
    }
}
