use super::*;

/// Changes a character capsule while preserving its world-space foot plane.
/// Product gameplay decides *when* a stance transition is requested; the engine owns only
/// deterministic geometry/state application.
pub fn apply_player_stance_geometry(
    world: &mut World,
    player: EntityId,
    target: PlayerStanceKind,
    fixed_tick: u64,
) -> bool {
    let character = world
        .get::<CharacterBody>(player)
        .copied()
        .unwrap_or_default()
        .sanitized();
    let Some(mut body) = world.get::<PhysicsBodyDesc>(player).copied() else {
        return false;
    };
    let CollisionShapeDesc::Capsule {
        radius,
        half_height: current_half_height,
    } = body.shape
    else {
        return false;
    };
    let target_half_height = match target {
        PlayerStanceKind::Standing => character.standing_half_height,
        PlayerStanceKind::Crouched => character.crouched_half_height,
    };
    let delta_y = target_half_height - current_half_height;

    if delta_y.abs() > 1.0e-6 {
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y += delta_y;
        }
        body.shape = CollisionShapeDesc::Capsule {
            radius,
            half_height: target_half_height,
        };
        ensure_physics_body(world, player, body);
    }

    if world.get::<PlayerStanceState>(player).is_none() {
        let _ = world.insert(
            player,
            PlayerStanceState::standing(character.standing_eye_height),
        );
    }
    if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
        state.current_eye_height = (state.current_eye_height - delta_y).clamp(0.0, 20.0);
        state.current = target;
        state.stand_requested = false;
        state.stand_blocked = false;
        state.target_eye_height = match target {
            PlayerStanceKind::Standing => character.standing_eye_height,
            PlayerStanceKind::Crouched => character.crouched_eye_height,
        };
        state.last_transition_tick = fixed_tick;
    }

    emit_player_event(
        world,
        player,
        PlayerEventKind::StanceChanged,
        match target {
            PlayerStanceKind::Standing => "stance=standing",
            PlayerStanceKind::Crouched => "stance=crouched",
        },
    );
    true
}

/// Smooths generic stance eye offsets and applies them to attached follow cameras.
pub fn update_player_stance_camera(world: &mut World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerStanceState>()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for player in players {
        let speed = world
            .get::<CharacterMotionTuning>(player)
            .copied()
            .unwrap_or_default()
            .sanitized()
            .stance_camera_speed;
        let alpha = if dt > 0.0 {
            1.0 - (-speed * dt).exp()
        } else {
            1.0
        };
        if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
            state.current_eye_height +=
                (state.target_eye_height - state.current_eye_height) * alpha.clamp(0.0, 1.0);
            if (state.target_eye_height - state.current_eye_height).abs() < 1.0e-4 {
                state.current_eye_height = state.target_eye_height;
            }
        }
    }

    let cameras = world
        .query::<FollowTargetCameraController>()
        .map(|(id, ctrl)| (id, ctrl.target))
        .collect::<Vec<_>>();
    for (camera, target) in cameras {
        let Some(eye_height) = world
            .get::<PlayerStanceState>(target)
            .map(|state| state.current_eye_height)
        else {
            continue;
        };
        if let Some(controller) = world.get_mut::<FollowTargetCameraController>(camera) {
            controller.offset_ls.y = eye_height;
        }
    }
}
