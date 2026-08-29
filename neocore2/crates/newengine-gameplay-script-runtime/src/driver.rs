use newengine_ecs::World;
use newengine_gameplay_script_api::{
    ScriptedAbilityRequest, ScriptedActionRequest, ScriptedGameplayProvider,
    ScriptedStateMachineEventRequest, ScriptedStateMachineStepRequest,
};

use crate::executor::GameplayCommandExecutor;
use crate::resources::{
    ScriptedAbilityQueue, ScriptedActionQueue, ScriptedGameplayOutcome, ScriptedGameplayOutcomeBus,
    ScriptedStateMachineEventQueue, ScriptedStateMachineInstance, ScriptedStateMachineStore,
};

pub fn enqueue_scripted_action(world: &mut World, request: ScriptedActionRequest) {
    if world.resource::<ScriptedActionQueue>().is_none() {
        world.insert_resource(ScriptedActionQueue::default());
    }
    world
        .resource_mut::<ScriptedActionQueue>()
        .expect("scripted action queue inserted above")
        .pending
        .push(request);
}

pub fn enqueue_scripted_ability(world: &mut World, request: ScriptedAbilityRequest) {
    if world.resource::<ScriptedAbilityQueue>().is_none() {
        world.insert_resource(ScriptedAbilityQueue::default());
    }
    world
        .resource_mut::<ScriptedAbilityQueue>()
        .expect("scripted ability queue inserted above")
        .pending
        .push(request);
}

pub fn register_state_machine_instance(
    world: &mut World,
    instance: ScriptedStateMachineInstance,
) -> Result<(), String> {
    validate_runtime_id("state machine instance", &instance.instance_id)?;
    validate_runtime_id("state machine", &instance.machine)?;
    validate_runtime_id("state", &instance.state)?;
    if world.resource::<ScriptedStateMachineStore>().is_none() {
        world.insert_resource(ScriptedStateMachineStore::default());
    }
    let store = world
        .resource_mut::<ScriptedStateMachineStore>()
        .expect("state machine store inserted above");
    if store.instances.contains_key(&instance.instance_id) {
        return Err(format!(
            "scripted state machine instance '{}' is already registered",
            instance.instance_id
        ));
    }
    store
        .instances
        .insert(instance.instance_id.clone(), instance);
    Ok(())
}

pub fn dispatch_state_machine_event(
    world: &mut World,
    event: ScriptedStateMachineEventRequest,
) -> Result<(), String> {
    validate_runtime_id("state machine instance", &event.instance_id)?;
    validate_runtime_id("state machine event", &event.event)?;
    if !world
        .resource::<ScriptedStateMachineStore>()
        .is_some_and(|store| store.instances.contains_key(&event.instance_id))
    {
        return Err(format!(
            "scripted state machine instance '{}' is not registered",
            event.instance_id
        ));
    }
    if world.resource::<ScriptedStateMachineEventQueue>().is_none() {
        world.insert_resource(ScriptedStateMachineEventQueue::default());
    }
    world
        .resource_mut::<ScriptedStateMachineEventQueue>()
        .expect("state machine event queue inserted above")
        .pending
        .push(event);
    Ok(())
}

pub fn step_scripted_gameplay(
    world: &mut World,
    provider: &dyn ScriptedGameplayProvider,
    executor: &GameplayCommandExecutor,
) {
    ensure_outcome_bus(world);
    step_actions(world, provider, executor);
    step_abilities(world, provider, executor);
    step_state_machines(world, provider, executor);
}

fn step_actions(
    world: &mut World,
    provider: &dyn ScriptedGameplayProvider,
    executor: &GameplayCommandExecutor,
) {
    let pending = world
        .resource_mut::<ScriptedActionQueue>()
        .map(|queue| std::mem::take(&mut queue.pending))
        .unwrap_or_default();
    for request in pending {
        let subject = request.action.clone();
        let outcome = match provider.invoke_action(&request) {
            Ok(commands) => execute_buffer(executor, world, "action", subject, commands),
            Err(error) => failed_outcome("action", subject, error),
        };
        push_outcome(world, outcome);
    }
}

fn step_abilities(
    world: &mut World,
    provider: &dyn ScriptedGameplayProvider,
    executor: &GameplayCommandExecutor,
) {
    let pending = world
        .resource_mut::<ScriptedAbilityQueue>()
        .map(|queue| std::mem::take(&mut queue.pending))
        .unwrap_or_default();
    for request in pending {
        let subject = request.ability.clone();
        let outcome = match provider.invoke_ability(&request) {
            Ok(commands) => execute_buffer(executor, world, "ability", subject, commands),
            Err(error) => failed_outcome("ability", subject, error),
        };
        push_outcome(world, outcome);
    }
}

