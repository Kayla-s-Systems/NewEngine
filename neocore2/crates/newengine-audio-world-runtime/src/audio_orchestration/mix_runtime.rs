impl AudioOrchestrationRuntimeModule {
    fn stop_voice_ids(voice_ids: &[u64]) {
        for voice_id in voice_ids {
            let _ = stop_audio_voice(*voice_id);
        }
    }

    fn stop_instance(&mut self, instance_id: AudioInstanceId) {
        self.clear_scalar_transitions_for_target(&AudioParameterTarget::Instance(instance_id));
        self.instance_gain_transitions.remove(&instance_id);
        if let Some(instance) = self.instances.remove(&instance_id) {
            Self::stop_voice_ids(&instance.voice_ids);
        }
    }

    fn clear_scalar_transitions_for_target(&mut self, target: &AudioParameterTarget) {
        self.scalar_transitions
            .retain(|(candidate, _), _| candidate != target);
    }

    fn stop_object_instances(&mut self, object_id: AudioObjectId) {
        let targets = self
            .instances
            .iter()
            .filter(|(_, instance)| instance.object_id == object_id)
            .map(|(instance_id, _)| *instance_id)
            .collect::<Vec<_>>();
        for instance_id in targets {
            self.stop_instance(instance_id);
        }
    }

    fn scalar_value(&self, target: &AudioParameterTarget, name: &str) -> Option<f32> {
        match target {
            AudioParameterTarget::Global => self.global_parameters.scalars.get(name).copied(),
            AudioParameterTarget::Object(object_id) => self
                .objects
                .get(object_id)
                .and_then(|object| object.state.parameters.scalars.get(name).copied()),
            AudioParameterTarget::Instance(instance_id) => self
                .instances
                .get(instance_id)
                .and_then(|instance| instance.parameters.scalars.get(name).copied()),
        }
    }

    fn write_scalar(
        &mut self,
        target: &AudioParameterTarget,
        name: &str,
        value: f32,
    ) -> Result<(), String> {
        match target {
            AudioParameterTarget::Global => {
                self.global_parameters.set_scalar(name.to_owned(), value)
            }
            AudioParameterTarget::Object(object_id) => self
                .objects
                .get_mut(object_id)
                .ok_or_else(|| format!("unknown audio object {}", object_id.0))
                .and_then(|object| object.state.parameters.set_scalar(name.to_owned(), value)),
            AudioParameterTarget::Instance(instance_id) => self
                .instances
                .get_mut(instance_id)
                .ok_or_else(|| format!("unknown audio instance {}", instance_id.0))
                .and_then(|instance| instance.parameters.set_scalar(name.to_owned(), value)),
        }
    }

    fn set_scalar(&mut self, target: AudioParameterTarget, name: String, value: f32) {
        self.scalar_transitions
            .remove(&(target.clone(), name.clone()));
        if let Err(error) = self.write_scalar(&target, &name, value) {
            newengine_ulog_api::ulog::warn!("audio orchestration: scalar rejected err='{}'", error);
        }
    }

    fn transition_scalar_samples(
        &mut self,
        target: AudioParameterTarget,
        name: String,
        target_value: f32,
        start_sample: u64,
        duration_samples: u64,
    ) {
        let Some(from) = self.scalar_value(&target, &name) else {
            newengine_ulog_api::ulog::warn!(
                "audio transport: scalar transition rejected missing parameter name='{}' target={:?}",
                name,
                target
            );
            return;
        };
        if duration_samples == 0 {
            self.scalar_transitions
                .remove(&(target.clone(), name.clone()));
            if let Err(error) = self.write_scalar(&target, &name, target_value) {
                newengine_ulog_api::ulog::warn!(
                    "audio transport: scalar transition apply failed err='{}'",
                    error
                );
            }
            return;
        }
        self.scalar_transitions.insert(
            (target, name),
            SampleTransition::new(from, target_value, start_sample, duration_samples),
        );
    }

    fn advance_scalar_transitions_to_sample(&mut self, sample: u64) {
        let mut updates = Vec::<((AudioParameterTarget, String), f32, bool)>::new();
        for (key, transition) in &self.scalar_transitions {
            let (value, finished) = transition.evaluate(sample);
            updates.push((key.clone(), value, finished));
        }
        for ((target, name), value, finished) in updates {
            if let Err(error) = self.write_scalar(&target, &name, value) {
                newengine_ulog_api::ulog::trace!(
                    "audio transport: scalar transition retired target={:?} name='{}' err='{}'",
                    target,
                    name,
                    error
                );
                self.scalar_transitions.remove(&(target, name));
                continue;
            }
            if finished {
                self.scalar_transitions.remove(&(target, name));
            }
        }
    }

    fn transition_instance_gain_samples(
        &mut self,
        instance_id: AudioInstanceId,
        target_gain: f32,
        start_sample: u64,
        duration_samples: u64,
    ) {
        let Some(instance) = self.instances.get(&instance_id) else {
            newengine_ulog_api::ulog::trace!(
                "audio transport: instance gain transition ignored unknown instance_id={}",
                instance_id.0
            );
            return;
        };
        let from = instance.gain;
        let target = newengine_audio_api::sanitize_gain(target_gain);
        if duration_samples == 0 {
            if let Some(instance) = self.instances.get_mut(&instance_id) {
                instance.gain = target;
            }
            self.instance_gain_transitions.remove(&instance_id);
            return;
        }
        self.instance_gain_transitions.insert(
            instance_id,
            SampleTransition::new(from, target, start_sample, duration_samples),
        );
    }

    fn advance_instance_gain_transitions_to_sample(&mut self, sample: u64) {
        let mut updates = Vec::<(AudioInstanceId, f32, bool)>::new();
        for (instance_id, transition) in &self.instance_gain_transitions {
            let (value, finished) = transition.evaluate(sample);
            updates.push((*instance_id, value, finished));
        }
        for (instance_id, value, finished) in updates {
            let Some(instance) = self.instances.get_mut(&instance_id) else {
                self.instance_gain_transitions.remove(&instance_id);
                continue;
            };
            instance.gain = newengine_audio_api::sanitize_gain(value);
            if finished {
                self.instance_gain_transitions.remove(&instance_id);
            }
        }
    }

    fn set_switch(&mut self, target: AudioParameterTarget, name: String, value: String) {
        let result = match target {
            AudioParameterTarget::Global => self.global_parameters.set_switch(name, value),
            AudioParameterTarget::Object(object_id) => self
                .objects
                .get_mut(&object_id)
                .ok_or_else(|| format!("unknown audio object {}", object_id.0))
                .and_then(|object| object.state.parameters.set_switch(name, value)),
            AudioParameterTarget::Instance(instance_id) => self
                .instances
                .get_mut(&instance_id)
                .ok_or_else(|| format!("unknown audio instance {}", instance_id.0))
                .and_then(|instance| instance.parameters.set_switch(name, value)),
        };
        if let Err(error) = result {
            newengine_ulog_api::ulog::warn!("audio orchestration: switch rejected err='{}'", error);
        }
    }

    fn snapshot_transition(&self, snapshot: &str, override_seconds: Option<f32>) -> Option<f32> {
        let spec = self.mix_graph.as_ref()?.snapshot(snapshot)?;
        Some(sanitize_transition(
            override_seconds.unwrap_or(spec.transition_seconds),
        ))
    }

    fn activate_snapshot(&mut self, snapshot: &str, weight: f32, transition_seconds: Option<f32>) {
        let Some(transition) = self.snapshot_transition(snapshot, transition_seconds) else {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: activate ignored unknown snapshot='{}'",
                snapshot
            );
            return;
        };
        let target = sanitize_weight(weight);
        self.snapshots
            .entry(snapshot.to_owned())
            .and_modify(|active| active.retarget(target, transition))
            .or_insert_with(|| ActiveSnapshot::new(target, transition));
    }

    fn deactivate_snapshot(&mut self, snapshot: &str, transition_seconds: Option<f32>) {
        let Some(transition) = self.snapshot_transition(snapshot, transition_seconds) else {
            return;
        };
        if let Some(active) = self.snapshots.get_mut(snapshot) {
            active.retarget(0.0, transition);
        }
    }

    fn transition_snapshot_samples(
        &mut self,
        snapshot: &str,
        target_weight: f32,
        start_sample: u64,
        duration_samples: u64,
    ) {
        if self
            .mix_graph
            .as_ref()
            .and_then(|graph| graph.snapshot(snapshot))
            .is_none()
        {
            newengine_ulog_api::ulog::warn!(
                "audio transport: snapshot transition ignored unknown snapshot='{}'",
                snapshot
            );
            return;
        }
        let target = sanitize_weight(target_weight);
        self.snapshots
            .entry(snapshot.to_owned())
            .and_modify(|active| {
                active.advance_to_sample(start_sample);
                active.retarget_samples(target, start_sample, duration_samples);
            })
            .or_insert_with(|| {
                let mut active = ActiveSnapshot::new(0.0, 0.0);
                active.retarget_samples(target, start_sample, duration_samples);
                active
            });
    }

    fn advance_snapshots_to_sample(&mut self, sample: u64) {
        for snapshot in self.snapshots.values_mut() {
            snapshot.advance_to_sample(sample);
        }
    }

    fn advance_snapshots(&mut self, dt: f32) {
        for snapshot in self.snapshots.values_mut() {
            snapshot.advance_seconds(dt);
        }
        self.snapshots.retain(|_, snapshot| {
            snapshot.current > 1.0e-5
                || snapshot.target > 1.0e-5
                || snapshot.remaining_seconds > 0.0
                || snapshot.sample_transition.is_some()
        });
    }

    fn snapshot_weights(&self) -> BTreeMap<String, f32> {
        self.snapshots
            .iter()
            .filter(|(_, snapshot)| snapshot.current > 1.0e-5)
            .map(|(id, snapshot)| (id.clone(), snapshot.current))
            .collect()
    }

    fn route_gain(&self, route: &AudioRouteId) -> f32 {
        if route.0.is_empty() {
            return 1.0;
        }
        self.mix_graph
            .as_ref()
            .and_then(|graph| {
                graph
                    .effective_linear_gain(route, &self.snapshot_weights())
                    .ok()
            })
            .unwrap_or(1.0)
            .clamp(0.0, 16.0)
    }

    fn sync_provider_route_gains(&self) {
        let Some(graph) = self.mix_graph.as_ref() else {
            return;
        };
        let weights = self.snapshot_weights();
        for route in graph.buses.iter().map(|bus| bus.id.clone()) {
            let gain = graph
                .effective_linear_gain(&route, &weights)
                .unwrap_or(1.0)
                .clamp(0.0, 16.0);
            match set_audio_route_gain(&AudioRouteGainRequest {
                route: route.clone(),
                gain,
            }) {
                Ok(Some(ack)) if !ack.accepted => newengine_ulog_api::ulog::warn!(
                    "audio orchestration: provider rejected route gain route='{}' gain={:.6}",
                    route.0,
                    gain
                ),
                Ok(_) => {}
                Err(error) => newengine_ulog_api::ulog::trace!(
                    "audio orchestration: route gain publish deferred route='{}' err='{}'",
                    route.0,
                    error
                ),
            }
        }
    }

    fn sync_instances(&mut self) {
        let transport_sample = self.transport.sample();
        self.provider_gain_ramp_until
            .retain(|_, end_sample| *end_sample > transport_sample);
        self.sync_provider_route_gains();

        let mut finished = Vec::<AudioInstanceId>::new();
        for (instance_id, instance) in &mut self.instances {
            if instance.render_armed && transport_sample < instance.transport_start_sample {
                continue;
            }
            let Some(object) = self.objects.get(&instance.object_id) else {
                finished.push(*instance_id);
                continue;
            };
            let provider_ramp_active = self
                .provider_gain_ramp_until
                .get(instance_id)
                .is_some_and(|end_sample| transport_sample < *end_sample);
            let request = AudioVoiceUpdateRequest {
                voice_id: 0,
                gain: (!provider_ramp_active).then_some(object.state.gain * instance.gain),
                speed: None,
                seek_seconds: None,
                paused: Some(false),
                position: instance.spatial.then_some(object.state.position),
                acoustic: Some(object.state.acoustic),
                environment: Some(object.state.environment),
            };
            let mut live = Vec::<u64>::with_capacity(instance.voice_ids.len());
            for voice_id in instance.voice_ids.iter().copied() {
                let mut request = request;
                request.voice_id = voice_id;
                match update_audio_voice(&request) {
                    Ok(Some(ack)) if ack.accepted => live.push(voice_id),
                    Ok(Some(ack)) if ack.message != "voice not found" => {
                        newengine_ulog_api::ulog::trace!(
                            "audio orchestration: voice update rejected instance_id={} voice_id={} message='{}'",
                            instance_id.0,
                            voice_id,
                            ack.message
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        newengine_ulog_api::ulog::trace!(
                            "audio orchestration: voice update failed instance_id={} voice_id={} err='{}'",
                            instance_id.0,
                            voice_id,
                            error
                        );
                        // Service errors are not proof that the logical voice disappeared.
                        live.push(voice_id);
                    }
                }
            }
            instance.voice_ids = live;
            if instance.voice_ids.is_empty() {
                finished.push(*instance_id);
            }
        }
        for instance_id in finished {
            self.stop_instance(instance_id);
        }
    }
}
