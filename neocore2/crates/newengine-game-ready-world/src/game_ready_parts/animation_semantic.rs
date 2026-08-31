use newengine_animation_api::AnimationSemanticEventV1;

#[derive(Clone, Debug, Default)]
pub(crate) struct GameReadyAnimationSemanticFrame {
    pub(crate) events: Vec<AnimationSemanticEventV1>,
}

pub(crate) fn capture_animation_semantic_frame(world: &mut newengine_ecs::World) {
    let events = newengine_engine_runtime::gameplay::drain_animation_semantic_events(world);
    world.insert_resource(GameReadyAnimationSemanticFrame { events });
}

pub(crate) fn semantic_events_for_entity(
    world: &newengine_ecs::World,
    stable_entity: u64,
) -> Vec<AnimationSemanticEventV1> {
    world
        .resource::<GameReadyAnimationSemanticFrame>()
        .map(|frame| {
            frame
                .events
                .iter()
                .filter(|event| event.entity.stable_id == stable_entity)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
