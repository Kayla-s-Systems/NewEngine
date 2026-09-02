use super::*;

struct MusicStatePlanRequest<'a> {
    graph: &'a InteractiveMusicGraph,
    object_id: AudioObjectId,
    current_stems: &'a BTreeMap<String, AudioInstanceId>,
    parameters: &'a AudioParameterSet,
    target_state: &'a str,
    start_sample: u64,
    crossfade_samples: u64,
}

impl AudioOrchestrationRuntimeModule {
    pub(super) fn allocate_music_instance_id(&self) -> AudioInstanceId {
        AudioInstanceId(
            self.handle
                .next_instance_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        )
    }

    pub(super) fn allocate_music_action_id(&self) -> AudioTransportActionId {
        AudioTransportActionId(
            self.handle
                .next_transport_action_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        )
    }

    pub(super) fn schedule_music_action(
        &mut self,
        action_ids: &mut Vec<AudioTransportActionId>,
        sample: u64,
        action: AudioTransportAction,
    ) -> Result<AudioTransportActionId, String> {
        let id = self.allocate_music_action_id();
        self.transport.schedule(
            id,
            AudioTransportSchedulePoint::AbsoluteSample { sample },
            action,
        )?;
        action_ids.push(id);
        Ok(id)
    }

    pub(super) fn rollback_music_actions(&mut self, action_ids: &[AudioTransportActionId]) {
        for id in action_ids {
            if self.transport.cancel(*id) {
                self.cancel_prearmed_transport_action(*id);
            }
        }
    }

    pub(super) fn install_music_graph(&mut self, graph: InteractiveMusicGraph) {
        if let Err(error) = graph.validate() {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::warn!(
                "interactive music: graph rejected id='{}' err='{}'",
                graph.id,
                error
            );
            return;
        }
        let key = graph.id.to_ascii_lowercase();
        if self
            .music_sessions
            .values()
            .any(|session| session.graph == key)
        {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::warn!(
                "interactive music: graph replacement rejected while sessions are active id='{}'",
                graph.id
            );
            return;
        }
        self.music_graphs.insert(key, graph);
    }

