use std::collections::BTreeSet;
use std::sync::Arc;

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    capture_runtime_world_snapshot, give_item, publish_gameplay_event, request_gameplay_capability,
    restore_runtime_world_snapshot, GameplayCapabilityRequest, GameplayEvent, Health, ItemCatalog,
    ItemId,
};
use newengine_entity_api::{EntitySpawnRequest, EntitySpawnTransform};
use newengine_entity_runtime::{default_entity_archetype_registry, EntityArchetypeRegistry};
use newengine_gameplay_script_api::{
    GameplayCommand, GameplayCommandBuffer, GameplayCommandReceipt,
};

use crate::resources::{
    GameplayEffectBus, GameplayEffectRequest, GameplayObjectiveBook, GameplayObjectiveRecord,
};

#[derive(Clone, Debug)]
pub struct GameplayCommandExecutionPolicy {
    pub max_commands: usize,
    pub max_damage_per_command: f32,
    pub max_item_quantity_per_command: u32,
    pub max_spawn_count_per_command: u32,
    pub max_effect_intensity: f32,
    pub allowed_archetype_prefixes: Vec<String>,
    pub allowed_effect_prefixes: Vec<String>,
}

impl Default for GameplayCommandExecutionPolicy {
    fn default() -> Self {
        Self {
            max_commands: 64,
            max_damage_per_command: 10_000.0,
            max_item_quantity_per_command: 10_000,
            max_spawn_count_per_command: 32,
            max_effect_intensity: 100.0,
            allowed_archetype_prefixes: Vec::new(),
            allowed_effect_prefixes: Vec::new(),
        }
    }
}

pub struct GameplayCommandExecutor {
    policy: GameplayCommandExecutionPolicy,
    archetypes: Arc<EntityArchetypeRegistry>,
}

impl Default for GameplayCommandExecutor {
    fn default() -> Self {
        Self {
            policy: GameplayCommandExecutionPolicy::default(),
            archetypes: default_entity_archetype_registry(),
        }
    }
}

impl GameplayCommandExecutor {
    #[inline]
    pub fn new(policy: GameplayCommandExecutionPolicy) -> Self {
        Self {
            policy,
            archetypes: default_entity_archetype_registry(),
        }
    }

    #[inline]
    pub fn with_archetypes(
        policy: GameplayCommandExecutionPolicy,
        archetypes: Arc<EntityArchetypeRegistry>,
    ) -> Self {
        Self { policy, archetypes }
    }

    pub fn validate(&self, world: &World, buffer: &GameplayCommandBuffer) -> Result<(), String> {
        buffer.validate_envelope(self.policy.max_commands)?;
        let archetypes = self
            .archetypes
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.id.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();

        for (index, command) in buffer.commands.iter().enumerate() {
            self.validate_command(world, command, &archetypes)
                .map_err(|error| format!("command[{index}] preflight failed: {error}"))?;
        }
        Ok(())
    }

