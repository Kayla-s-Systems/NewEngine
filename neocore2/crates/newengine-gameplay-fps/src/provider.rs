#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use newengine_ecs::EntityId;
use newengine_engine_runtime::gameplay::{
    step_world_items, GameplayExecutionPhase, GameplayFrame, GameplayPhysicsQueryProvider,
    GameplaySystemProvider, GameplayWorld,
};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto};

use crate::character_control::apply_fps_character_commands;
use crate::character_physics::{
    collect_character_queries, resolve_character_query_hits, step_character_locomotion,
    sync_physics_world_settings,
};
use crate::content::ensure_fps_player_loadouts;
use crate::inventory_hud::step_inventory_commands;
use crate::{step_fps_demo_gameplay, step_player_combat, step_projectile_sphere_launcher};

/// FPS gameplay execution package selected by a runtime profile.
pub struct FpsGameplayProvider;

impl FpsGameplayProvider {
    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl GameplaySystemProvider for FpsGameplayProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "newengine.gameplay.fps"
    }

    fn run_phase(
        &self,
        phase: GameplayExecutionPhase,
        world: &mut GameplayWorld,
        frame: GameplayFrame,
    ) {
        match phase {
            GameplayExecutionPhase::BeforePhysics => {
                sync_physics_world_settings(world);
                ensure_fps_player_loadouts(world);
                apply_fps_character_commands(world, frame.dt, frame.fixed_tick);
                step_inventory_commands(world, frame.fixed_tick);
                step_world_items(world, frame.dt);
                step_player_combat(world, frame.dt, frame.fixed_tick);
                step_projectile_sphere_launcher(world, frame.dt);
            }
            GameplayExecutionPhase::AfterPhysics => {
                step_character_locomotion(world, frame.dt);
            }
            GameplayExecutionPhase::AfterDerived => {
                step_fps_demo_gameplay(world, frame.dt);
            }
        }
    }
}

impl GameplayPhysicsQueryProvider for FpsGameplayProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "newengine.gameplay.fps.physics-queries"
    }

    fn collect_queries(&self, world: &GameplayWorld) -> Vec<PhysicsQueryDto> {
        let mut queries = collect_character_queries(world);
        queries.extend(crate::combat::collect_combat_queries(world));
        queries
    }

    fn resolve_query_hits(
        &self,
        world: &mut GameplayWorld,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64> {
        let mut consumed =
            crate::combat::resolve_combat_queries(world, fixed_tick, hits, key_to_entity);
        consumed.extend(resolve_character_query_hits(
            world,
            fixed_tick,
            hits,
            key_to_entity,
        ));
        consumed
    }
}
