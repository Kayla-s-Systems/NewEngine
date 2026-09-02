/// Bounded edge-diffraction query contributor. Candidate edges come only from the canonical static
/// mesh collider that the previous direct-path occlusion observation identified as the blocker.
pub struct AudioDiffractionPhysicsQueryProvider {
    edge_cache: Mutex<BTreeMap<u64, CachedBlockerEdges>>,
    pending: Mutex<BTreeMap<u64, PendingDiffractionRay>>,
    tracked_emitters: Mutex<BTreeSet<u64>>,
    clear_emitters: Mutex<BTreeSet<u64>>,
    next_query: AtomicU64,
    sample_tick: AtomicU64,
}

impl Default for AudioDiffractionPhysicsQueryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDiffractionPhysicsQueryProvider {
    pub fn new() -> Self {
        Self {
            edge_cache: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(BTreeMap::new()),
            tracked_emitters: Mutex::new(BTreeSet::new()),
            clear_emitters: Mutex::new(BTreeSet::new()),
            next_query: AtomicU64::new(1),
            sample_tick: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    #[inline]
    fn alloc_query_id(&self) -> u64 {
        let value =
            self.next_query.fetch_add(1, Ordering::Relaxed) & AUDIO_DIFFRACTION_QUERY_COUNTER_MASK;
        AUDIO_DIFFRACTION_QUERY_NAMESPACE | value.max(1)
    }

    #[inline]
    fn sample_due(&self) -> bool {
        self.sample_tick.fetch_add(1, Ordering::Relaxed) % DIFFRACTION_QUERY_INTERVAL_TICKS == 0
    }

    fn static_mesh_entities(world: &World) -> BTreeMap<u64, EntityId> {
        world
            .query::<StaticMeshCollider>()
            .map(|(entity, _)| (entity.stable_u64(), entity))
            .collect()
    }

    fn collect_candidates(
        &self,
        world: &World,
        listener: Vec3,
        mesh_entities: &BTreeMap<u64, EntityId>,
    ) -> Vec<DiffractionEmitterCandidate> {
        // Keep only the nearest bounded working set while scanning emitters. Diffraction consumes
        // at most MAX_DIFFRACTION_EMITTERS_PER_TICK, so sorting an unbounded temporary Vec makes
        // scene density leak directly into the acoustic fixed-tick cost.
        let mut candidates = Vec::with_capacity(MAX_DIFFRACTION_EMITTERS_PER_TICK);
        for (entity, emitter) in world.query::<AudioEmitter>() {
            let Some(observation) = world.get::<AudioOcclusionObservation>(entity) else {
                continue;
            };
            if !emitter.enabled
                || !emitter.spatial
                || emitter.cue.trim().is_empty()
                || observation.occlusion <= 1.0e-4
            {
                continue;
            }
            let Some(blocker_key) = observation.dominant_blocker_entity else {
                continue;
            };
            let Some(blocker_entity) = mesh_entities.get(&blocker_key).copied() else {
                continue;
            };
            let position = read_entity_world_pose_local_chain(world, entity)
                .map(|pose| pose.0)
                .or_else(|| {
                    world
                        .get::<Transform>(entity)
                        .map(|transform| transform.position)
                })
                .unwrap_or(Vec3::ZERO);
            if !position.is_finite() {
                continue;
            }
            let distance = (position - listener).length();
            if !distance.is_finite() || distance <= 1.0e-4 {
                continue;
            }
            let candidate = DiffractionEmitterCandidate {
                emitter_key: entity.stable_u64(),
                position,
                distance,
                blocker_key,
                blocker_entity,
            };
            let insert_at = candidates
                .binary_search_by(|existing: &DiffractionEmitterCandidate| {
                    existing
                        .distance
                        .total_cmp(&candidate.distance)
                        .then_with(|| existing.emitter_key.cmp(&candidate.emitter_key))
                })
                .unwrap_or_else(|index| index);
            if insert_at >= MAX_DIFFRACTION_EMITTERS_PER_TICK {
                continue;
            }
            candidates.insert(insert_at, candidate);
            if candidates.len() > MAX_DIFFRACTION_EMITTERS_PER_TICK {
                candidates.pop();
            }
        }
        candidates
    }

    fn blocker_edges(
        &self,
        world: &World,
        blocker_entity: EntityId,
        blocker_key: u64,
    ) -> Arc<[CachedWorldEdge]> {
        let Some(collider) = world.get::<StaticMeshCollider>(blocker_entity) else {
            return Arc::from([]);
        };
        let transform = world
            .get::<Transform>(blocker_entity)
            .copied()
            .unwrap_or_default();
        let revision = collider.runtime_revision(transform);
        if let Some(cached) = self.edge_cache.lock().get(&blocker_key) {
            if cached.revision == revision {
                return Arc::clone(&cached.edges);
            }
        }

        let local = mesh_diffraction_edges(collider.vertices.as_ref(), collider.triangles.as_ref());
        let edges = local
            .into_iter()
            .map(|edge| CachedWorldEdge {
                vertex_indices: edge.vertex_indices,
                endpoints: edge.endpoints.map(|point| {
                    let local = Vec3::new(point[0], point[1], point[2]);
                    let world = transform.rotation * local + transform.position;
                    [world.x, world.y, world.z]
                }),
                wedge_angle_radians: edge.wedge_angle_radians,
            })
            .collect::<Vec<_>>();
        let edges: Arc<[CachedWorldEdge]> = Arc::from(edges.into_boxed_slice());
        self.edge_cache.lock().insert(
            blocker_key,
            CachedBlockerEdges {
                revision,
                edges: Arc::clone(&edges),
            },
        );
        edges
    }

    fn nearest_edges_to_direct_path(
        edges: &[CachedWorldEdge],
        source: [f32; 3],
        receiver: [f32; 3],
    ) -> Vec<CachedWorldEdge> {
        let mut nearest = Vec::<(f32, CachedWorldEdge)>::with_capacity(
            MAX_EDGE_GEOMETRY_PREFILTER.min(edges.len()),
        );
        for &edge in edges {
            let metric = edge_direct_path_distance_sq(edge, source, receiver);
            if !metric.is_finite() {
                continue;
            }
            let insert_at = nearest
                .binary_search_by(|(distance, existing)| {
                    distance
                        .total_cmp(&metric)
                        .then_with(|| existing.vertex_indices.cmp(&edge.vertex_indices))
                })
                .unwrap_or_else(|index| index);
            if insert_at >= MAX_EDGE_GEOMETRY_PREFILTER {
                continue;
            }
            nearest.insert(insert_at, (metric, edge));
            if nearest.len() > MAX_EDGE_GEOMETRY_PREFILTER {
                nearest.pop();
            }
        }
        nearest.into_iter().map(|(_, edge)| edge).collect()
    }

    fn blocker_material(
        world: &World,
        blocker_entity: EntityId,
    ) -> (bool, AcousticMaterialProfile) {
        if let Some(surface) = world.get::<AcousticSurface>(blocker_entity) {
            return (true, surface.clone().sanitized().profile);
        }
        if let Some(surface) = world.get::<PhysicsSurface>(blocker_entity) {
            if let Some(profile) = world
                .resource::<AcousticMaterialLibrary>()
                .and_then(|library| library.resolve(surface.id.as_str()))
                .map(|surface| surface.profile)
            {
                return (true, profile);
            }
        }
        (
            false,
            resolve_acoustic_surface_for_entity(world, Some(blocker_entity)).profile,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn push_leg(
        &self,
        pending: &mut BTreeMap<u64, PendingDiffractionRay>,
        queries: &mut Vec<PhysicsQueryDto>,
        candidate: DiffractionEmitterCandidate,
        listener: Vec3,
        listener_key: Option<u64>,
        edge: CachedWorldEdge,
        geometry: AudioEdgeDiffractionGeometry,
        leg: DiffractionProbeLeg,
        material_known: bool,
        material: AcousticMaterialProfile,
    ) {
        let point = Vec3::new(
            geometry.diffraction_point[0],
            geometry.diffraction_point[1],
            geometry.diffraction_point[2],
        );
        let origin = match leg {
            DiffractionProbeLeg::Source => candidate.position,
            DiffractionProbeLeg::Listener => listener,
        };
        let delta = point - origin;
        let full_length = delta.length();
        if !full_length.is_finite() || full_length <= EDGE_VISIBILITY_ENDPOINT_EPSILON * 2.0 {
            return;
        }
        let dir = delta / full_length;
        let max_t = full_length - EDGE_VISIBILITY_ENDPOINT_EPSILON;
        let seq = self.alloc_query_id();
        pending.insert(
            seq,
            PendingDiffractionRay {
                emitter_key: candidate.emitter_key,
                listener_key,
                blocker_key: candidate.blocker_key,
                leg,
                edge,
                geometry,
                max_t,
                source_position: [
                    candidate.position.x,
                    candidate.position.y,
                    candidate.position.z,
                ],
                listener_position: [listener.x, listener.y, listener.z],
                material_known,
                material,
            },
        );
        queries.push(PhysicsQueryDto {
            seq,
            ignore_entity: match leg {
                DiffractionProbeLeg::Source => Some(candidate.emitter_key),
                DiffractionProbeLeg::Listener => listener_key,
            },
            kind: PhysicsQueryKindDto::Ray {
                origin: [origin.x, origin.y, origin.z],
                dir: [dir.x, dir.y, dir.z],
                max_t,
            },
        });
    }

    fn update_tracking(&self, current: BTreeSet<u64>) {
        let mut tracked = self.tracked_emitters.lock();
        let removed = tracked
            .difference(&current)
            .copied()
            .collect::<BTreeSet<_>>();
        *self.clear_emitters.lock() = removed;
        *tracked = current;
    }
}