fn step_state_machines(
    world: &mut World,
    provider: &dyn ScriptedGameplayProvider,
    executor: &GameplayCommandExecutor,
) {
    let pending = world
        .resource_mut::<ScriptedStateMachineEventQueue>()
        .map(|queue| std::mem::take(&mut queue.pending))
        .unwrap_or_default();

    for event in pending {
        let Some(instance) = world
            .resource::<ScriptedStateMachineStore>()
            .and_then(|store| store.instances.get(&event.instance_id))
            .cloned()
        else {
            push_outcome(
                world,
                failed_outcome(
                    "state_machine",
                    event.instance_id,
                    "state machine instance disappeared before processing".to_owned(),
                ),
            );
            continue;
        };

        let request = ScriptedStateMachineStepRequest {
            machine: instance.machine.clone(),
            state: instance.state.clone(),
            actor: instance.actor,
            target: instance.target,
            event: event.event,
            context: event.context,
            variables: instance.variables.clone(),
        };

        let response = match provider.step_state_machine(&request) {
            Ok(response) => response,
            Err(error) => {
                push_outcome(
                    world,
                    failed_outcome("state_machine", instance.instance_id, error),
                );
                continue;
            }
        };

        if response.next_state.trim().is_empty() {
            push_outcome(
                world,
                failed_outcome(
                    "state_machine",
                    instance.instance_id,
                    "scripted state machine returned empty next_state".to_owned(),
                ),
            );
            continue;
        }

        let command_result = if response.commands.commands.is_empty() {
            Ok(None)
        } else {
            executor.execute(world, &response.commands).map(Some)
        };

        match command_result {
            Ok(receipt) => {
                if let Some(store) = world.resource_mut::<ScriptedStateMachineStore>() {
                    if let Some(live) = store.instances.get_mut(&instance.instance_id) {
                        live.state = response.next_state.clone();
                        live.variables = response.variables;
                    }
                }
                let tx = receipt
                    .as_ref()
                    .map(|receipt| receipt.transaction_id.as_str())
                    .unwrap_or("<no-commands>");
                let command_count = receipt
                    .as_ref()
                    .map(|receipt| receipt.applied_commands)
                    .unwrap_or(0);
                newengine_ulog_api::ulog::info!(
                    "scripted gameplay state-machine committed provider='{}' instance='{}' machine='{}' from='{}' event='{}' next='{}' tx='{}' commands={}",
                    provider.id(),
                    instance.instance_id,
                    instance.machine,
                    instance.state,
                    request.event,
                    response.next_state,
                    tx,
                    command_count,
                );
                push_outcome(
                    world,
                    ScriptedGameplayOutcome {
                        kind: "state_machine".to_owned(),
                        subject: instance.instance_id,
                        ok: true,
                        message: "state machine step committed".to_owned(),
                        receipt,
                        next_state: Some(response.next_state),
                    },
                );
            }
            Err(error) => push_outcome(
                world,
                failed_outcome("state_machine", instance.instance_id, error),
            ),
        }
    }
}

fn execute_buffer(
    executor: &GameplayCommandExecutor,
    world: &mut World,
    kind: &str,
    subject: String,
    commands: newengine_gameplay_script_api::GameplayCommandBuffer,
) -> ScriptedGameplayOutcome {
    if commands.commands.is_empty() {
        return ScriptedGameplayOutcome {
            kind: kind.to_owned(),
            subject,
            ok: true,
            message: "script returned no gameplay commands".to_owned(),
            receipt: None,
            next_state: None,
        };
    }
    match executor.execute(world, &commands) {
        Ok(receipt) => {
            newengine_ulog_api::ulog::info!(
                "scripted gameplay transaction committed kind='{}' subject='{}' tx='{}' commands={} damage={:.2} items={} spawned={} objectives={} effects={}",
                kind,
                subject,
                receipt.transaction_id,
                receipt.applied_commands,
                receipt.total_damage,
                receipt.items_given,
                receipt.spawned_entities.len(),
                receipt.objectives_touched.len(),
                receipt.effects_enqueued,
            );
            ScriptedGameplayOutcome {
                kind: kind.to_owned(),
                subject,
                ok: true,
                message: "gameplay command transaction committed".to_owned(),
                receipt: Some(receipt),
                next_state: None,
            }
        }
        Err(error) => failed_outcome(kind, subject, error),
    }
}

fn failed_outcome(kind: &str, subject: String, error: String) -> ScriptedGameplayOutcome {
    ScriptedGameplayOutcome {
        kind: kind.to_owned(),
        subject,
        ok: false,
        message: error,
        receipt: None,
        next_state: None,
    }
}

