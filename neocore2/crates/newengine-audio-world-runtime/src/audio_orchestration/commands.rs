pub struct AudioOrchestrationRuntimeModule {
    handle: AudioOrchestrationHandle,
    objects: HashMap<AudioObjectId, RuntimeObject>,
    instances: HashMap<AudioInstanceId, RuntimeInstance>,
    global_parameters: AudioParameterSet,
    mix_graph: Option<AudioMixGraph>,
    snapshots: BTreeMap<String, ActiveSnapshot>,
    scalar_transitions: BTreeMap<(AudioParameterTarget, String), SampleTransition>,
    instance_gain_transitions: BTreeMap<AudioInstanceId, SampleTransition>,
    music_graphs: BTreeMap<String, InteractiveMusicGraph>,
    music_sessions: BTreeMap<AudioMusicSessionId, RuntimeMusicSession>,
    music_transitions_scheduled: u64,
    music_transitions_completed: u64,
    music_transitions_rejected: u64,
    transport: AudioTransportRuntime,
    provider_clock: Option<AudioRenderClock>,
    provider_clock_anchor: Option<ProviderClockAnchor>,
    prearmed_transport_actions: BTreeMap<AudioTransportActionId, PrearmedTransportAction>,
    provider_gain_ramp_until: BTreeMap<AudioInstanceId, u64>,
    command_scratch: Vec<AudioOrchestrationCommand>,
}

impl AudioOrchestrationRuntimeModule {
    pub fn new(handle: AudioOrchestrationHandle) -> Self {
        let command_scratch = Vec::with_capacity(handle.config.command_initial_reserve);
        Self {
            handle,
            objects: HashMap::new(),
            instances: HashMap::new(),
            global_parameters: AudioParameterSet::default(),
            mix_graph: None,
            snapshots: BTreeMap::new(),
            scalar_transitions: BTreeMap::new(),
            instance_gain_transitions: BTreeMap::new(),
            music_graphs: BTreeMap::new(),
            music_sessions: BTreeMap::new(),
            music_transitions_scheduled: 0,
            music_transitions_completed: 0,
            music_transitions_rejected: 0,
            transport: AudioTransportRuntime::default(),
            provider_clock: None,
            provider_clock_anchor: None,
            prearmed_transport_actions: BTreeMap::new(),
            provider_gain_ramp_until: BTreeMap::new(),
            command_scratch,
        }
    }

    fn process_commands(&mut self) {
        let mut commands = std::mem::take(&mut self.command_scratch);
        self.handle.queue.lock().drain_into(&mut commands);
        for command in commands.drain(..) {
            self.apply_command(command);
        }
        self.command_scratch = commands;
    }

