#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::{AudioListenerState, AudioOcclusionSettings};

    fn cube_collider() -> StaticMeshCollider {
        let vertices = vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        StaticMeshCollider::new(vertices, triangles).expect("cube collider")
    }

    fn world_with_blocker() -> (World, EntityId, EntityId) {
        let mut world = World::new();
        world.insert_resource(AudioListenerRuntimeState {
            listener: AudioListenerState {
                position: [-4.0, 0.0, 0.0],
                ..AudioListenerState::default()
            },
            listener_entity: None,
            frame_index: 1,
        });
        let blocker = world.spawn();
        let _ = world.insert(blocker, Transform::default());
        let _ = world.insert(blocker, cube_collider());
        let emitter = world.spawn();
        let _ = world.insert(
            emitter,
            Transform {
                position: Vec3::new(4.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        let mut audio = AudioEmitter::new("shared/audio/test.ysncd@edge");
        audio.occlusion = AudioOcclusionSettings::default();
        let _ = world.insert(emitter, audio);
        let _ = world.insert(
            emitter,
            AudioOcclusionObservation {
                fixed_tick: 1,
                samples: 3,
                blocked_samples: 3,
                obstruction: 1.0,
                occlusion: 1.0,
                estimated_thickness_m: 2.0,
                center_blocker_layers: 1,
                dominant_blocker_entity: Some(blocker.stable_u64()),
                dominant_material: "surface.default".to_owned(),
                material: AcousticMaterialProfile::transparent(),
            },
        );
        (world, emitter, blocker)
    }

    fn entity_keys(world: &World) -> BTreeMap<u64, EntityId> {
        world
            .iter_entities()
            .map(|entity| (entity.stable_u64(), entity))
            .collect()
    }

    #[test]
    fn provider_queries_only_edges_of_the_proven_occlusion_blocker() {
        let (mut world, emitter, blocker) = world_with_blocker();
        let unrelated = world.spawn();
        let _ = world.insert(
            unrelated,
            Transform {
                position: Vec3::new(3.0, 0.0, 0.0),
                ..Transform::default()
            },
        );
        let _ = world.insert(unrelated, cube_collider());
        let provider = AudioDiffractionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        assert!(!queries.is_empty());
        assert!(queries.len() <= MAX_EDGE_CANDIDATES_PER_EMITTER * 2);
        assert!(provider
            .pending
            .lock()
            .values()
            .all(|pending| pending.blocker_key == blocker.stable_u64()));
        let keys = entity_keys(&world);
        let consumed = provider.resolve_query_hits(&mut world, 2, &[], &keys);
        assert_eq!(consumed.len(), queries.len());
        let observation = world
            .get::<AudioEdgeDiffractionObservation>(emitter)
            .expect("diffraction observation");
        assert_eq!(observation.blocker_entity, Some(blocker.stable_u64()));
        assert!(observation.paths.iter().all(|path| path.visible));
    }

    #[test]
    fn edge_geometry_prefilter_is_bounded_and_prefers_direct_path() {
        let mut edges = Vec::new();
        for index in 0..128_u32 {
            let y = 50.0 + index as f32;
            edges.push(CachedWorldEdge {
                vertex_indices: [index * 2, index * 2 + 1],
                endpoints: [[0.0, y, -1.0], [0.0, y, 1.0]],
                wedge_angle_radians: 1.0,
            });
        }
        let direct_edge = CachedWorldEdge {
            vertex_indices: [999, 1_000],
            endpoints: [[0.0, 0.05, -1.0], [0.0, 0.05, 1.0]],
            wedge_angle_radians: 1.0,
        };
        edges.push(direct_edge);
        let selected = AudioDiffractionPhysicsQueryProvider::nearest_edges_to_direct_path(
            &edges,
            [-4.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
        );
        assert!(selected.len() <= MAX_EDGE_GEOMETRY_PREFILTER);
        assert_eq!(
            selected.first().unwrap().vertex_indices,
            direct_edge.vertex_indices
        );
    }

    #[test]
    fn diffraction_sampling_is_bounded_to_acoustic_cadence() {
        let (world, _, _) = world_with_blocker();
        let provider = AudioDiffractionPhysicsQueryProvider::new();
        assert!(
            !provider.collect_queries(&world).is_empty(),
            "first sample must run immediately"
        );
        for _ in 1..DIFFRACTION_QUERY_INTERVAL_TICKS {
            assert!(
                provider.collect_queries(&world).is_empty(),
                "intermediate fixed ticks must reuse the last diffraction observation"
            );
        }
        assert!(
            !provider.collect_queries(&world).is_empty(),
            "next acoustic interval must refresh diffraction queries"
        );
    }

    #[test]
    fn one_blocked_visibility_leg_closes_only_its_edge_candidate() {
        let (mut world, emitter, _) = world_with_blocker();
        let obstacle = world.spawn();
        let provider = AudioDiffractionPhysicsQueryProvider::new();
        let queries = provider.collect_queries(&world);
        let query = queries.first().expect("diffraction query");
        let max_t = match query.kind {
            PhysicsQueryKindDto::Ray { max_t, .. } => max_t,
            _ => panic!("ray expected"),
        };
        let hit = PhysicsQueryHitDto {
            subshape_id: 0,
            hit_index: 0,
            back_face: false,

            seq: query.seq,
            entity: obstacle.stable_u64(),
            position: [0.0; 3],
            normal: [0.0, 1.0, 0.0],
            distance: max_t * 0.5,
        };
        let keys = entity_keys(&world);
        provider.resolve_query_hits(&mut world, 3, &[hit], &keys);
        let observation = world
            .get::<AudioEdgeDiffractionObservation>(emitter)
            .expect("diffraction observation");
        assert_eq!(
            observation
                .paths
                .iter()
                .filter(|path| !path.visible)
                .count(),
            1
        );
        assert!(observation.paths.iter().any(|path| path.visible));
    }
}
