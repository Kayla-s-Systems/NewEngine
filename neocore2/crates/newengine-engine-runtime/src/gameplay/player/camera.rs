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
            focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            follow_rotation: false,
            render_cadence_only: true,
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
    next.focus_offset_ls = Vec3::ZERO;
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

/// Shadow-caster visibility is intentionally not identical to camera presentation visibility.
/// Camera-near first-person avatar shells may be hidden from the main color pass to avoid clipping,
/// but they must remain in world-space shadow passes so the player still has a physically complete
/// CSM/local shadow. `RuntimeHidden` remains a hard reject for every other reason.
#[inline]
pub fn display_shadow_caster_visible_in_mode(
    world: &World,
    entity: EntityId,
    runtime: bool,
) -> bool {
    if !runtime {
        return display_visible_in_mode(world, entity, false);
    }

    let vis = world
        .get::<DisplayVisibility>(entity)
        .copied()
        .unwrap_or_default();
    if !matches!(vis.mode, DisplayMode::RuntimeHidden) {
        return vis.visible_in_game();
    }

    let Some(binding) = world.get::<PlayerViewVisibility>(entity).copied() else {
        return false;
    };
    let first_person_active = world
        .resource::<PlayerViewState>()
        .copied()
        .unwrap_or_default()
        .first_person_active;

    first_person_active
        && binding.policy == PlayerViewVisibilityPolicy::HideInFirstPerson
        && !matches!(binding.base_mode, DisplayMode::RuntimeHidden)
}

#[cfg(test)]
mod shadow_visibility_tests {
    use super::*;

    #[test]
    fn first_person_hidden_avatar_remains_a_shadow_caster() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
        let _ = world.insert(
            entity,
            PlayerViewVisibility {
                base_mode: DisplayMode::GameOnly,
                policy: PlayerViewVisibilityPolicy::HideInFirstPerson,
            },
        );
        world.insert_resource(PlayerViewState {
            first_person_active: true,
        });

        assert!(!display_visible_in_mode(&world, entity, true));
        assert!(display_shadow_caster_visible_in_mode(&world, entity, true));
    }

    #[test]
    fn unrelated_runtime_hidden_entity_does_not_cast() {
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(
            entity,
            DisplayVisibility {
                mode: DisplayMode::RuntimeHidden,
            },
        );
        assert!(!display_shadow_caster_visible_in_mode(&world, entity, true));
    }
}
