impl GameplayPhysicsQueryProvider for AudioDiffractionPhysicsQueryProvider {
    fn id(&self) -> &'static str {
        "engine.audio.physics-diffraction"
    }

    fn collect_queries(&self, world: &World) -> Vec<PhysicsQueryDto> {
        let Some(listener_state) = world.resource::<AudioListenerRuntimeState>().copied() else {
            self.pending.lock().clear();
            self.update_tracking(BTreeSet::new());
            return Vec::new();
        };
        let listener_position = listener_state.listener.sanitized().position;
        let listener = Vec3::new(
            listener_position[0],
            listener_position[1],
            listener_position[2],
        );
        if !listener.is_finite() {
            self.pending.lock().clear();
            self.update_tracking(BTreeSet::new());
            return Vec::new();
        }
        if !self.sample_due() {
            // A previous diffraction observation remains valid until the next acoustic sample.
            // Never carry unresolved ray bookkeeping across skipped physics ticks.
            self.pending.lock().clear();
            return Vec::new();
        }

        let mesh_entities = Self::static_mesh_entities(world);
        self.edge_cache
            .lock()
            .retain(|key, _| mesh_entities.contains_key(key));
        let candidates = self.collect_candidates(world, listener, &mesh_entities);
        self.update_tracking(
            candidates
                .iter()
                .map(|candidate| candidate.emitter_key)
                .collect(),
        );
        let listener_key = listener_state.listener_entity;
        let mut pending = BTreeMap::new();
        let mut queries = Vec::new();
        for candidate in candidates {
            let edges = self.blocker_edges(world, candidate.blocker_entity, candidate.blocker_key);
            let source = [
                candidate.position.x,
                candidate.position.y,
                candidate.position.z,
            ];
            let receiver = [listener.x, listener.y, listener.z];
            let prefiltered_edges = Self::nearest_edges_to_direct_path(&edges, source, receiver);
            let mut paths = Vec::with_capacity(MAX_EDGE_CANDIDATES_PER_EMITTER);
            for edge in prefiltered_edges {
                let Some(geometry) = edge_diffraction_geometry(edge.endpoints, source, receiver)
                else {
                    continue;
                };
                if geometry.excess_length_m <= 1.0e-4 || geometry.bend_angle_radians <= 1.0e-3 {
                    continue;
                }
                let insert_at = paths
                    .binary_search_by(|(existing_edge, existing_geometry): &(CachedWorldEdge, AudioEdgeDiffractionGeometry)| {
                        existing_geometry
                            .path_length_m
                            .total_cmp(&geometry.path_length_m)
                            .then_with(|| existing_edge.vertex_indices.cmp(&edge.vertex_indices))
                    })
                    .unwrap_or_else(|index| index);
                if insert_at >= MAX_EDGE_CANDIDATES_PER_EMITTER {
                    continue;
                }
                paths.insert(insert_at, (edge, geometry));
                if paths.len() > MAX_EDGE_CANDIDATES_PER_EMITTER {
                    paths.pop();
                }
            }
            let (material_known, material) =
                Self::blocker_material(world, candidate.blocker_entity);
            for (edge, geometry) in paths {
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    candidate,
                    listener,
                    listener_key,
                    edge,
                    geometry,
                    DiffractionProbeLeg::Source,
                    material_known,
                    material,
                );
                self.push_leg(
                    &mut pending,
                    &mut queries,
                    candidate,
                    listener,
                    listener_key,
                    edge,
                    geometry,
                    DiffractionProbeLeg::Listener,
                    material_known,
                    material,
                );
            }
        }
        *self.pending.lock() = pending;
        queries
    }

    fn resolve_query_hits(
        &self,
        world: &mut World,
        fixed_tick: u64,
        hits: &[PhysicsQueryHitDto],
        key_to_entity: &BTreeMap<u64, EntityId>,
    ) -> BTreeSet<u64> {
        let pending = std::mem::take(&mut *self.pending.lock());
        let clear_emitters = std::mem::take(&mut *self.clear_emitters.lock());
        for key in clear_emitters {
            if let Some(entity) = key_to_entity.get(&key).copied() {
                let _ = world.remove::<AudioEdgeDiffractionObservation>(entity);
            }
        }
        if pending.is_empty() {
            return BTreeSet::new();
        }
        let hits_by_seq = hits
            .iter()
            .map(|hit| (hit.seq, *hit))
            .collect::<BTreeMap<_, _>>();
        let mut consumed = BTreeSet::new();
        let mut aggregates = BTreeMap::<(u64, u64, [u32; 2]), DiffractionAggregate>::new();
        for (seq, ray) in pending {
            consumed.insert(seq);
            let blocked = hits_by_seq.get(&seq).is_some_and(|hit| {
                hit.entity != ray.emitter_key
                    && ray.listener_key != Some(hit.entity)
                    && hit.distance.is_finite()
                    && hit.distance >= 0.0
                    && hit.distance <= ray.max_t + 1.0e-4
            });
            let aggregate = aggregates
                .entry((ray.emitter_key, ray.blocker_key, ray.edge.vertex_indices))
                .or_insert_with(|| DiffractionAggregate::new(ray));
            match ray.leg {
                DiffractionProbeLeg::Source => aggregate.source_blocked = blocked,
                DiffractionProbeLeg::Listener => aggregate.listener_blocked = blocked,
            }
        }

        let mut by_emitter = BTreeMap::<
            u64,
            (
                u64,
                [f32; 3],
                [f32; 3],
                Vec<AudioEdgeDiffractionPathObservation>,
            ),
        >::new();
        for ((emitter_key, blocker_key, _), aggregate) in aggregates {
            let entry = by_emitter.entry(emitter_key).or_insert_with(|| {
                (
                    blocker_key,
                    aggregate.source_position,
                    aggregate.listener_position,
                    Vec::new(),
                )
            });
            entry.3.push(AudioEdgeDiffractionPathObservation {
                edge_vertex_indices: aggregate.edge.vertex_indices,
                visible: !aggregate.source_blocked && !aggregate.listener_blocked,
                diffraction_point: aggregate.geometry.diffraction_point,
                arrival_direction: aggregate.geometry.arrival_direction,
                path_length_m: aggregate.geometry.path_length_m,
                excess_length_m: aggregate.geometry.excess_length_m,
                bend_angle_radians: aggregate.geometry.bend_angle_radians,
                wedge_angle_radians: aggregate.edge.wedge_angle_radians,
                material_known: aggregate.material_known,
                material: aggregate.material,
            });
        }

        for (emitter_key, (blocker_key, source_position, listener_position, mut paths)) in
            by_emitter
        {
            let Some(entity) = key_to_entity.get(&emitter_key).copied() else {
                continue;
            };
            paths.sort_by(|a, b| {
                a.path_length_m
                    .total_cmp(&b.path_length_m)
                    .then_with(|| a.edge_vertex_indices.cmp(&b.edge_vertex_indices))
            });
            let _ = world.insert(
                entity,
                AudioEdgeDiffractionObservation {
                    fixed_tick,
                    source_position,
                    listener_position,
                    blocker_entity: Some(blocker_key),
                    paths,
                },
            );
        }
        consumed
    }
}
