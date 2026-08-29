#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    spawn_player_controller, CharacterBody, CharacterMotionTuning,
};
use newengine_entity_api::EntitySpawnRequest;
use newengine_entity_runtime::{register_entity_archetype, EntityArchetypeFactory};
use newengine_math::Vec3;

pub(crate) fn register_game_ready_entity_archetypes_best_effort() {
    if let Err(error) = register_entity_archetype(Arc::new(FpsPlayerArchetype)) {
        newengine_ulog_api::ulog::warn!(
            "game-ready archetype registration skipped id='player.fps' err='{}'",
            error
        );
    }
}

struct FpsPlayerArchetype;

impl EntityArchetypeFactory for FpsPlayerArchetype {
    fn id(&self) -> &'static str {
        "player.fps"
    }

    fn owner(&self) -> &'static str {
        "newengine-gameplay-fps"
    }

    fn description(&self) -> &'static str {
        "FPS-controllable character composition; FPS gameplay providers attach authored loadout/rules"
    }

    fn spawn(
        &self,
        world: &mut World,
        request: &EntitySpawnRequest,
        instance_index: usize,
    ) -> Result<EntityId, String> {
        let name = request
            .properties
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("Player/FPS/{instance_index}"));
        Ok(spawn_player_controller(
            world,
            None,
            name,
            Vec3::ZERO,
            CharacterBody::default(),
            CharacterMotionTuning::default(),
            false,
        ))
    }
}