    pub(super) fn create_music_session(
        &mut self,
        session_id: AudioMusicSessionId,
        graph_id: String,
        object_id: AudioObjectId,
    ) {
        if session_id.0 == 0 || object_id.0 == 0 {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        }
        if !self.objects.contains_key(&object_id) {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::warn!(
                "interactive music: session create rejected unknown object_id={} session_id={}",
                object_id.0,
                session_id.0
            );
            return;
        }
        if self.music_sessions.contains_key(&session_id) {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        }
        let key = graph_id.trim().to_ascii_lowercase();
        let Some(graph) = self.music_graphs.get(&key).cloned() else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::warn!(
                "interactive music: session create rejected unknown graph='{}'",
                graph_id
            );
            return;
        };
        let Some(initial) = graph.state(&graph.initial_state).cloned() else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        };
        let sample = self.transport.sample();
        let parameters = AudioParameterSet::default();
        match self.plan_music_state(MusicStatePlanRequest {
            graph: &graph,
            object_id,
            current_stems: &BTreeMap::new(),
            parameters: &parameters,
            target_state: &initial.id,
            start_sample: sample,
            crossfade_samples: 0,
        }) {
            Ok(pending) => {
                self.music_sessions.insert(
                    session_id,
                    RuntimeMusicSession {
                        graph: key,
                        object_id,
                        active_state: String::new(),
                        stems: BTreeMap::new(),
                        parameters,
                        pending: Some(pending),
                    },
                );
                self.music_transitions_scheduled =
                    self.music_transitions_scheduled.saturating_add(1);
            }
            Err(error) => {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::warn!(
                    "interactive music: initial state schedule failed graph='{}' state='{}' err='{}'",
                    graph.id,
                    initial.id,
                    error
                );
            }
        }
    }

    pub(super) fn destroy_music_session(&mut self, session_id: AudioMusicSessionId) {
        let Some(session) = self.music_sessions.remove(&session_id) else {
            return;
        };
        let mut instances = session.stems.values().copied().collect::<Vec<_>>();
        if let Some(pending) = session.pending {
            self.rollback_music_actions(&pending.action_ids);
            instances.extend(pending.target_stems.values().copied());
        }
        instances.sort_unstable_by_key(|id| id.0);
        instances.dedup();
        for instance_id in instances {
            self.stop_instance(instance_id);
        }
    }

    pub(super) fn request_music_state(
        &mut self,
        session_id: AudioMusicSessionId,
        target_state: String,
    ) {
        let target_state = target_state.trim().to_owned();
        let Some(snapshot) = self.music_sessions.get(&session_id).cloned() else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        };
        let Some(graph) = self.music_graphs.get(&snapshot.graph).cloned() else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        };
        let Some(target) = graph.state(&target_state).cloned() else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::warn!(
                "interactive music: unknown target state='{}' session_id={}",
                target_state,
                session_id.0
            );
            return;
        };
        if snapshot.active_state.eq_ignore_ascii_case(&target.id)
            || snapshot
                .pending
                .as_ref()
                .is_some_and(|pending| pending.target_state.eq_ignore_ascii_case(&target.id))
        {
            return;
        }

        if let Some(pending) = snapshot.pending.as_ref() {
            if self.transport.sample() >= pending.start_sample {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::trace!(
                    "interactive music: state request rejected during active crossfade session_id={} target='{}'",
                    session_id.0,
                    target.id
                );
                return;
            }
            self.rollback_music_actions(&pending.action_ids);
            if let Some(session) = self.music_sessions.get_mut(&session_id) {
                session.pending = None;
            }
        }

        if snapshot.active_state.is_empty() {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            return;
        }
        let Some(transition) = graph
            .transition(&snapshot.active_state, &target.id)
            .cloned()
        else {
            self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
            newengine_ulog_api::ulog::trace!(
                "interactive music: no authored transition session_id={} from='{}' to='{}'",
                session_id.0,
                snapshot.active_state,
                target.id
            );
            return;
        };
        let boundary = match self
            .transport
            .resolve_schedule_point(&transition.quantization)
        {
            Ok(sample) if sample >= self.transport.sample() => sample,
            Ok(sample) => {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::trace!(
                    "interactive music: quantization resolved in past session_id={} sample={} current={}",
                    session_id.0,
                    sample,
                    self.transport.sample()
                );
                return;
            }
            Err(error) => {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::trace!(
                    "interactive music: quantization rejected session_id={} err='{}'",
                    session_id.0,
                    error
                );
                return;
            }
        };
        match self.plan_music_state(MusicStatePlanRequest {
            graph: &graph,
            object_id: snapshot.object_id,
            current_stems: &snapshot.stems,
            parameters: &snapshot.parameters,
            target_state: &target.id,
            start_sample: boundary,
            crossfade_samples: transition.crossfade_samples,
        }) {
            Ok(pending) => {
                if let Some(session) = self.music_sessions.get_mut(&session_id) {
                    session.pending = Some(pending);
                }
                self.music_transitions_scheduled =
                    self.music_transitions_scheduled.saturating_add(1);
            }
            Err(error) => {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::warn!(
                    "interactive music: transition scheduling failed session_id={} from='{}' to='{}' err='{}'",
                    session_id.0,
                    snapshot.active_state,
                    target.id,
                    error
                );
            }
        }
    }

    fn plan_music_state(
        &mut self,
        request: MusicStatePlanRequest<'_>,
    ) -> Result<PendingMusicTransition, String> {
        let MusicStatePlanRequest {
            graph,
            object_id,
            current_stems,
            parameters,
            target_state,
            start_sample,
            crossfade_samples,
        } = request;
        let state = graph
            .state(target_state)
            .ok_or_else(|| format!("unknown interactive music state '{target_state}'"))?;
        let complete_sample = start_sample.saturating_add(crossfade_samples);
        let mut target_stems = BTreeMap::<String, AudioInstanceId>::new();
        let mut action_ids = Vec::<AudioTransportActionId>::new();
        let result = (|| -> Result<(), String> {
            // Incoming and retained stems are scheduled first so zero-duration transitions never
            // create a gap by stopping the outgoing layer before its replacement is admitted.
            for layer in &state.layers {
                let stem_key = layer.stem.to_ascii_lowercase();
                let stem = graph
                    .stem(&layer.stem)
                    .ok_or_else(|| format!("unknown interactive music stem '{}'", layer.stem))?;
                let target_gain = newengine_audio_api::sanitize_gain(
                    stem.request.gain * stem.request.stream.gain * layer.gain,
                );
                if let Some(instance_id) = current_stems.get(&stem_key).copied() {
                    target_stems.insert(stem_key, instance_id);
                    self.schedule_music_action(
                        &mut action_ids,
                        start_sample,
                        AudioTransportAction::TransitionInstanceGain {
                            instance_id,
                            target_gain,
                            duration_samples: crossfade_samples,
                        },
                    )?;
                    continue;
                }

                let instance_id = self.allocate_music_instance_id();
                let mut request = stem.request.clone();
                request.parameters.overlay_from(parameters);
                if crossfade_samples == 0 {
                    request.gain = newengine_audio_api::sanitize_gain(request.gain * layer.gain);
                } else {
                    request.gain = 0.0;
                }
                self.schedule_music_action(
                    &mut action_ids,
                    start_sample,
                    AudioTransportAction::PlayStream {
                        instance_id,
                        object_id,
                        request: Box::new(request),
                    },
                )?;
                if crossfade_samples > 0 {
                    self.schedule_music_action(
                        &mut action_ids,
                        start_sample,
                        AudioTransportAction::TransitionInstanceGain {
                            instance_id,
                            target_gain,
                            duration_samples: crossfade_samples,
                        },
                    )?;
                }
                target_stems.insert(stem_key, instance_id);
            }

            for (stem_key, instance_id) in current_stems {
                if target_stems.contains_key(stem_key) {
                    continue;
                }
                if crossfade_samples > 0 {
                    self.schedule_music_action(
                        &mut action_ids,
                        start_sample,
                        AudioTransportAction::TransitionInstanceGain {
                            instance_id: *instance_id,
                            target_gain: 0.0,
                            duration_samples: crossfade_samples,
                        },
                    )?;
                }
                self.schedule_music_action(
                    &mut action_ids,
                    complete_sample,
                    AudioTransportAction::StopInstance {
                        instance_id: *instance_id,
                    },
                )?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.rollback_music_actions(&action_ids);
            return Err(error);
        }
        Ok(PendingMusicTransition {
            target_state: state.id.clone(),
            target_stems,
            action_ids,
            start_sample,
            complete_sample,
        })
    }

    pub(super) fn set_music_scalar(
        &mut self,
        session_id: AudioMusicSessionId,
        name: String,
        value: f32,
    ) {
        let (object_id, graph_key, parameters) = {
            let Some(session) = self.music_sessions.get_mut(&session_id) else {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                return;
            };
            if let Err(error) = session.parameters.set_scalar(name.clone(), value) {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::trace!(
                    "interactive music: scalar rejected err='{}'",
                    error
                );
                return;
            }
            (
                session.object_id,
                session.graph.clone(),
                session.parameters.clone(),
            )
        };
        self.set_scalar(AudioParameterTarget::Object(object_id), name, value);
        let selected = self
            .music_graphs
            .get(&graph_key)
            .and_then(|graph| graph.selected_state(&parameters))
            .map(str::to_owned);
        if let Some(state) = selected {
            self.request_music_state(session_id, state);
        }
    }

    pub(super) fn set_music_switch(
        &mut self,
        session_id: AudioMusicSessionId,
        name: String,
        value: String,
    ) {
        let (object_id, graph_key, parameters) = {
            let Some(session) = self.music_sessions.get_mut(&session_id) else {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                return;
            };
            if let Err(error) = session.parameters.set_switch(name.clone(), value.clone()) {
                self.music_transitions_rejected = self.music_transitions_rejected.saturating_add(1);
                newengine_ulog_api::ulog::trace!(
                    "interactive music: switch rejected err='{}'",
                    error
                );
                return;
            }
            (
                session.object_id,
                session.graph.clone(),
                session.parameters.clone(),
            )
        };
        self.set_switch(AudioParameterTarget::Object(object_id), name, value);
        let selected = self
            .music_graphs
            .get(&graph_key)
            .and_then(|graph| graph.selected_state(&parameters))
            .map(str::to_owned);
        if let Some(state) = selected {
            self.request_music_state(session_id, state);
        }
    }

    pub(super) fn finalize_music_transitions(&mut self) {
        let sample = self.transport.sample();
        let ready = self
            .music_sessions
            .iter()
            .filter_map(|(session_id, session)| {
                session
                    .pending
                    .as_ref()
                    .filter(|pending| sample >= pending.complete_sample)
                    .map(|_| *session_id)
            })
            .collect::<Vec<_>>();
        for session_id in ready {
            let Some(session) = self.music_sessions.get_mut(&session_id) else {
                continue;
            };
            let Some(pending) = session.pending.take() else {
                continue;
            };
            session.active_state = pending.target_state;
            session.stems = pending.target_stems;
            self.music_transitions_completed = self.music_transitions_completed.saturating_add(1);
        }
    }

    pub(super) fn reconcile_music_instances(&mut self) {
        let live = self
            .instances
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        for session in self.music_sessions.values_mut() {
            session
                .stems
                .retain(|_, instance_id| live.contains(instance_id));
        }
    }

    pub(super) fn music_state(&self) -> InteractiveMusicRuntimeState {
        let mut sessions_state = BTreeMap::<AudioMusicSessionId, AudioMusicSessionState>::new();
        let mut active_instances = std::collections::BTreeSet::<AudioInstanceId>::new();
        let mut pending_transitions = 0usize;
        for (session_id, session) in &self.music_sessions {
            for instance_id in session.stems.values() {
                if self.instances.contains_key(instance_id) {
                    active_instances.insert(*instance_id);
                }
            }
            if let Some(pending) = &session.pending {
                pending_transitions += 1;
                for instance_id in pending.target_stems.values() {
                    if self.instances.contains_key(instance_id) {
                        active_instances.insert(*instance_id);
                    }
                }
            }
            sessions_state.insert(
                *session_id,
                AudioMusicSessionState {
                    graph: session.graph.clone(),
                    object_id: session.object_id.0,
                    active_state: session.active_state.clone(),
                    pending_state: session
                        .pending
                        .as_ref()
                        .map(|pending| pending.target_state.clone()),
                    active_stems: session
                        .stems
                        .values()
                        .filter(|instance_id| self.instances.contains_key(instance_id))
                        .count(),
                    transition_start_sample: session
                        .pending
                        .as_ref()
                        .map(|pending| pending.start_sample),
                    transition_complete_sample: session
                        .pending
                        .as_ref()
                        .map(|pending| pending.complete_sample),
                },
            );
        }
        InteractiveMusicRuntimeState {
            graphs: self.music_graphs.len(),
            sessions: self.music_sessions.len(),
            active_stems: active_instances.len(),
            pending_transitions,
            transitions_scheduled: self.music_transitions_scheduled,
            transitions_completed: self.music_transitions_completed,
            transitions_rejected: self.music_transitions_rejected,
            sessions_state,
        }
    }
}
