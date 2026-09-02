use super::*;

fn ai_agent_snapshot(world: &World, agent: EntityId) -> AiAgentSnapshotV1 {
    let position = world
        .get::<Transform>(agent)
        .map(|transform| nav_vec3(transform.position));
    let memory = world
        .get::<TargetMemory>(agent)
        .copied()
        .unwrap_or_default();
    let perception = world
        .get::<PerceptionState>(agent)
        .copied()
        .unwrap_or_default();
    let mut visible_facts = Vec::new();
    if let Some(target) = perception.visible_target {
        let target_position = world
            .get::<Transform>(target)
            .map(|transform| transform.position)
            .unwrap_or(memory.last_known_position);
        visible_facts.push(AiPerceptionFactV1 {
            fact_id: "combat.target.visible".to_owned(),
            tags: Vec::new(),
            value: serde_json::json!({
                "target": target.stable_u64(),
                "position": [target_position.x, target_position.y, target_position.z],
                "distance": perception.candidate_distance,
            }),
        });
    }
    AiAgentSnapshotV1 {
        entity: EntityHandle::new(agent.stable_u64()),
        agent_id: format!("ecs:{:016x}", agent.stable_u64()),
        position,
        velocity: None,
        tags: Vec::new(),
        current_task: None,
        visible_facts,
        blackboard: serde_json::json!({
            "combat": {
                "memory_target": memory.target.map(EntityId::stable_u64),
                "memory_visible": memory.visible,
                "last_known_position": [
                    memory.last_known_position.x,
                    memory.last_known_position.y,
                    memory.last_known_position.z
                ],
                "seconds_since_seen": memory.seconds_since_seen,
            }
        }),
    }
}

pub fn build_ai_frame_input(world: &World, fixed_tick: u64) -> AiFrameInputV1 {
    let mut agents = world
        .query::<AIController>()
        .filter(|(entity, _)| controller_is_operational(world, *entity))
        .map(|(entity, _)| ai_agent_snapshot(world, entity))
        .collect::<Vec<_>>();
    agents.sort_by_key(|agent| agent.entity.stable_id);
    AiFrameInputV1 {
        frame_id: fixed_tick,
        fixed_tick,
        seed: fixed_tick.rotate_left(17) ^ 0x4e53_4149_5041_4931,
        agents,
        world_facts: Vec::new(),
    }
}

fn entity_by_stable_id(world: &World, stable_id: u64) -> Option<EntityId> {
    world
        .iter_entities()
        .find(|entity| entity.stable_u64() == stable_id)
}

fn json_vec3(value: &serde_json::Value) -> Option<Vec3> {
    let array = value.as_array()?;
    if array.len() != 3 {
        return None;
    }
    let x = array[0].as_f64()? as f32;
    let y = array[1].as_f64()? as f32;
    let z = array[2].as_f64()? as f32;
    let vector = Vec3::new(x, y, z);
    vector.is_finite().then_some(vector)
}

fn apply_ai_intent(world: &mut World, intent: &AiIntentDtoV1) {
    let Some(agent) = entity_by_stable_id(world, intent.agent.stable_id) else {
        return;
    };
    if !controller_is_operational(world, agent) {
        clear_ai_runtime_state(world, agent);
        return;
    }
    match &intent.kind {
        AiIntentKind::Idle => {
            set_combat_intent(world, agent, CombatIntentKind::Idle, None, Vec3::ZERO);
        }
        AiIntentKind::Custom(name) if name == "combat.engage" || name == "combat.investigate" => {
            let target = intent
                .payload
                .get("target")
                .and_then(|value| value.as_u64())
                .and_then(|stable_id| entity_by_stable_id(world, stable_id));
            let target_position = intent
                .payload
                .get("target_position")
                .and_then(json_vec3)
                .or_else(|| {
                    target.and_then(|target| {
                        world
                            .get::<Transform>(target)
                            .map(|transform| transform.position)
                    })
                })
                .unwrap_or_else(|| {
                    world
                        .get::<TargetMemory>(agent)
                        .map(|memory| memory.last_known_position)
                        .unwrap_or(Vec3::ZERO)
                });
            let kind = if name == "combat.engage" {
                CombatIntentKind::Engage
            } else {
                CombatIntentKind::Investigate
            };
            set_combat_intent(world, agent, kind, target, target_position);
        }
        _ => {}
    }
}

pub fn apply_ai_frame_output(world: &mut World, output: &AiFrameOutputV1) {
    if !output.accepted {
        return;
    }
    for intent in &output.intents {
        apply_ai_intent(world, intent);
    }
}

pub fn step_ai_decisions(world: &mut World, dt: f32, fixed_tick: u64) {
    let dt = finite_non_negative(dt, 0.0).min(0.25);
    let agents = world
        .query::<AIController>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    let mut due_agents = Vec::new();
    for agent in &agents {
        if !controller_is_operational(world, *agent) {
            clear_ai_runtime_state(world, *agent);
            continue;
        }
        let controller = world
            .get::<AIController>(*agent)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let remaining = (controller.decision_cooldown_remaining - dt).max(0.0);
        if remaining <= 1.0e-6 {
            due_agents.push(*agent);
        }
        let _ = world.insert(
            *agent,
            AIController {
                decision_cooldown_remaining: remaining,
                ..controller
            },
        );
    }
    if due_agents.is_empty() {
        return;
    }

    let due_ids = due_agents
        .iter()
        .map(|entity| entity.stable_u64())
        .collect::<BTreeSet<_>>();
    let mut input = build_ai_frame_input(world, fixed_tick);
    input
        .agents
        .retain(|agent| due_ids.contains(&agent.entity.stable_id));
    if input.agents.is_empty() {
        return;
    }
    let payload = match serde_json::to_vec(&input) {
        Ok(payload) => payload,
        Err(error) => {
            newengine_ulog_api::ulog::warn!("AI frame encode failed: {}", error);
            reset_ai_decision_cooldowns(world, &due_agents);
            return;
        }
    };
    let response = match newengine_core::call_service_v1_optional(
        ENGINE_AI_SERVICE_ID,
        ai_method::FRAME_JSON_V1,
        &payload,
    ) {
        Ok(Some(response)) => response,
        Ok(None) => {
            for agent in &due_agents {
                if controller_is_operational(world, *agent) {
                    set_combat_intent(world, *agent, CombatIntentKind::Idle, None, Vec3::ZERO);
                }
            }
            reset_ai_decision_cooldowns(world, &due_agents);
            return;
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!("engine.ai frame failed: {}", error);
            reset_ai_decision_cooldowns(world, &due_agents);
            return;
        }
    };
    let output: AiFrameOutputV1 = match serde_json::from_slice(&response) {
        Ok(output) => output,
        Err(error) => {
            newengine_ulog_api::ulog::warn!("engine.ai returned invalid frame JSON: {}", error);
            reset_ai_decision_cooldowns(world, &due_agents);
            return;
        }
    };
    apply_ai_frame_output(world, &output);
    reset_ai_decision_cooldowns(world, &due_agents);
}

fn reset_ai_decision_cooldowns(world: &mut World, agents: &[EntityId]) {
    for agent in agents {
        let Some(controller) = world.get::<AIController>(*agent).copied() else {
            continue;
        };
        if !controller_is_operational(world, *agent) {
            continue;
        }
        let controller = controller.sanitized();
        let _ = world.insert(
            *agent,
            AIController {
                decision_cooldown_remaining: controller.decision_interval_seconds,
                ..controller
            },
        );
    }
}
