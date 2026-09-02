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
