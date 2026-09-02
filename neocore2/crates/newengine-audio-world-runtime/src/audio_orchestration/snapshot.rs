impl AudioOrchestrationRuntimeModule {
    fn transport_state(&self) -> AudioTransportRuntimeState {
        let mut transport = self.transport.snapshot();
        transport.active_transitions = self.scalar_transitions.len()
            + self.instance_gain_transitions.len()
            + self
                .snapshots
                .values()
                .filter(|snapshot| snapshot.sample_transition.is_some())
                .count();
        transport
    }

    fn snapshot_state(&self) -> AudioOrchestrationRuntimeState {
        let transport = self.transport_state();
        let transport_sample = self.transport.sample();
        AudioOrchestrationRuntimeState {
            objects: self.objects.len(),
            instances: self.instances.len(),
            provider_voices: self
                .instances
                .values()
                .map(|instance| instance.voice_ids.len())
                .sum(),
            logical_routes: self.mix_graph.as_ref().map_or(0, |graph| graph.buses.len()),
            active_snapshots: self.snapshot_weights(),
            dropped_commands: self.handle.dropped_commands(),
            dropped_transport_events: self.handle.dropped_transport_events(),
            transport,
            transport_instances: self
                .instances
                .iter()
                .map(|(instance_id, instance)| {
                    (
                        *instance_id,
                        AudioTransportInstanceState {
                            start_sample: instance.transport_start_sample,
                            dispatch_sample: instance.transport_dispatch_sample,
                            logical_sample: transport_sample
                                .saturating_sub(instance.transport_start_sample),
                            dispatch_lateness_samples: instance
                                .transport_dispatch_sample
                                .saturating_sub(instance.transport_start_sample),
                        },
                    )
                })
                .collect(),
            music: self.music_state(),
        }
    }

    fn publish_runtime_state(&self, ctx: &mut ModuleCtx<'_, ()>) {
        // Build the composite state exactly once. The previous path independently rebuilt
        // transport/music snapshots and then rebuilt both again inside snapshot_state(), causing
        // duplicate map walks and allocations every frame.
        let state = self.snapshot_state();
        ctx.resources_mut().insert(state.transport.clone());
        ctx.resources_mut().insert(state.music.clone());
        ctx.resources_mut().insert(state);
    }

    fn stop_all(&mut self) {
        for (_, instance) in self.instances.drain() {
            Self::stop_voice_ids(&instance.voice_ids);
        }
        self.objects.clear();
        self.snapshots.clear();
        self.scalar_transitions.clear();
        self.instance_gain_transitions.clear();
        self.provider_gain_ramp_until.clear();
        self.prearmed_transport_actions.clear();
        self.provider_clock = None;
        self.provider_clock_anchor = None;
        self.music_sessions.clear();
        self.music_graphs.clear();
    }
}
