use newengine_ecs::World;
use newengine_game_data::{GameData, GameDataSnapshot};

/// Resolves the immutable project-authored scene snapshot without invoking the source provider.
///
/// Missing data is not replaced with an embedded game. Callers must fail closed until the
/// selected project has installed its `GameDataSnapshot`.
#[inline]
pub(crate) fn active_game_data(world: &World) -> Option<&GameData> {
    world
        .resource::<GameDataSnapshot>()
        .map(GameDataSnapshot::data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_project_snapshot_has_no_runtime_fallback() {
        let world = World::new();
        assert!(active_game_data(&world).is_none());
    }

    #[test]
    fn active_snapshot_is_project_authority() {
        let mut world = World::new();
        let mut data = GameData::default();
        data.player.move_speed = 19.5;
        world.insert_resource(GameDataSnapshot::new("test.project", data));
        assert_eq!(active_game_data(&world).unwrap().player.move_speed, 19.5);
    }
}
