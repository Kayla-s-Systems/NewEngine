impl AudioEnvironmentFrame {
    fn resolve_at_internal(
        &self,
        emitter_key: Option<u64>,
        emitter_position: Vec3,
    ) -> AudioEnvironmentResolution {
        let emitter_membership = select_membership(&self.zones, emitter_position);
        let listener_membership = self.listener_membership.as_ref();

        let emitter_zone = emitter_membership
            .as_ref()
            .map(|membership| &self.zones[membership.zone_index]);
        let listener_zone =
            listener_membership.map(|membership| &self.zones[membership.zone_index]);

        let mut resolution = AudioEnvironmentResolution {
            emitter_zone: emitter_zone
                .map(|zone| zone.zone.zone_id.clone())
                .unwrap_or_default(),
            listener_zone: listener_zone
                .map(|zone| zone.zone.zone_id.clone())
                .unwrap_or_default(),
            ..AudioEnvironmentResolution::default()
        };

        match (
            emitter_membership.as_ref(),
            listener_membership,
            emitter_zone,
            listener_zone,
        ) {
            (
                Some(emitter_membership),
                Some(listener_membership),
                Some(emitter),
                Some(listener),
            ) if emitter.zone.zone_id == listener.zone.zone_id => {
                resolution.portal_gain = 1.0;
                resolution.transition_seconds = emitter
                    .zone
                    .transition_seconds
                    .max(listener.zone.transition_seconds);
                let reflection_observation =
                    emitter_key.and_then(|key| self.reflection_observations.get(&key));
                let listener_preset = geometry_adjusted_reverb(
                    listener,
                    emitter_position,
                    self.listener_position,
                    listener.zone.reverb,
                    reflection_observation,
                );
                let early_reflections = explicit_early_reflection_field(
                    listener.zone.reverb,
                    emitter_position,
                    self.listener_position,
                    reflection_observation,
                );
                let early_reflection_direction = early_reflections
                    .active()
                    .first()
                    .map(|tap| tap.direction)
                    .or_else(|| {
                        fresh_reflection_observation(
                            reflection_observation,
                            emitter_position,
                            self.listener_position,
                        )
                        .and_then(|observation| observation.paths.iter().find(|path| path.visible))
                        .map(|path| path.arrival_direction)
                    })
                    .unwrap_or([0.0; 3]);
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend::default(),
                    listener_send: AudioReverbSend {
                        room_bus_id: listener.stable_key,
                        gain: listener.zone.send_gain
                            * emitter_membership.influence
                            * listener_membership.influence,
                        preset: listener_preset,
                        early_reflections,
                        early_reflection_direction,
                    },
                    direct_path: AudioDirectPathResponse::clear(),
                    portal_gain: 1.0,
                }
                .sanitized();
            }
            (
                Some(emitter_membership),
                Some(listener_membership),
                Some(emitter),
                Some(listener),
            ) => {
                let route_gain = self
                    .indirect_routes
                    .get(&emitter.zone.zone_id)
                    .map(|route| route.gain)
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                resolution.portal_gain = route_gain;
                resolution.transition_seconds = emitter
                    .zone
                    .transition_seconds
                    .max(listener.zone.transition_seconds);
                let direct_route = self.direct_routes.get(&emitter.zone.zone_id);
                let direct_route_gain = direct_route.map(|route| route.gain).unwrap_or(0.0);
                let geometric_direct = direct_route.and_then(|route| {
                    direct_portal_route_response(
                        route,
                        &self.portals,
                        &self.portal_centers,
                        emitter_position,
                        self.listener_position,
                    )
                });
                let direct_path = geometric_direct.map(|(response, _, _)| response).unwrap_or(
                    AudioDirectPathResponse {
                        gain: direct_route_gain,
                        ..AudioDirectPathResponse::clear()
                    },
                );
                let indirect_boundaries = self
                    .indirect_routes
                    .get(&emitter.zone.zone_id)
                    .and_then(|route| portal_route_boundary_centers(route, &self.portal_centers));
                let (source_reverb_boundary, listener_reverb_boundary) =
                    indirect_boundaries.unwrap_or((None, None));
                let source_preset = source_reverb_boundary
                    .map(|center| {
                        geometry_adjusted_reverb(
                            emitter,
                            emitter_position,
                            center,
                            emitter.zone.reverb,
                            None,
                        )
                    })
                    .unwrap_or(emitter.zone.reverb);
                let listener_preset = listener_reverb_boundary
                    .map(|center| {
                        geometry_adjusted_reverb(
                            listener,
                            center,
                            self.listener_position,
                            listener.zone.reverb,
                            None,
                        )
                    })
                    .unwrap_or(listener.zone.reverb);
                let indirect_arrival_direction = listener_reverb_boundary
                    .map(|center| direction_array(self.listener_position, center))
                    .unwrap_or([0.0; 3]);
                resolution.state = AudioEnvironmentState {
                    source_send: AudioReverbSend {
                        room_bus_id: emitter.stable_key,
                        gain: emitter.zone.send_gain * emitter_membership.influence * route_gain,
                        preset: source_preset,
                        early_reflections: AudioEarlyReflectionField::empty(),
                        early_reflection_direction: indirect_arrival_direction,
                    },
                    listener_send: AudioReverbSend {
                        room_bus_id: listener.stable_key,
                        gain: listener.zone.send_gain * listener_membership.influence * route_gain,
                        preset: listener_preset,
                        early_reflections: AudioEarlyReflectionField::empty(),
                        early_reflection_direction: indirect_arrival_direction,
                    },
                    direct_path,
                    portal_gain: route_gain,
                }
                .sanitized();
            }
            _ => {
                // No common authored acoustic-zone route is not the same thing as an
                // acoustically blocked path. In worlds without AudioEnvironmentZone
                // authoring (or while listener/emitter membership is unresolved), the
                // conservative runtime fallback is a dry unity direct path. Explicitly
                // disconnected authored zones above still resolve to zero through the
                // portal graph.
                resolution.state = AudioEnvironmentState::clear();
                resolution.portal_gain = 1.0;
            }
        }

        resolution
    }
}
