use super::*;

#[derive(Debug)]
pub(super) struct CommandQueue {
    pending: VecDeque<AudioOrchestrationCommand>,
    capacity: usize,
    dropped: u64,
}

impl CommandQueue {
    fn new(capacity: usize, initial_reserve: usize) -> Self {
        Self {
            pending: VecDeque::with_capacity(initial_reserve.min(capacity)),
            capacity,
            dropped: 0,
        }
    }

    fn push(&mut self, command: AudioOrchestrationCommand) -> Result<(), String> {
        if self.pending.len() >= self.capacity {
            self.dropped = self.dropped.saturating_add(1);
            return Err(format!(
                "audio orchestration command queue is full (capacity={})",
                self.capacity
            ));
        }
        self.pending.push_back(command);
        Ok(())
    }

    pub(super) fn drain_into(&mut self, target: &mut Vec<AudioOrchestrationCommand>) {
        target.clear();
        if target.capacity() < self.pending.len() {
            target.reserve(self.pending.len());
        }
        target.extend(self.pending.drain(..));
    }

    #[cfg(test)]
    pub(super) fn drain(&mut self) -> Vec<AudioOrchestrationCommand> {
        self.pending.drain(..).collect()
    }
}

/// Project-facing in-process command handle. It is deliberately above `engine.audio`: projects
/// submit semantic object/mix operations here, while this runtime translates them into the stable
/// cue/voice gateway. No gameplay entity classes or project-known event names live in the engine.
#[derive(Clone)]
pub struct AudioOrchestrationHandle {
    pub(super) queue: Arc<Mutex<CommandQueue>>,
    pub(super) next_object_id: Arc<AtomicU64>,
    pub(super) next_instance_id: Arc<AtomicU64>,
    pub(super) next_transport_action_id: Arc<AtomicU64>,
    pub(super) next_music_session_id: Arc<AtomicU64>,
    pub(super) transport_events: Arc<Mutex<VecDeque<AudioTransportMarkerOccurrence>>>,
    pub(super) dropped_transport_events: Arc<AtomicU64>,
    pub(super) config: AudioOrchestrationRuntimeConfig,
}

impl Default for AudioOrchestrationHandle {
    fn default() -> Self {
        Self::with_config(AudioOrchestrationRuntimeConfig::default())
            .expect("default audio orchestration runtime config must be valid")
    }
}

impl AudioOrchestrationHandle {
    pub fn with_capacity(capacity: usize) -> Self {
        let defaults = AudioOrchestrationRuntimeConfig::default();
        let config = AudioOrchestrationRuntimeConfig {
            command_capacity: capacity.max(1),
            command_initial_reserve: defaults.command_initial_reserve.min(capacity.max(1)),
            ..defaults
        };
        Self::with_config(config).expect("with_capacity constructs a valid runtime config")
    }

    pub fn with_config(config: AudioOrchestrationRuntimeConfig) -> Result<Self, String> {
        let config = config.validate()?;
        Ok(Self {
            queue: Arc::new(Mutex::new(CommandQueue::new(
                config.command_capacity,
                config.command_initial_reserve,
            ))),
            next_object_id: Arc::new(AtomicU64::new(1)),
            next_instance_id: Arc::new(AtomicU64::new(1)),
            next_transport_action_id: Arc::new(AtomicU64::new(1)),
            next_music_session_id: Arc::new(AtomicU64::new(1)),
            transport_events: Arc::new(Mutex::new(VecDeque::with_capacity(
                config
                    .command_initial_reserve
                    .min(config.transport_event_capacity),
            ))),
            dropped_transport_events: Arc::new(AtomicU64::new(0)),
            config,
        })
    }

    pub fn submit(&self, command: AudioOrchestrationCommand) -> Result<(), String> {
        self.queue.lock().push(command)
    }