    pub fn execute(
        &self,
        world: &mut World,
        buffer: &GameplayCommandBuffer,
    ) -> Result<GameplayCommandReceipt, String> {
        self.validate(world, buffer)?;

        let world_snapshot = capture_runtime_world_snapshot(world);
        let objective_snapshot = world.resource::<GameplayObjectiveBook>().cloned();
        let effect_snapshot = world.resource::<GameplayEffectBus>().cloned();

        let result = self.apply_buffer(world, buffer);
        match result {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                restore_runtime_world_snapshot(world, world_snapshot);
                restore_resource(world, objective_snapshot);
                restore_resource(world, effect_snapshot);
                Err(format!(
                    "gameplay command transaction '{}' rolled back: {error}",
                    buffer.transaction_id
                ))
            }
        }
    }

    fn validate_command(
        &self,
        world: &World,
        command: &GameplayCommand,
        registered_archetypes: &BTreeSet<String>,
    ) -> Result<(), String> {
        match command {
            GameplayCommand::DealDamage {
                target,
                amount,
                source,
                damage_type,
            } => {
                let target = resolve_entity(world, *target)
                    .ok_or_else(|| format!("damage target entity {} does not exist", target))?;
                if world.get::<Health>(target).is_none() {
                    return Err(format!(
                        "damage target entity {} has no Health component",
                        target.stable_u64()
                    ));
                }
                validate_finite_range(
                    "damage amount",
                    *amount,
                    0.0,
                    self.policy.max_damage_per_command,
                )?;
                if let Some(source) = source {
                    if resolve_entity(world, *source).is_none() {
                        return Err(format!("damage source entity {source} does not exist"));
                    }
                }
                if !damage_type.is_empty() {
                    validate_id("damage_type", damage_type)?;
                }
            }
            GameplayCommand::GiveItem {
                owner,
                item,
                quantity,
            } => {
                if resolve_entity(world, *owner).is_none() {
                    return Err(format!("item owner entity {owner} does not exist"));
                }
                validate_id("item", item)?;
                if *quantity == 0 || *quantity > self.policy.max_item_quantity_per_command {
                    return Err(format!(
                        "item quantity must be in 1..={}, got {}",
                        self.policy.max_item_quantity_per_command, quantity
                    ));
                }
                let item_id =
                    ItemId::from_name(item).ok_or_else(|| format!("invalid item id '{item}'"))?;
                if world
                    .resource::<ItemCatalog>()
                    .and_then(|catalog| catalog.get(item_id))
                    .is_none()
                {
                    return Err(format!("item '{item}' is not registered in ItemCatalog"));
                }
            }
            GameplayCommand::SetObjective {
                objective,
                status,
                progress,
                ..
            } => {
                validate_id("objective", objective)?;
                if status.as_ref().is_some_and(|value| value.len() > 4096) {
                    return Err("objective status exceeds 4096 bytes".to_owned());
                }
                if let Some(progress) = progress {
                    validate_finite_range("objective progress", *progress, 0.0, 1.0)?;
                }
            }
            GameplayCommand::SpawnArchetype {
                archetype,
                count,
                transform,
                tags,
                owner,
                ..
            } => {
                validate_id("archetype", archetype)?;
                if *count == 0 || *count > self.policy.max_spawn_count_per_command {
                    return Err(format!(
                        "spawn count must be in 1..={}, got {}",
                        self.policy.max_spawn_count_per_command, count
                    ));
                }
                let normalized = archetype.to_ascii_lowercase();
                if !registered_archetypes.contains(&normalized) {
                    return Err(format!("unknown entity archetype '{archetype}'"));
                }
                if !prefix_allowed(&normalized, &self.policy.allowed_archetype_prefixes) {
                    return Err(format!(
                        "archetype '{archetype}' is denied by execution policy"
                    ));
                }
                if let Some(transform) = transform {
                    validate_vec3("spawn position", transform.position)?;
                    validate_vec4("spawn rotation", transform.rotation_xyzw)?;
                    validate_vec3("spawn scale", transform.scale)?;
                    if transform.scale.iter().any(|value| *value <= 0.0) {
                        return Err("spawn scale components must be positive".to_owned());
                    }
                }
                if tags.len() > 64 {
                    return Err("spawn tags exceed max count 64".to_owned());
                }
                for tag in tags {
                    validate_id("spawn tag", tag)?;
                }
                if owner.as_ref().is_some_and(|value| value.len() > 256) {
                    return Err("spawn owner exceeds 256 bytes".to_owned());
                }
            }
            GameplayCommand::PublishEvent {
                event,
                source,
                payload,
            } => {
                let mut authored = GameplayEvent::new(event.clone()).with_payload(payload.clone());
                if let Some(source) = source {
                    if resolve_entity(world, *source).is_none() {
                        return Err(format!("event source entity {source} does not exist"));
                    }
                    authored = authored.with_stable_source(*source);
                }
                authored.validate()?;
            }
            GameplayCommand::InvokeCapability {
                capability,
                source,
                target,
                payload,
            } => {
                for (label, entity) in [("source", source), ("target", target)] {
                    if let Some(entity) = entity {
                        if resolve_entity(world, *entity).is_none() {
                            return Err(format!(
                                "capability {label} entity {entity} does not exist"
                            ));
                        }
                    }
                }
                GameplayCapabilityRequest {
                    capability: capability.clone(),
                    source: *source,
                    target: *target,
                    payload: payload.clone(),
                }
                .validate()?;
            }
            GameplayCommand::PlayEffect {
                effect,
                position,
                source,
                target,
                intensity,
                parameters,
            } => {
                validate_id("effect", effect)?;
                if !prefix_allowed(
                    &effect.to_ascii_lowercase(),
                    &self.policy.allowed_effect_prefixes,
                ) {
                    return Err(format!("effect '{effect}' is denied by execution policy"));
                }
                if let Some(position) = position {
                    validate_vec3("effect position", *position)?;
                }
                for (label, entity) in [("source", source), ("target", target)] {
                    if let Some(entity) = entity {
                        if resolve_entity(world, *entity).is_none() {
                            return Err(format!("effect {label} entity {entity} does not exist"));
                        }
                    }
                }
                validate_finite_range(
                    "effect intensity",
                    *intensity,
                    0.0,
                    self.policy.max_effect_intensity,
                )?;
                if parameters.len() > 64 {
                    return Err("effect parameters exceed max count 64".to_owned());
                }
            }
        }
        Ok(())
    }

    fn apply_buffer(
        &self,
        world: &mut World,
        buffer: &GameplayCommandBuffer,
    ) -> Result<GameplayCommandReceipt, String> {
        let mut receipt = GameplayCommandReceipt {
            transaction_id: buffer.transaction_id.clone(),
            ..GameplayCommandReceipt::default()
        };
        for (index, command) in buffer.commands.iter().enumerate() {
            self.apply_command(world, command, &mut receipt)
                .map_err(|error| format!("command[{index}] apply failed: {error}"))?;
            receipt.applied_commands += 1;
        }
        Ok(receipt)
    }

    fn apply_command(
        &self,
        world: &mut World,
        command: &GameplayCommand,
        receipt: &mut GameplayCommandReceipt,
    ) -> Result<(), String> {
        match command {
            GameplayCommand::DealDamage { target, amount, .. } => {
                let target = resolve_entity(world, *target)
                    .ok_or_else(|| format!("damage target entity {target} vanished"))?;
                let applied = world
                    .get_mut::<Health>(target)
                    .ok_or_else(|| format!("damage target entity {target:?} lost Health"))?
                    .apply_damage(*amount);
                receipt.total_damage += applied;
            }
            GameplayCommand::GiveItem {
                owner,
                item,
                quantity,
            } => {
                let owner = resolve_entity(world, *owner)
                    .ok_or_else(|| format!("item owner entity {owner} vanished"))?;
                let item_id =
                    ItemId::from_name(item).ok_or_else(|| format!("invalid item id '{item}'"))?;
                let mutation = give_item(world, owner, item_id, *quantity)?;
                if mutation.accepted != *quantity {
                    return Err(format!(
                        "GiveItem requested {} x{} but inventory accepted only {}",
                        item, quantity, mutation.accepted
                    ));
                }
                receipt.items_given = receipt.items_given.saturating_add(u64::from(*quantity));
            }
            GameplayCommand::SetObjective {
                objective,
                state,
                status,
                progress,
            } => {
                if world.resource::<GameplayObjectiveBook>().is_none() {
                    world.insert_resource(GameplayObjectiveBook::default());
                }
                world
                    .resource_mut::<GameplayObjectiveBook>()
                    .expect("objective book inserted above")
                    .objectives
                    .insert(
                        objective.clone(),
                        GameplayObjectiveRecord {
                            state: *state,
                            status: status.clone(),
                            progress: *progress,
                        },
                    );
                receipt.objectives_touched.push(objective.clone());
            }
            GameplayCommand::SpawnArchetype {
                archetype,
                count,
                transform,
                properties,
                tags,
                owner,
            } => {
                let request = EntitySpawnRequest {
                    count: *count as usize,
                    archetype: archetype.clone(),
                    transform: transform.map(|transform| EntitySpawnTransform {
                        position: transform.position,
                        rotation_xyzw: transform.rotation_xyzw,
                        scale: transform.scale,
                    }),
                    properties: properties.clone(),
                    tags: tags.clone(),
                    owner: owner.clone(),
                };
                for instance_index in 0..request.count {
                    let entity = self.archetypes.spawn(world, &request, instance_index)?;
                    receipt.spawned_entities.push(entity.stable_u64());
                }
            }
            GameplayCommand::PublishEvent {
                event,
                source,
                payload,
            } => {
                let mut authored = GameplayEvent::new(event.clone()).with_payload(payload.clone());
                if let Some(source) = source {
                    authored = authored.with_stable_source(*source);
                }
                publish_gameplay_event(world, authored)?;
                receipt.events_published += 1;
            }
            GameplayCommand::InvokeCapability {
                capability,
                source,
                target,
                payload,
            } => {
                request_gameplay_capability(
                    world,
                    GameplayCapabilityRequest {
                        capability: capability.clone(),
                        source: *source,
                        target: *target,
                        payload: payload.clone(),
                    },
                )?;
                receipt.capability_requests_enqueued += 1;
            }
            GameplayCommand::PlayEffect {
                effect,
                position,
                source,
                target,
                intensity,
                parameters,
            } => {
                if world.resource::<GameplayEffectBus>().is_none() {
                    world.insert_resource(GameplayEffectBus::default());
                }
                world
                    .resource_mut::<GameplayEffectBus>()
                    .expect("effect bus inserted above")
                    .push_bounded(GameplayEffectRequest {
                        effect: effect.clone(),
                        position: *position,
                        source: *source,
                        target: *target,
                        intensity: *intensity,
                        parameters: parameters.clone(),
                    });
                receipt.effects_enqueued += 1;
            }
        }
        Ok(())
    }
}

