use newengine_animation_api::AnimationSemanticEventV1;

#[derive(Clone, Debug, Default)]
pub struct FpsAnimationSemanticFrame {
    pub events: Vec<AnimationSemanticEventV1>,
}

pub fn capture_animation_semantic_frame(world: &mut newengine_ecs::World) {
    let events = newengine_engine_runtime::gameplay::drain_animation_semantic_events(world);
    world.insert_resource(FpsAnimationSemanticFrame { events });
}

pub fn semantic_events_for_entity(
    world: &newengine_ecs::World,
    stable_entity: u64,
) -> Vec<AnimationSemanticEventV1> {
    world
        .resource::<FpsAnimationSemanticFrame>()
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
