use super::*;

#[inline]
pub fn first_player(world: &World) -> Option<EntityId> {
    let mut best: Option<EntityId> = None;
    for (id, _) in world.query::<PlayerActor>() {
        match best {
            Some(cur) if cur.stable_u64() <= id.stable_u64() => {}
            _ => best = Some(id),
        }
    }
    best
}

#[inline]
pub fn is_player_controller_enabled(world: &World, player: EntityId) -> bool {
    world
        .get::<PlayerController>(player)
        .map(|controller| controller.enabled)
        .unwrap_or(true)
}

#[inline]
pub fn clear_player_input(world: &mut World, player: EntityId) {
    if let Some(input) = world.get_mut::<MotorInput>(player) {
        *input = MotorInput::default();
    }
}

#[inline]
pub fn apply_player_command_frame(
    world: &mut World,
    player: EntityId,
    source_frame: u64,
    actions: ActionCommandFrame,
) {
    if !world.exists(player) {
        return;
    }
    if let Some(pending) = world.get_mut::<PlayerCommandFrame>(player) {
        pending.source_frame = pending.source_frame.max(source_frame);
        pending.actions.merge_pending(actions);
    } else {
        let _ = world.insert(player, PlayerCommandFrame::new(source_frame, actions));
    }
}

pub fn apply_player_input(
    world: &mut World,
    player: EntityId,
    input_mask: u64,
    look_delta_px: Vec2,
    look_active: bool,
) {
    if !is_player_controller_enabled(world, player) {
        clear_player_input(world, player);
        return;
    }

    let mut axis = Vec3::ZERO;

    if input_mask & input_move::FORWARD != 0 {
        axis.z += 1.0;
    }
    if input_mask & input_move::BACK != 0 {
        axis.z -= 1.0;
    }
    if input_mask & input_move::RIGHT != 0 {
        axis.x += 1.0;
    }
    if input_mask & input_move::LEFT != 0 {
        axis.x -= 1.0;
    }
    if input_mask & input_move::UP != 0 {
        axis.y += 1.0;
    }
    if input_mask & input_move::DOWN != 0 {
        axis.y -= 1.0;
    }

    let sprint_multiplier = world
        .get::<CharacterMotionTuning>(player)
        .copied()
        .unwrap_or_default()
        .sanitized()
        .sprint_multiplier;

    let mut applied = false;
    if let Some(input) = world.get_mut::<MotorInput>(player) {
        input.move_axis = axis;
        input.look_delta += look_delta_px;
        input.look_active = look_active;
        input.speed_mul = if input_mask & input_move::SPRINT != 0 {
            sprint_multiplier
        } else {
            1.0
        };
        input.zoom_delta = 0.0;
        applied = true;
    }
    if applied {
        emit_player_event(
            world,
            player,
            PlayerEventKind::InputApplied,
            "local input sampled",
        );
    }
}

/// Consumes render-frame pulses after one fixed simulation step.
pub fn consume_player_transient_input(world: &mut World) {
    let players = world
        .query2_ids::<PlayerController, MotorInput>()
        .collect::<Vec<_>>();
    for player in players {
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.look_delta = Vec2::ZERO;
            input.zoom_delta = 0.0;
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions.clear_pulses();
        }
    }
}
