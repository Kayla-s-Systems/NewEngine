#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use newengine_ecs::EntityId;
use newengine_engine_runtime::gameplay::{
    step_world_items, GameplayExecutionPhase, GameplayFrame, GameplayPhysicsQueryProvider,
    GameplaySystemProvider, GameplayWorld,
};
use newengine_gameplay_fps_api::{FpsCharacterMenuPolicyProvider, FpsGameplayPolicyProvider};
use newengine_gameplay_script_api::ScriptedGameplayProvider;
use newengine_gameplay_script_runtime::{step_scripted_gameplay, GameplayCommandExecutor};
use newengine_physics_api::{PhysicsQueryDto, PhysicsQueryHitDto};

use newengine_fps_character_runtime::apply_fps_character_commands;
use newengine_fps_character_runtime::{
    collect_character_queries, ensure_footstep_audio_preloaded, resolve_character_query_hits,
    step_character_locomotion, step_fps_noclip_motion, sync_physics_world_settings,
};
use newengine_fps_combat_runtime::step_player_combat;
use newengine_fps_content_runtime::ensure_fps_player_loadouts;
use newengine_fps_inventory_ui_runtime::{
    character_select_is_open, ensure_character_menu_policy, step_inventory_commands,
};
use newengine_fps_objective_runtime::step_fps_objective_events;
use newengine_fps_projectile_runtime::step_projectile_sphere_launcher;

/// FPS gameplay execution package selected by a runtime profile. The provider
/// executes generic Rust mechanisms while policy/callback decisions come from
/// the injected script-policy boundary.
pub struct FpsGameplayProvider {
    policy_provider: Arc<dyn FpsGameplayPolicyProvider>,
    script_provider: Arc<dyn ScriptedGameplayProvider>,
    character_menu_policy_provider: Option<Arc<dyn FpsCharacterMenuPolicyProvider>>,
    command_executor: GameplayCommandExecutor,
}

impl FpsGameplayProvider {
    #[inline]
    pub fn shared(
        policy_provider: Arc<dyn FpsGameplayPolicyProvider>,
        script_provider: Arc<dyn ScriptedGameplayProvider>,
    ) -> Arc<Self> {
        Arc::new(Self {
            policy_provider,
            script_provider,
            character_menu_policy_provider: None,
            command_executor: GameplayCommandExecutor::default(),
        })
    }

    pub fn shared_with_character_menu(
        policy_provider: Arc<dyn FpsGameplayPolicyProvider>,
        script_provider: Arc<dyn ScriptedGameplayProvider>,
        character_menu_policy_provider: Arc<dyn FpsCharacterMenuPolicyProvider>,
    ) -> Arc<Self> {
        Arc::new(Self {
            policy_provider,
            script_provider,
            character_menu_policy_provider: Some(character_menu_policy_provider),
            command_executor: GameplayCommandExecutor::default(),
        })
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
            GameplayExecutionPhase::FrameInput => {
                if let Some(provider) = self.character_menu_policy_provider.as_deref() {
                    ensure_character_menu_policy(world, provider);
                }
                // Menu/inventory UI actions consume the render-frame PlayerCommandFrame.
                // They must not wait for a physics tick: event-driven presentation may
                // legitimately render with fixed_step_count == 0. The UI runtime deduplicates
                // pulses by source_frame, so fixed catch-up/render repeats cannot double-toggle.
                step_inventory_commands(world, frame.fixed_tick);
            }
            GameplayExecutionPhase::BeforePhysics => {
                sync_physics_world_settings(world);
                ensure_footstep_audio_preloaded(world);
                ensure_fps_player_loadouts(world);
                step_scripted_gameplay(
                    world,
                    self.script_provider.as_ref(),
                    &self.command_executor,
                );
                let character_selector_open = character_select_is_open(world);
                if !character_selector_open {
                    apply_fps_character_commands(world, frame.dt, frame.fixed_tick);
                    step_player_combat(world, frame.dt, frame.fixed_tick);
                    step_projectile_sphere_launcher(world, frame.dt);
                }
                // Keep noclip velocity synchronized even while the selector is open. The UI
                // capture zeros MotorInput movement, so this also prevents inertial drift while
                // the checkbox/menu owns focus.
                step_fps_noclip_motion(world, frame.dt);
                step_world_items(world, frame.dt);
            }
            GameplayExecutionPhase::AfterPhysics => {
                step_character_locomotion(world, frame.dt);
            }
            GameplayExecutionPhase::AfterDerived => {
                step_fps_objective_events(
                    world,
                    frame.dt,
                    self.policy_provider.as_ref(),
                    &self.command_executor,
                );
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
        queries.extend(newengine_fps_combat_runtime::collect_combat_queries(world));
        queries
    }

    fn resolve_query_hits(
        &self,
        world: &mut GameplayWorld,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64> {
        let mut consumed = newengine_fps_combat_runtime::resolve_combat_queries(
            world,
            fixed_tick,
            hits,
            key_to_entity,
            self.policy_provider.as_ref(),
            &self.command_executor,
        );
        consumed.extend(resolve_character_query_hits(
            world,
            fixed_tick,
            hits,
            key_to_entity,
        ));
        consumed
    }
}
