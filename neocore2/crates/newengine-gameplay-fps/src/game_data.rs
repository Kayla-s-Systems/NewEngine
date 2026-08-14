use newengine_ecs::World;
use newengine_game_data::{default_game_data, GameData, GameDataSnapshot};

/// Resolves the immutable scene snapshot without invoking the source provider.
#[inline]
pub(crate) fn active_game_data(world: &World) -> &GameData {
    if let Some(snapshot) = world.resource::<GameDataSnapshot>() {
        snapshot.data()
    } else {
        default_game_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_snapshot_overrides_rust_fallback_without_provider_calls() {
        let mut world = World::new();
        let mut data = GameData::default();
        data.player.move_speed = 19.5;
        world.insert_resource(GameDataSnapshot::new("test.lua", data));
        assert_eq!(active_game_data(&world).player.move_speed, 19.5);
    }
}
