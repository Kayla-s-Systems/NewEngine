#![forbid(unsafe_op_in_unsafe_fn)]

mod locomotion;
mod queries;
mod resolution;
mod tuning;

pub(crate) use locomotion::step_character_locomotion;
pub(crate) use queries::collect_character_queries;
pub(crate) use resolution::resolve_character_query_hits;
pub(crate) use tuning::sync_physics_world_settings;

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_ecs::World;
    use newengine_engine_runtime::gameplay::spawn_default_player;
    use newengine_gameplay_fps_api::FpsPlayerTuning;
    use newengine_math::Vec3;

    #[test]
    fn ground_probe_is_owned_by_fps_provider() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let vertical_extent = tuning.body_half_height + tuning.body_radius;
        let player = spawn_default_player(
            &mut world,
            None,
            "fps-ground-probe-player",
            Vec3::new(3.0, vertical_extent + tuning.contact_skin, -2.0),
        );
        let queries = collect_character_queries(&world);
        assert!(queries.iter().any(|query| query.seq == player.stable_u64()));
    }
}
