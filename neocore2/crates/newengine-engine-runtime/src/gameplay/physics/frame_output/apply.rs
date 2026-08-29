use super::*;

pub(in crate::gameplay::physics) fn apply_frame_output(
    world: &mut World,
    output: PhysicsFrameOutput,
    gameplay_queries: &GameplayPhysicsQueryProviderRegistry,
) {
    let PhysicsFrameOutput {
        fixed_tick,
        pose_updates,
        velocity_updates,
        events,
        query_hits,
        report,
    } = output;
    // Query/contact outputs may reference service-owned terrain colliders that do not carry a
    // native PhysicsBodyDesc component. Resolve against the complete live entity table so surface,
    // damage and contact events are not silently dropped at the host boundary.
    let key_to_entity = world
        .iter_entities()
        .map(|entity| (entity.stable_u64(), entity))
        .collect::<BTreeMap<_, _>>();

    for update in pose_updates {
        apply_pose_update(world, &key_to_entity, update);
    }

    for update in velocity_updates {
        apply_velocity_update(world, &key_to_entity, update);
    }

    // Query meaning belongs to gameplay providers. The reusable physics bridge only
    // transports and deterministically dispatches provider-neutral query results.
    let _ = gameplay_queries.resolve_query_hits(world, fixed_tick, &query_hits, &key_to_entity);

    world.insert_resource(report_from_dto(report, events, &key_to_entity));
}