fn resolve_entity(world: &World, stable_id: u64) -> Option<EntityId> {
    world
        .iter_entities()
        .find(|entity| entity.stable_u64() == stable_id)
}

fn restore_resource<T: Clone + Send + Sync + 'static>(world: &mut World, snapshot: Option<T>) {
    if let Some(snapshot) = snapshot {
        world.insert_resource(snapshot);
    } else {
        let _ = world.remove_resource::<T>();
    }
}

fn prefix_allowed(value: &str, prefixes: &[String]) -> bool {
    prefixes.is_empty()
        || prefixes
            .iter()
            .any(|prefix| value.starts_with(&prefix.to_ascii_lowercase()))
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
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

fn validate_finite_range(label: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!(
            "{label} must be finite in [{min}, {max}], got {value}"
        ));
    }
    Ok(())
}

fn validate_vec3(label: &str, value: [f32; 3]) -> Result<(), String> {
    if value.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(format!("{label} contains non-finite component"))
    }
}

fn validate_vec4(label: &str, value: [f32; 4]) -> Result<(), String> {
    if value.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(format!("{label} contains non-finite component"))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use newengine_engine_runtime::gameplay::{
        inventory_quantity, GameplayCapabilityBus, ItemDefinition, ItemKind, PlayerInventory,
    };
    use newengine_entity_runtime::EntityArchetypeFactory;
    use newengine_gameplay_script_api::GameplayObjectiveState;
    use newengine_gameplay_script_api::{GameplayCommand, GameplaySpawnTransform};

    const TEST_ITEM: &str = "test.script.ammo";

    fn install_test_item(world: &mut World) {
        let definition =
            ItemDefinition::stackable(TEST_ITEM, "Script Ammo", ItemKind::Ammo, 100, 0.01)
                .expect("test item");
        let mut catalog = ItemCatalog::default();
        catalog.register(definition).expect("register item");
        world.insert_resource(catalog);
    }

    #[test]
    fn transaction_applies_all_supported_command_families() {
        let mut world = World::new();
        let actor = world.spawn();
        let target = world.spawn();
        let _ = world.insert(actor, PlayerInventory::default());
        let _ = world.insert(target, Health::new(100.0));
        install_test_item(&mut world);

        let buffer = GameplayCommandBuffer {
            transaction_id: "tx-success".to_owned(),
            commands: vec![
                GameplayCommand::DealDamage {
                    target: target.stable_u64(),
                    amount: 10.0,
                    source: Some(actor.stable_u64()),
                    damage_type: "script.test".to_owned(),
                },
                GameplayCommand::GiveItem {
                    owner: actor.stable_u64(),
                    item: TEST_ITEM.to_owned(),
                    quantity: 2,
                },
                GameplayCommand::SetObjective {
                    objective: "objective.script.test".to_owned(),
                    state: GameplayObjectiveState::Active,
                    status: Some("Running".to_owned()),
                    progress: Some(0.5),
                },
                GameplayCommand::SpawnArchetype {
                    archetype: "entity.empty".to_owned(),
                    count: 1,
                    transform: Some(GameplaySpawnTransform::default()),
                    properties: serde_json::Value::Null,
                    tags: vec!["scripted".to_owned()],
                    owner: Some("test".to_owned()),
                },
                GameplayCommand::PlayEffect {
                    effect: "fx.script.test".to_owned(),
                    position: Some([0.0, 1.0, 0.0]),
                    source: Some(actor.stable_u64()),
                    target: Some(target.stable_u64()),
                    intensity: 1.0,
                    parameters: BTreeMap::new(),
                },
                GameplayCommand::InvokeCapability {
                    capability: "project.test.capability.v1".to_owned(),
                    source: Some(actor.stable_u64()),
                    target: Some(target.stable_u64()),
                    payload: serde_json::json!({"arbitrary": true}),
                },
            ],
            ..GameplayCommandBuffer::default()
        };

        let receipt = GameplayCommandExecutor::default()
            .execute(&mut world, &buffer)
            .expect("transaction");
        assert_eq!(receipt.applied_commands, 6);
        assert_eq!(receipt.spawned_entities.len(), 1);
        assert_eq!(world.get::<Health>(target).unwrap().current, 90.0);
        assert_eq!(
            inventory_quantity(&world, actor, ItemId::from_name(TEST_ITEM).unwrap()),
            2
        );
        assert_eq!(
            world
                .resource::<GameplayObjectiveBook>()
                .unwrap()
                .get("objective.script.test")
                .unwrap()
                .progress,
            Some(0.5)
        );
        assert_eq!(
            world
                .resource::<GameplayEffectBus>()
                .unwrap()
                .pending()
                .len(),
            1
        );
        assert_eq!(receipt.capability_requests_enqueued, 1);
        let capability_bus = world
            .resource::<GameplayCapabilityBus>()
            .expect("capability bus");
        assert_eq!(capability_bus.pending().len(), 1);
        assert_eq!(
            capability_bus.pending()[0].capability,
            "project.test.capability.v1"
        );
    }

    struct FailingArchetype;

    impl EntityArchetypeFactory for FailingArchetype {
        fn id(&self) -> &'static str {
            "test.failing"
        }

        fn spawn(
            &self,
            _world: &mut World,
            _request: &EntitySpawnRequest,
            _instance_index: usize,
        ) -> Result<EntityId, String> {
            Err("intentional archetype failure".to_owned())
        }
    }

    #[test]
    fn apply_failure_rolls_back_prior_mutations() {
        let mut world = World::new();
        let target = world.spawn();
        let _ = world.insert(target, Health::new(100.0));
        let registry = Arc::new(EntityArchetypeRegistry::with_builtins());
        registry
            .register(Arc::new(FailingArchetype))
            .expect("register failing archetype");
        let executor = GameplayCommandExecutor::with_archetypes(
            GameplayCommandExecutionPolicy::default(),
            registry,
        );
        let buffer = GameplayCommandBuffer {
            transaction_id: "tx-rollback".to_owned(),
            commands: vec![
                GameplayCommand::DealDamage {
                    target: target.stable_u64(),
                    amount: 25.0,
                    source: None,
                    damage_type: String::new(),
                },
                GameplayCommand::InvokeCapability {
                    capability: "project.rollback.probe.v1".to_owned(),
                    source: None,
                    target: None,
                    payload: serde_json::json!({"must_rollback": true}),
                },
                GameplayCommand::SpawnArchetype {
                    archetype: "test.failing".to_owned(),
                    count: 1,
                    transform: None,
                    properties: serde_json::Value::Null,
                    tags: Vec::new(),
                    owner: None,
                },
            ],
            ..GameplayCommandBuffer::default()
        };

        let error = executor.execute(&mut world, &buffer).unwrap_err();
        assert!(error.contains("rolled back"));
        assert_eq!(world.get::<Health>(target).unwrap().current, 100.0);
        assert!(
            world
                .resource::<GameplayCapabilityBus>()
                .map(|bus| bus.pending().is_empty())
                .unwrap_or(true),
            "failed transaction must not leak capability side effects"
        );
    }
}