    pub fn install_mix_graph(&self, graph: AudioMixGraph) -> Result<(), String> {
        graph.validate()?;
        // Publish the zero-snapshot route table synchronously so direct engine.audio plays cannot
        // race the first orchestration update. The provider receives only flattened opaque route
        // gains; hierarchy remains exclusively owned by AudioMixGraph/orchestration.
        let empty_weights = BTreeMap::new();
        for bus in &graph.buses {
            let gain = graph
                .effective_linear_gain(&bus.id, &empty_weights)?
                .clamp(0.0, 16.0);
            if let Err(error) = set_audio_route_gain(&AudioRouteGainRequest {
                route: bus.id.clone(),
                gain,
            }) {
                newengine_ulog_api::ulog::trace!(
                    "audio orchestration: initial route publish deferred route='{}' err='{}'",
                    bus.id.0,
                    error
                );
            }
        }
        self.submit(AudioOrchestrationCommand::InstallMixGraph { graph })
    }

    pub fn create_object(&self, state: AudioObjectState) -> Result<AudioObjectId, String> {
        let id = AudioObjectId(self.next_object_id.fetch_add(1, Ordering::Relaxed).max(1));
        self.submit(AudioOrchestrationCommand::CreateObject {
            object_id: id,
            state: state.sanitized(),
        })?;
        Ok(id)
    }

