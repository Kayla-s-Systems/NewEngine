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
        let query = queries
            .iter()
            .find(|query| query.seq == player.stable_u64())
            .expect("player ground query");
        let transform = world
            .get::<newengine_transform::Transform>(player)
            .expect("player transform");
        let sole_y = transform.position.y - vertical_extent;
        match query.kind {
            newengine_physics_api::PhysicsQueryKindDto::Ray { origin, dir, max_t } => {
                let epsilon = queries::ground_probe_origin_epsilon(tuning.contact_skin);
                assert!(
                    origin[1] > sole_y,
                    "ground probe must start above the capsule sole: origin={} sole={}",
                    origin[1],
                    sole_y
                );
                assert!((origin[1] - sole_y - epsilon).abs() < 1.0e-6);
                assert_eq!(dir, [0.0, -1.0, 0.0]);
                assert!(
                    max_t >= tuning.contact_skin + tuning.ground_probe_distance + epsilon - 1.0e-6,
                    "moving the origin above the sole must not shorten authored probe reach"
                );
            }
            other => panic!("expected ground ray, got {other:?}"),
        }
    }
}