fn ensure_outcome_bus(world: &mut World) {
    if world.resource::<ScriptedGameplayOutcomeBus>().is_none() {
        world.insert_resource(ScriptedGameplayOutcomeBus::default());
    }
}

fn push_outcome(world: &mut World, outcome: ScriptedGameplayOutcome) {
    ensure_outcome_bus(world);
    world
        .resource_mut::<ScriptedGameplayOutcomeBus>()
        .expect("outcome bus inserted above")
        .push(outcome);
}

fn validate_runtime_id(label: &str, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 {
        return Err(format!("{label} id must contain 1..=256 bytes"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        return Err(format!(
            "{label} id '{value}' contains unsupported characters"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use newengine_gameplay_script_api::{
        GameplayCommand, GameplayCommandBuffer, GameplayObjectiveState,
        ScriptedStateMachineStepResponse,
    };

    struct TestProvider;

    impl ScriptedGameplayProvider for TestProvider {
        fn id(&self) -> &'static str {
            "test.scripted.provider"
        }

        fn invoke_action(
            &self,
            request: &ScriptedActionRequest,
        ) -> Result<GameplayCommandBuffer, String> {
            Ok(GameplayCommandBuffer {
                transaction_id: format!("action:{}", request.action),
                commands: vec![GameplayCommand::SetObjective {
                    objective: "objective.action".to_owned(),
                    state: GameplayObjectiveState::Active,
                    status: Some(request.action.clone()),
                    progress: Some(0.25),
                }],
                ..GameplayCommandBuffer::default()
            })
        }

        fn invoke_ability(
            &self,
            request: &ScriptedAbilityRequest,
        ) -> Result<GameplayCommandBuffer, String> {
            Ok(GameplayCommandBuffer {
                transaction_id: format!("ability:{}", request.ability),
                commands: vec![GameplayCommand::PlayEffect {
                    effect: "fx.ability.test".to_owned(),
                    position: request.origin,
                    source: Some(request.actor),
                    target: request.target,
                    intensity: 1.0,
                    parameters: BTreeMap::new(),
                }],
                ..GameplayCommandBuffer::default()
            })
        }

        fn step_state_machine(
            &self,
            request: &ScriptedStateMachineStepRequest,
        ) -> Result<ScriptedStateMachineStepResponse, String> {
            Ok(ScriptedStateMachineStepResponse {
                next_state: format!("{}.next", request.state),
                commands: GameplayCommandBuffer {
                    transaction_id: format!("machine:{}", request.machine),
                    commands: vec![GameplayCommand::SetObjective {
                        objective: "objective.machine".to_owned(),
                        state: GameplayObjectiveState::Active,
                        status: Some(request.event.clone()),
                        progress: Some(0.5),
                    }],
                    ..GameplayCommandBuffer::default()
                },
                variables: BTreeMap::from([("steps".to_owned(), serde_json::Value::from(1))]),
            })
        }
    }

    #[test]
    fn action_ability_and_state_machine_share_transaction_executor() {
        let mut world = World::new();
        let actor = world.spawn();
        let executor = GameplayCommandExecutor::default();
        enqueue_scripted_action(
            &mut world,
            ScriptedActionRequest {
                action: "action.test".to_owned(),
                actor: actor.stable_u64(),
                ..ScriptedActionRequest::default()
            },
        );
        enqueue_scripted_ability(
            &mut world,
            ScriptedAbilityRequest {
                ability: "ability.test".to_owned(),
                actor: actor.stable_u64(),
                origin: Some([0.0, 0.0, 0.0]),
                ..ScriptedAbilityRequest::default()
            },
        );
        register_state_machine_instance(
            &mut world,
            ScriptedStateMachineInstance::new("machine.instance", "machine.test", "idle"),
        )
        .unwrap();
        dispatch_state_machine_event(
            &mut world,
            ScriptedStateMachineEventRequest {
                instance_id: "machine.instance".to_owned(),
                event: "advance".to_owned(),
                context: serde_json::Value::Null,
            },
        )
        .unwrap();

        step_scripted_gameplay(&mut world, &TestProvider, &executor);

        let outcomes = world.resource::<ScriptedGameplayOutcomeBus>().unwrap();
        assert_eq!(outcomes.pending().len(), 3);
        assert!(outcomes.pending().iter().all(|outcome| outcome.ok));
        assert_eq!(
            world
                .resource::<ScriptedStateMachineStore>()
                .unwrap()
                .get("machine.instance")
                .unwrap()
                .state,
            "idle.next"
        );
    }
}
