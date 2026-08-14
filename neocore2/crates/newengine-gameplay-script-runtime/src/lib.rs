#![forbid(unsafe_op_in_unsafe_fn)]

//! Transactional runtime for script-produced gameplay commands, actions, abilities and state machines.
//! Scripts own policy; Rust owns validation, authoritative state and ECS mutation.

mod driver;
mod executor;
mod resources;

pub use driver::{
    dispatch_state_machine_event, enqueue_scripted_ability, enqueue_scripted_action,
    register_state_machine_instance, step_scripted_gameplay,
};
pub use executor::{GameplayCommandExecutionPolicy, GameplayCommandExecutor};
pub use resources::{
    GameplayEffectBus, GameplayEffectRequest, GameplayObjectiveBook, GameplayObjectiveRecord,
    ScriptedAbilityQueue, ScriptedActionQueue, ScriptedGameplayOutcome, ScriptedGameplayOutcomeBus,
    ScriptedStateMachineEventQueue, ScriptedStateMachineInstance, ScriptedStateMachineStore,
};
