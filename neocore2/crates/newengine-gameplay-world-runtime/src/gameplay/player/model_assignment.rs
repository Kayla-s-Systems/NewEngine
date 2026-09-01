use super::*;

/// Assigns (or replaces) the desired runtime avatar for a player entity.
///
/// The assignment is revisioned. Product/world packages observe the revision and
/// rebuild only the presentation binding; character controller, physics, inventory
/// and possession remain attached to the same `PlayerActor` entity.
pub fn set_player_model_assignment(
    world: &mut World,
    player: EntityId,
    assignment: PlayerModelAssignment,
) -> Result<u64, String> {
    if world.get::<PlayerActor>(player).is_none() {
        return Err(format!(
            "player model assignment target {} is not a PlayerActor",
            player.stable_u64()
        ));
    }

    let assignment = assignment.next_revision_after(world.get::<PlayerModelAssignment>(player));
    let revision = assignment.revision;
    let source = assignment.source.clone();
    let _ = world.insert(player, assignment);
    emit_player_event(
        world,
        player,
        PlayerEventKind::ModelAssignmentChanged,
        format!("revision={revision} source='{source}'"),
    );
    Ok(revision)
}

/// Clears the desired avatar while keeping the player/controller entity alive.
/// The active world package removes the current visual binding on its next frame.
pub fn clear_player_model_assignment(world: &mut World, player: EntityId) -> Result<u64, String> {
    set_player_model_assignment(world, player, PlayerModelAssignment::default())
}