    fn apply_command(&mut self, command: AudioOrchestrationCommand) {
        match command {
            AudioOrchestrationCommand::InstallMixGraph { graph } => match graph.validate() {
                Ok(()) => {
                    let budget_config = AudioVoiceBudgetConfig {
                        reservations: graph.voice_budgets.clone(),
                    };
                    match set_audio_voice_budgets(&budget_config) {
                        Ok(Some(ack)) if !ack.accepted => {
                            newengine_ulog_api::ulog::warn!(
                                "audio orchestration: provider rejected voice budgets max_physical_voices={} message='{}'",
                                ack.max_physical_voices,
                                ack.message
                            );
                        }
                        Ok(_) => {}
                        Err(error) => {
                            newengine_ulog_api::ulog::warn!(
                                "audio orchestration: voice budget publish failed err='{}'",
                                error
                            );
                        }
                    }
                    self.snapshots.retain(|id, _| graph.snapshot(id).is_some());
                    self.mix_graph = Some(graph);
                    self.sync_provider_route_gains();
                    if let Some(graph) = self.mix_graph.as_ref() {
                        newengine_ulog_api::ulog::info!(
                            "audio orchestration: project mix graph installed routes={} snapshots={} policy='opaque-route-authority'",
                            graph.buses.len(), graph.snapshots.len()
                        );
                    }
                }
                Err(error) => {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: rejected mix graph err='{}'",
                        error
                    );
                }
            },
            AudioOrchestrationCommand::CreateObject { object_id, state } => {
                if self
                    .objects
                    .insert(
                        object_id,
                        RuntimeObject {
                            state: (*state).sanitized(),
                        },
                    )
                    .is_some()
                {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: create replaced existing object_id={}",
                        object_id.0
                    );
                }
            }
            AudioOrchestrationCommand::DestroyObject { object_id } => {
                let music_sessions = self
                    .music_sessions
                    .iter()
                    .filter_map(|(session_id, session)| {
                        (session.object_id == object_id).then_some(*session_id)
                    })
                    .collect::<Vec<_>>();
                for session_id in music_sessions {
                    self.destroy_music_session(session_id);
                }
                self.stop_object_instances(object_id);
                self.clear_scalar_transitions_for_target(&AudioParameterTarget::Object(object_id));
                self.objects.remove(&object_id);
            }
            AudioOrchestrationCommand::UpdateObject { object_id, state } => {
                self.clear_scalar_transitions_for_target(&AudioParameterTarget::Object(object_id));
                if let Some(object) = self.objects.get_mut(&object_id) {
                    object.state = (*state).sanitized();
                } else {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: update ignored unknown object_id={}",
                        object_id.0
                    );
                }
            }
            AudioOrchestrationCommand::Play {
                instance_id,
                object_id,
                request,
            } => {
                let sample = self.transport.sample();
                self.play_instance(instance_id, object_id, request, sample, sample, None)
            }
            AudioOrchestrationCommand::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                let sample = self.transport.sample();
                self.play_stream_instance(instance_id, object_id, *request, sample, sample, None)
            }
            AudioOrchestrationCommand::StopInstance { instance_id } => {
                self.stop_instance(instance_id);
            }
            AudioOrchestrationCommand::StopByTag { object_id, tag } => {
                let targets = self
                    .instances
                    .iter()
                    .filter(|(_, instance)| {
                        instance.object_id == object_id
                            && instance.tags.iter().any(|candidate| candidate == &tag)
                    })
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>();
                for instance_id in targets {
                    self.stop_instance(instance_id);
                }
            }
            AudioOrchestrationCommand::SetScalar {
                target,
                name,
                value,
            } => self.set_scalar(target, name, value),
            AudioOrchestrationCommand::SetSwitch {
                target,
                name,
                value,
            } => self.set_switch(target, name, value),
            AudioOrchestrationCommand::ActivateSnapshot {
                snapshot,
                weight,
                transition_seconds,
            } => self.activate_snapshot(&snapshot, weight, transition_seconds),
            AudioOrchestrationCommand::DeactivateSnapshot {
                snapshot,
                transition_seconds,
            } => self.deactivate_snapshot(&snapshot, transition_seconds),
            AudioOrchestrationCommand::ConfigureTransport { config } => {
                if let Err(error) = self.transport.configure(config) {
                    newengine_ulog_api::ulog::warn!(
                        "audio transport: configuration rejected err='{}'",
                        error
                    );
                } else {
                    self.provider_clock_anchor =
                        self.provider_clock.map(|clock| ProviderClockAnchor {
                            transport_sample: self.transport.sample(),
                            provider_sample: clock.sample,
                            provider_rate: clock.sample_rate,
                        });
                }
            }
            AudioOrchestrationCommand::ScheduleTransport {
                action_id,
                when,
                action,
            } => {
                if let Err(error) = self.transport.schedule(action_id, when, action) {
                    newengine_ulog_api::ulog::warn!(
                        "audio transport: schedule rejected action_id={} err='{}'",
                        action_id.0,
                        error
                    );
                }
            }
            AudioOrchestrationCommand::CancelTransportAction { action_id } => {
                if self.transport.cancel(action_id) {
                    self.cancel_prearmed_transport_action(action_id);
                } else {
                    newengine_ulog_api::ulog::trace!(
                        "audio transport: cancel ignored unknown action_id={}",
                        action_id.0
                    );
                }
            }
            AudioOrchestrationCommand::InstallMusicGraph { graph } => {
                self.install_music_graph(graph);
            }
            AudioOrchestrationCommand::CreateMusicSession {
                session_id,
                graph,
                object_id,
            } => self.create_music_session(session_id, graph, object_id),
            AudioOrchestrationCommand::DestroyMusicSession { session_id } => {
                self.destroy_music_session(session_id);
            }
            AudioOrchestrationCommand::RequestMusicState { session_id, state } => {
                self.request_music_state(session_id, state);
            }
            AudioOrchestrationCommand::SetMusicScalar {
                session_id,
                name,
                value,
            } => self.set_music_scalar(session_id, name, value),
            AudioOrchestrationCommand::SetMusicSwitch {
                session_id,
                name,
                value,
            } => self.set_music_switch(session_id, name, value),
        }
    }
}