    pub fn destroy_object(&self, object_id: AudioObjectId) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::DestroyObject { object_id })
    }

    pub fn update_object(
        &self,
        object_id: AudioObjectId,
        state: AudioObjectState,
    ) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::UpdateObject {
            object_id,
            state: state.sanitized(),
        })
    }

    pub fn play(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
    ) -> Result<AudioInstanceId, String> {
        let request = request.sanitized()?;
        let id = AudioInstanceId(self.next_instance_id.fetch_add(1, Ordering::Relaxed).max(1));
        self.submit(AudioOrchestrationCommand::Play {
            instance_id: id,
            object_id,
            request,
        })?;
        Ok(id)
    }

    pub fn play_stream(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayStreamInstanceRequest,
    ) -> Result<AudioInstanceId, String> {
        let request = request.sanitized()?;
        let id = AudioInstanceId(self.next_instance_id.fetch_add(1, Ordering::Relaxed).max(1));
        self.submit(AudioOrchestrationCommand::PlayStream {
            instance_id: id,
            object_id,
            request,
        })?;
        Ok(id)
    }

    pub fn stop(&self, instance_id: AudioInstanceId) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::StopInstance { instance_id })
    }

    pub fn stop_by_tag(
        &self,
        object_id: AudioObjectId,
        tag: impl Into<String>,
    ) -> Result<(), String> {
        let tag = tag.into();
        if tag.trim().is_empty() {
            return Err("audio instance tag must not be empty".to_owned());
        }
        self.submit(AudioOrchestrationCommand::StopByTag {
            object_id,
            tag: tag.trim().to_owned(),
        })
    }

    pub fn set_scalar(
        &self,
        target: AudioParameterTarget,
        name: impl Into<String>,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err("audio scalar parameter must be finite".to_owned());
        }
        self.submit(AudioOrchestrationCommand::SetScalar {
            target,
            name: name.into(),
            value,
        })
    }

    pub fn set_switch(
        &self,
        target: AudioParameterTarget,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::SetSwitch {
            target,
            name: name.into(),
            value: value.into(),
        })
    }

    pub fn activate_snapshot(
        &self,
        snapshot: impl Into<String>,
        weight: f32,
        transition_seconds: Option<f32>,
    ) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::ActivateSnapshot {
            snapshot: snapshot.into(),
            weight,
            transition_seconds,
        })
    }

    pub fn deactivate_snapshot(
        &self,
        snapshot: impl Into<String>,
        transition_seconds: Option<f32>,
    ) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::DeactivateSnapshot {
            snapshot: snapshot.into(),
            transition_seconds,
        })
    }

    pub fn configure_transport(&self, config: AudioTransportConfig) -> Result<(), String> {
        let config = config.validate()?;
        self.submit(AudioOrchestrationCommand::ConfigureTransport { config })
    }

    pub fn schedule_transport_action(
        &self,
        when: AudioTransportSchedulePoint,
        action: AudioTransportAction,
    ) -> Result<AudioTransportActionId, String> {
        let action = action.validate()?;
        let id = AudioTransportActionId(
            self.next_transport_action_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        );
        self.submit(AudioOrchestrationCommand::ScheduleTransport {
            action_id: id,
            when,
            action,
        })?;
        Ok(id)
    }

    pub fn schedule_play(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
        when: AudioTransportSchedulePoint,
    ) -> Result<(AudioInstanceId, AudioTransportActionId), String> {
        let request = request.sanitized()?;
        let instance_id =
            AudioInstanceId(self.next_instance_id.fetch_add(1, Ordering::Relaxed).max(1));
        let action_id = self.schedule_transport_action(
            when,
            AudioTransportAction::Play {
                instance_id,
                object_id,
                request,
            },
        )?;
        Ok((instance_id, action_id))
    }

    pub fn schedule_stream(
        &self,
        object_id: AudioObjectId,
        request: AudioPlayStreamInstanceRequest,
        when: AudioTransportSchedulePoint,
    ) -> Result<(AudioInstanceId, AudioTransportActionId), String> {
        let request = request.sanitized()?;
        let instance_id =
            AudioInstanceId(self.next_instance_id.fetch_add(1, Ordering::Relaxed).max(1));
        let action_id = self.schedule_transport_action(
            when,
            AudioTransportAction::PlayStream {
                instance_id,
                object_id,
                request,
            },
        )?;
        Ok((instance_id, action_id))
    }

    pub fn cancel_transport_action(&self, action_id: AudioTransportActionId) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::CancelTransportAction { action_id })
    }

    pub fn drain_transport_markers(&self) -> Vec<AudioTransportMarkerOccurrence> {
        self.transport_events.lock().drain(..).collect()
    }

    #[inline]
    pub fn dropped_transport_events(&self) -> u64 {
        self.dropped_transport_events.load(Ordering::Relaxed)
    }

    pub fn install_music_graph(&self, graph: InteractiveMusicGraph) -> Result<(), String> {
        graph.validate()?;
        self.submit(AudioOrchestrationCommand::InstallMusicGraph { graph })
    }

    pub fn create_music_session(
        &self,
        graph: String,
        object_id: AudioObjectId,
    ) -> Result<AudioMusicSessionId, String> {
        let graph = graph.trim().to_owned();
        if graph.is_empty() {
            return Err("interactive music session requires a graph id".to_owned());
        }
        if object_id.0 == 0 {
            return Err("interactive music session requires a non-zero AudioObjectId".to_owned());
        }
        let session_id = AudioMusicSessionId(
            self.next_music_session_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        );
        self.submit(AudioOrchestrationCommand::CreateMusicSession {
            session_id,
            graph,
            object_id,
        })?;
        Ok(session_id)
    }

    pub fn destroy_music_session(&self, session_id: AudioMusicSessionId) -> Result<(), String> {
        self.submit(AudioOrchestrationCommand::DestroyMusicSession { session_id })
    }

    pub fn request_music_state(
        &self,
        session_id: AudioMusicSessionId,
        state: String,
    ) -> Result<(), String> {
        let state = state.trim().to_owned();
        if state.is_empty() {
            return Err("interactive music state must not be empty".to_owned());
        }
        self.submit(AudioOrchestrationCommand::RequestMusicState { session_id, state })
    }

    pub fn set_music_scalar(
        &self,
        session_id: AudioMusicSessionId,
        name: String,
        value: f32,
    ) -> Result<(), String> {
        if name.trim().is_empty() || !value.is_finite() {
            return Err(
                "interactive music scalar requires a non-empty name and finite value".to_owned(),
            );
        }
        self.submit(AudioOrchestrationCommand::SetMusicScalar {
            session_id,
            name: name.trim().to_owned(),
            value,
        })
    }

    pub fn set_music_switch(
        &self,
        session_id: AudioMusicSessionId,
        name: String,
        value: String,
    ) -> Result<(), String> {
        let name = name.trim().to_owned();
        let value = value.trim().to_owned();
        if name.is_empty() || value.is_empty() {
            return Err("interactive music switch requires non-empty name/value".to_owned());
        }
        self.submit(AudioOrchestrationCommand::SetMusicSwitch {
            session_id,
            name,
            value,
        })
    }

    #[inline]
    pub fn dropped_commands(&self) -> u64 {
        self.queue.lock().dropped
    }
}
