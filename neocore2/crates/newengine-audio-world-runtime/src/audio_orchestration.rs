use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use newengine_audio_api::{
    AudioInstanceId, AudioMixGraph, AudioMusicSessionId, AudioMusicSessionState, AudioObjectId,
    AudioObjectState, AudioOrchestrationCommand, AudioParameterSet, AudioParameterTarget,
    AudioPlayInstanceRequest, AudioPlayStreamInstanceRequest, AudioRouteId, AudioTransportAction,
    AudioTransportActionId, AudioTransportConfig, AudioTransportInstanceState,
    AudioTransportMarkerOccurrence, AudioTransportRuntimeState, AudioTransportSchedulePoint,
    AudioVoiceBudgetConfig, AudioVoiceUpdateRequest, InteractiveMusicGraph,
    InteractiveMusicRuntimeState,
};
use newengine_audio_client::{
    play_audio_cue, play_audio_stream, set_audio_voice_budgets, stop_audio_voice,
    update_audio_voice,
};
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::audio_transport::{AudioTransportHandle, AudioTransportRuntime, DueTransportAction};
use crate::interactive_music::InteractiveMusicHandle;

const DEFAULT_COMMAND_CAPACITY: usize = 2_048;
const DEFAULT_TRANSPORT_EVENT_CAPACITY: usize = 4_096;

#[derive(Debug)]
struct CommandQueue {
    pending: VecDeque<AudioOrchestrationCommand>,
    capacity: usize,
    dropped: u64,
}

impl CommandQueue {
    fn new(capacity: usize) -> Self {
        Self {
            pending: VecDeque::with_capacity(capacity.min(256)),
            capacity: capacity.max(1),
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

    fn drain(&mut self) -> Vec<AudioOrchestrationCommand> {
        self.pending.drain(..).collect()
    }
}

/// Project-facing in-process command handle. It is deliberately above `engine.audio`: projects
/// submit semantic object/mix operations here, while this runtime translates them into the stable
/// cue/voice gateway. No gameplay entity classes or project-known event names live in the engine.
#[derive(Clone)]
pub struct AudioOrchestrationHandle {
    queue: Arc<Mutex<CommandQueue>>,
    next_object_id: Arc<AtomicU64>,
    next_instance_id: Arc<AtomicU64>,
    next_transport_action_id: Arc<AtomicU64>,
    next_music_session_id: Arc<AtomicU64>,
    transport_events: Arc<Mutex<VecDeque<AudioTransportMarkerOccurrence>>>,
    dropped_transport_events: Arc<AtomicU64>,
}

impl Default for AudioOrchestrationHandle {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_COMMAND_CAPACITY)
    }
}

impl AudioOrchestrationHandle {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(CommandQueue::new(capacity))),
            next_object_id: Arc::new(AtomicU64::new(1)),
            next_instance_id: Arc::new(AtomicU64::new(1)),
            next_transport_action_id: Arc::new(AtomicU64::new(1)),
            next_music_session_id: Arc::new(AtomicU64::new(1)),
            transport_events: Arc::new(Mutex::new(VecDeque::new())),
            dropped_transport_events: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn submit(&self, command: AudioOrchestrationCommand) -> Result<(), String> {
        self.queue.lock().push(command)
    }

    pub fn install_mix_graph(&self, graph: AudioMixGraph) -> Result<(), String> {
        graph.validate()?;
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

#[derive(Clone, Debug, Default)]
pub struct AudioOrchestrationRuntimeState {
    pub objects: usize,
    pub instances: usize,
    pub provider_voices: usize,
    pub logical_routes: usize,
    pub active_snapshots: BTreeMap<String, f32>,
    pub dropped_commands: u64,
    pub dropped_transport_events: u64,
    pub transport: AudioTransportRuntimeState,
    pub transport_instances: BTreeMap<AudioInstanceId, AudioTransportInstanceState>,
    pub music: InteractiveMusicRuntimeState,
}

#[derive(Clone, Debug)]
struct RuntimeObject {
    state: AudioObjectState,
}

#[derive(Clone, Debug)]
struct RuntimeInstance {
    object_id: AudioObjectId,
    voice_ids: Vec<u64>,
    route: AudioRouteId,
    tags: Vec<String>,
    gain: f32,
    spatial: bool,
    parameters: AudioParameterSet,
    transport_start_sample: u64,
    transport_dispatch_sample: u64,
}

#[derive(Clone, Copy, Debug)]
struct SampleSnapshotTransition {
    from: f32,
    target: f32,
    start_sample: u64,
    end_sample: u64,
}

#[derive(Clone, Copy, Debug)]
struct SampleScalarTransition {
    from: f32,
    target: f32,
    start_sample: u64,
    end_sample: u64,
}

#[derive(Clone, Copy, Debug)]
struct SampleInstanceGainTransition {
    from: f32,
    target: f32,
    start_sample: u64,
    end_sample: u64,
}

#[derive(Clone, Debug)]
struct PendingMusicTransition {
    target_state: String,
    target_stems: BTreeMap<String, AudioInstanceId>,
    action_ids: Vec<AudioTransportActionId>,
    start_sample: u64,
    complete_sample: u64,
}

#[derive(Clone, Debug)]
struct RuntimeMusicSession {
    graph: String,
    object_id: AudioObjectId,
    active_state: String,
    stems: BTreeMap<String, AudioInstanceId>,
    parameters: AudioParameterSet,
    pending: Option<PendingMusicTransition>,
}

#[derive(Clone, Copy, Debug)]
struct ActiveSnapshot {
    current: f32,
    target: f32,
    remaining_seconds: f32,
    sample_transition: Option<SampleSnapshotTransition>,
}

impl ActiveSnapshot {
    fn new(target: f32, transition_seconds: f32) -> Self {
        let target = sanitize_weight(target);
        let transition_seconds = sanitize_transition(transition_seconds);
        Self {
            current: if transition_seconds <= 0.0 {
                target
            } else {
                0.0
            },
            target,
            remaining_seconds: transition_seconds,
            sample_transition: None,
        }
    }

    fn retarget(&mut self, target: f32, transition_seconds: f32) {
        self.target = sanitize_weight(target);
        self.remaining_seconds = sanitize_transition(transition_seconds);
        self.sample_transition = None;
        if self.remaining_seconds <= 0.0 {
            self.current = self.target;
        }
    }

    fn retarget_samples(&mut self, target: f32, start_sample: u64, duration_samples: u64) {
        let target = sanitize_weight(target);
        self.target = target;
        self.remaining_seconds = 0.0;
        if duration_samples == 0 {
            self.current = target;
            self.sample_transition = None;
            return;
        }
        self.sample_transition = Some(SampleSnapshotTransition {
            from: self.current,
            target,
            start_sample,
            end_sample: start_sample.saturating_add(duration_samples),
        });
    }

    fn advance_seconds(&mut self, dt: f32) {
        if self.sample_transition.is_some() {
            return;
        }
        if self.remaining_seconds <= 0.0 {
            self.current = self.target;
            return;
        }
        let dt = dt.max(0.0).min(self.remaining_seconds);
        let t = (dt / self.remaining_seconds).clamp(0.0, 1.0);
        self.current += (self.target - self.current) * t;
        self.remaining_seconds = (self.remaining_seconds - dt).max(0.0);
        if self.remaining_seconds <= f32::EPSILON {
            self.current = self.target;
            self.remaining_seconds = 0.0;
        }
    }

    fn advance_to_sample(&mut self, sample: u64) {
        let Some(transition) = self.sample_transition else {
            return;
        };
        if sample <= transition.start_sample {
            self.current = transition.from;
            return;
        }
        if sample >= transition.end_sample {
            self.current = transition.target;
            self.target = transition.target;
            self.sample_transition = None;
            return;
        }
        let elapsed = sample - transition.start_sample;
        let duration = transition.end_sample - transition.start_sample;
        let t = elapsed as f64 / duration.max(1) as f64;
        self.current = (f64::from(transition.from)
            + (f64::from(transition.target) - f64::from(transition.from)) * t)
            as f32;
    }
}

pub struct AudioOrchestrationRuntimeModule {
    handle: AudioOrchestrationHandle,
    objects: HashMap<AudioObjectId, RuntimeObject>,
    instances: HashMap<AudioInstanceId, RuntimeInstance>,
    global_parameters: AudioParameterSet,
    mix_graph: Option<AudioMixGraph>,
    snapshots: BTreeMap<String, ActiveSnapshot>,
    scalar_transitions: BTreeMap<(AudioParameterTarget, String), SampleScalarTransition>,
    instance_gain_transitions: BTreeMap<AudioInstanceId, SampleInstanceGainTransition>,
    music_graphs: BTreeMap<String, InteractiveMusicGraph>,
    music_sessions: BTreeMap<AudioMusicSessionId, RuntimeMusicSession>,
    music_transitions_scheduled: u64,
    music_transitions_completed: u64,
    music_transitions_rejected: u64,
    transport: AudioTransportRuntime,
}

impl AudioOrchestrationRuntimeModule {
    pub fn new(handle: AudioOrchestrationHandle) -> Self {
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
        }
    }

    fn drain_commands(&self) -> Vec<AudioOrchestrationCommand> {
        self.handle.queue.lock().drain()
    }

    fn publish_transport_markers(&self, markers: Vec<AudioTransportMarkerOccurrence>) {
        if markers.is_empty() {
            return;
        }
        let mut events = self.handle.transport_events.lock();
        for marker in markers {
            if events.len() >= DEFAULT_TRANSPORT_EVENT_CAPACITY {
                events.pop_front();
                self.handle
                    .dropped_transport_events
                    .fetch_add(1, Ordering::Relaxed);
            }
            events.push_back(marker);
        }
    }

    fn apply_due_transport_action(&mut self, due: DueTransportAction) {
        if due.lateness_samples > 0 {
            newengine_ulog_api::ulog::trace!(
                "audio transport: late logical dispatch action_id={} intended_sample={} dispatch_sample={} lateness_samples={}",
                due.id.0,
                due.intended_sample,
                due.dispatch_sample,
                due.lateness_samples
            );
        }
        match due.action {
            AudioTransportAction::Play {
                instance_id,
                object_id,
                request,
            } => self.play_instance(
                instance_id,
                object_id,
                request,
                due.intended_sample,
                due.dispatch_sample,
            ),
            AudioTransportAction::PlayStream {
                instance_id,
                object_id,
                request,
            } => self.play_stream_instance(
                instance_id,
                object_id,
                request,
                due.intended_sample,
                due.dispatch_sample,
            ),
            AudioTransportAction::StopInstance { instance_id } => self.stop_instance(instance_id),
            AudioTransportAction::SetScalar {
                target,
                name,
                value,
            } => self.set_scalar(target, name, value),
            AudioTransportAction::SetSwitch {
                target,
                name,
                value,
            } => self.set_switch(target, name, value),
            AudioTransportAction::TransitionScalar {
                target,
                name,
                target_value,
                duration_samples,
            } => self.transition_scalar_samples(
                target,
                name,
                target_value,
                due.intended_sample,
                duration_samples,
            ),
            AudioTransportAction::TransitionInstanceGain {
                instance_id,
                target_gain,
                duration_samples,
            } => self.transition_instance_gain_samples(
                instance_id,
                target_gain,
                due.intended_sample,
                duration_samples,
            ),
            AudioTransportAction::TransitionSnapshot {
                snapshot,
                target_weight,
                duration_samples,
            } => self.transition_snapshot_samples(
                &snapshot,
                target_weight,
                due.intended_sample,
                duration_samples,
            ),
        }
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
                            state: state.sanitized(),
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
                    object.state = state.sanitized();
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
                self.play_instance(instance_id, object_id, request, sample, sample)
            }
            AudioOrchestrationCommand::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                let sample = self.transport.sample();
                self.play_stream_instance(instance_id, object_id, request, sample, sample)
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
                if !self.transport.cancel(action_id) {
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

    fn play_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
        transport_start_sample: u64,
        transport_dispatch_sample: u64,
    ) {
        let request = match request.sanitized() {
            Ok(request) => request,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: play rejected object_id={} instance_id={} err='{}'",
                    object_id.0,
                    instance_id.0,
                    error
                );
                return;
            }
        };
        let Some(object) = self.objects.get(&object_id).cloned() else {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: play ignored unknown object_id={} instance_id={}",
                object_id.0,
                instance_id.0
            );
            return;
        };
        if !request.route.0.is_empty()
            && self
                .mix_graph
                .as_ref()
                .is_some_and(|graph| !graph.contains_route(&request.route))
        {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: play rejected unknown route='{}' instance_id={}",
                request.route.0,
                instance_id.0
            );
            return;
        }
        self.stop_instance(instance_id);

        let mix_gain = self.route_gain(&request.route);
        let mut play =
            newengine_audio_api::AudioCuePlayRequest::new(request.cue.logical_path.clone());
        play.position = request.spatial.then_some(object.state.position);
        play.gain = object.state.gain * request.gain * mix_gain;
        play.pitch = request.pitch;
        play.seed = request
            .seed
            .or(Some(instance_id.0 ^ object_id.0.rotate_left(23)));
        play.scope_id = Some(object_id.0);
        play.start_sample_offset = transport_dispatch_sample.saturating_sub(transport_start_sample);
        if play.start_sample_offset > 0 {
            play.transport_sample_rate = self.transport.sample_rate();
        }
        play.acoustic = object.state.acoustic;
        play.environment = object.state.environment;
        let mut parameters = self.global_parameters.clone();
        parameters.overlay_from(&object.state.parameters);
        parameters.overlay_from(&request.parameters);
        play.parameters = parameters.sanitized();

        match play_audio_cue(&play) {
            Ok(Some(ack)) if ack.accepted => {
                let mut voice_ids = ack.voice_ids;
                if voice_ids.is_empty() {
                    voice_ids.extend(ack.voice_id);
                }
                voice_ids.sort_unstable();
                voice_ids.dedup();
                if voice_ids.is_empty() {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: accepted cue returned no voice handles cue='{}' instance_id={}",
                        request.cue.logical_path,
                        instance_id.0
                    );
                    return;
                }
                self.instances.insert(
                    instance_id,
                    RuntimeInstance {
                        object_id,
                        voice_ids,
                        route: request.route,
                        tags: request.tags,
                        gain: request.gain,
                        spatial: request.spatial,
                        parameters: request.parameters,
                        transport_start_sample,
                        transport_dispatch_sample,
                    },
                );
            }
            Ok(Some(ack)) => {
                newengine_ulog_api::ulog::trace!(
                    "audio orchestration: cue rejected cue='{}' instance_id={} message='{}'",
                    request.cue.logical_path,
                    instance_id.0,
                    ack.message
                );
            }
            Ok(None) => {}
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: cue play failed cue='{}' instance_id={} err='{}'",
                    request.cue.logical_path,
                    instance_id.0,
                    error
                );
            }
        }
    }

    fn play_stream_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayStreamInstanceRequest,
        transport_start_sample: u64,
        transport_dispatch_sample: u64,
    ) {
        let request = match request.sanitized() {
            Ok(request) => request,
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: stream play rejected object_id={} instance_id={} err='{}'",
                    object_id.0,
                    instance_id.0,
                    error
                );
                return;
            }
        };
        let Some(object) = self.objects.get(&object_id).cloned() else {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: stream play ignored unknown object_id={} instance_id={}",
                object_id.0,
                instance_id.0
            );
            return;
        };
        if !request.route.0.is_empty()
            && self
                .mix_graph
                .as_ref()
                .is_some_and(|graph| !graph.contains_route(&request.route))
        {
            newengine_ulog_api::ulog::warn!(
                "audio orchestration: stream play rejected unknown route='{}' instance_id={}",
                request.route.0,
                instance_id.0
            );
            return;
        }
        self.stop_instance(instance_id);

        let mix_gain = self.route_gain(&request.route);
        let mut stream = request.stream.clone();
        let instance_gain = newengine_audio_api::sanitize_gain(request.gain * stream.gain);
        stream.gain =
            newengine_audio_api::sanitize_gain(object.state.gain * instance_gain * mix_gain);
        if request.spatial {
            stream.spatial = Some(newengine_audio_api::AudioSpatialParams {
                position: object.state.position,
            });
        }
        stream.scope_id = Some(object_id.0);
        stream.acoustic = object.state.acoustic;
        stream.environment = object.state.environment;
        let lateness_samples = transport_dispatch_sample.saturating_sub(transport_start_sample);
        if lateness_samples > 0 {
            stream.start_seconds = (stream.start_seconds
                + lateness_samples as f64 / f64::from(self.transport.sample_rate()))
            .clamp(0.0, 86_400.0);
        }
        stream = stream.sanitized();

        match play_audio_stream(&stream) {
            Ok(Some(ack)) if ack.accepted => {
                let mut voice_ids = ack.voice_ids;
                if voice_ids.is_empty() {
                    voice_ids.extend(ack.voice_id);
                }
                voice_ids.sort_unstable();
                voice_ids.dedup();
                if voice_ids.is_empty() {
                    newengine_ulog_api::ulog::warn!(
                        "audio orchestration: accepted stream returned no voice handles uri='{}' instance_id={}",
                        stream.clip.uri,
                        instance_id.0
                    );
                    return;
                }
                self.instances.insert(
                    instance_id,
                    RuntimeInstance {
                        object_id,
                        voice_ids,
                        route: request.route,
                        tags: request.tags,
                        gain: instance_gain,
                        spatial: request.spatial,
                        parameters: request.parameters,
                        transport_start_sample,
                        transport_dispatch_sample,
                    },
                );
            }
            Ok(Some(ack)) => {
                newengine_ulog_api::ulog::trace!(
                    "audio orchestration: stream rejected uri='{}' instance_id={} message='{}'",
                    stream.clip.uri,
                    instance_id.0,
                    ack.message
                );
            }
            Ok(None) => {}
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "audio orchestration: stream play failed uri='{}' instance_id={} err='{}'",
                    stream.clip.uri,
                    instance_id.0,
                    error
                );
            }
        }
    }

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
            SampleScalarTransition {
                from,
                target: target_value,
                start_sample,
                end_sample: start_sample.saturating_add(duration_samples),
            },
        );
    }

    fn advance_scalar_transitions_to_sample(&mut self, sample: u64) {
        let mut updates = Vec::<((AudioParameterTarget, String), f32, bool)>::new();
        for (key, transition) in &self.scalar_transitions {
            let (value, finished) = if sample <= transition.start_sample {
                (transition.from, false)
            } else if sample >= transition.end_sample {
                (transition.target, true)
            } else {
                let elapsed = sample - transition.start_sample;
                let duration = transition.end_sample - transition.start_sample;
                let t = elapsed as f64 / duration.max(1) as f64;
                (
                    (f64::from(transition.from)
                        + (f64::from(transition.target) - f64::from(transition.from)) * t)
                        as f32,
                    false,
                )
            };
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
            SampleInstanceGainTransition {
                from,
                target,
                start_sample,
                end_sample: start_sample.saturating_add(duration_samples),
            },
        );
    }

    fn advance_instance_gain_transitions_to_sample(&mut self, sample: u64) {
        let mut updates = Vec::<(AudioInstanceId, f32, bool)>::new();
        for (instance_id, transition) in &self.instance_gain_transitions {
            let (value, finished) = if sample <= transition.start_sample {
                (transition.from, false)
            } else if sample >= transition.end_sample {
                (transition.target, true)
            } else {
                let elapsed = sample - transition.start_sample;
                let duration = transition.end_sample - transition.start_sample;
                let t = elapsed as f64 / duration.max(1) as f64;
                (
                    (f64::from(transition.from)
                        + (f64::from(transition.target) - f64::from(transition.from)) * t)
                        as f32,
                    false,
                )
            };
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

    fn allocate_music_instance_id(&self) -> AudioInstanceId {
        AudioInstanceId(
            self.handle
                .next_instance_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        )
    }

    fn allocate_music_action_id(&self) -> AudioTransportActionId {
        AudioTransportActionId(
            self.handle
                .next_transport_action_id
                .fetch_add(1, Ordering::Relaxed)
                .max(1),
        )
    }

    fn schedule_music_action(
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

    fn rollback_music_actions(&mut self, action_ids: &[AudioTransportActionId]) {
        for id in action_ids {
            let _ = self.transport.cancel(*id);
        }
    }

    fn install_music_graph(&mut self, graph: InteractiveMusicGraph) {
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

    fn create_music_session(
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
        match self.plan_music_state(
            &graph,
            object_id,
            &BTreeMap::new(),
            &parameters,
            &initial.id,
            sample,
            0,
        ) {
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

    fn destroy_music_session(&mut self, session_id: AudioMusicSessionId) {
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

    fn request_music_state(&mut self, session_id: AudioMusicSessionId, target_state: String) {
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
        match self.plan_music_state(
            &graph,
            snapshot.object_id,
            &snapshot.stems,
            &snapshot.parameters,
            &target.id,
            boundary,
            transition.crossfade_samples,
        ) {
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
        graph: &InteractiveMusicGraph,
        object_id: AudioObjectId,
        current_stems: &BTreeMap<String, AudioInstanceId>,
        parameters: &AudioParameterSet,
        target_state: &str,
        start_sample: u64,
        crossfade_samples: u64,
    ) -> Result<PendingMusicTransition, String> {
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
                        request,
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

    fn set_music_scalar(&mut self, session_id: AudioMusicSessionId, name: String, value: f32) {
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

    fn set_music_switch(&mut self, session_id: AudioMusicSessionId, name: String, value: String) {
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

    fn finalize_music_transitions(&mut self) {
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

    fn reconcile_music_instances(&mut self) {
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

    fn music_state(&self) -> InteractiveMusicRuntimeState {
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

    fn sync_instances(&mut self) {
        let weights = self.snapshot_weights();
        let mut route_gains = BTreeMap::<AudioRouteId, f32>::new();
        if let Some(graph) = self.mix_graph.as_ref() {
            for route in self
                .instances
                .values()
                .map(|instance| instance.route.clone())
            {
                if route.0.is_empty() || route_gains.contains_key(&route) {
                    continue;
                }
                let gain = graph
                    .effective_linear_gain(&route, &weights)
                    .unwrap_or(1.0)
                    .clamp(0.0, 16.0);
                route_gains.insert(route, gain);
            }
        }

        let mut finished = Vec::<AudioInstanceId>::new();
        for (instance_id, instance) in &mut self.instances {
            let Some(object) = self.objects.get(&instance.object_id) else {
                finished.push(*instance_id);
                continue;
            };
            let route_gain = if instance.route.0.is_empty() {
                1.0
            } else {
                route_gains.get(&instance.route).copied().unwrap_or(1.0)
            };
            let request = AudioVoiceUpdateRequest {
                voice_id: 0,
                gain: Some(object.state.gain * instance.gain * route_gain),
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
                            logical_sample: self
                                .transport
                                .sample()
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

    fn stop_all(&mut self) {
        let instances = self
            .instances
            .drain()
            .map(|(_, instance)| instance)
            .collect::<Vec<_>>();
        for instance in instances {
            Self::stop_voice_ids(&instance.voice_ids);
        }
        self.objects.clear();
        self.snapshots.clear();
        self.scalar_transitions.clear();
        self.instance_gain_transitions.clear();
        self.music_sessions.clear();
        self.music_graphs.clear();
    }
}

impl Module<()> for AudioOrchestrationRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.audio.orchestration.runtime"
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        ctx.resources_mut().insert(self.handle.clone());
        ctx.resources_mut()
            .insert(AudioTransportHandle::new(self.handle.clone()));
        ctx.resources_mut()
            .insert(InteractiveMusicHandle::new(self.handle.clone()));
        ctx.resources_mut().insert(self.transport_state());
        ctx.resources_mut().insert(self.music_state());
        ctx.resources_mut().insert(self.snapshot_state());
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        for command in self.drain_commands() {
            self.apply_command(command);
        }
        let dt = ctx.frame().map(|frame| frame.dt).unwrap_or(1.0 / 60.0);
        let (markers, due_actions) = self.transport.advance_seconds(dt);
        self.publish_transport_markers(markers);
        for due in due_actions {
            self.advance_snapshots_to_sample(due.intended_sample);
            self.advance_scalar_transitions_to_sample(due.intended_sample);
            self.advance_instance_gain_transitions_to_sample(due.intended_sample);
            self.apply_due_transport_action(due);
        }
        self.advance_snapshots_to_sample(self.transport.sample());
        self.advance_scalar_transitions_to_sample(self.transport.sample());
        self.advance_instance_gain_transitions_to_sample(self.transport.sample());
        self.finalize_music_transitions();
        self.advance_snapshots(dt);
        self.sync_instances();
        self.reconcile_music_instances();
        ctx.resources_mut().insert(self.transport_state());
        ctx.resources_mut().insert(self.music_state());
        ctx.resources_mut().insert(self.snapshot_state());
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        self.stop_all();
        ctx.resources_mut().insert(self.transport_state());
        ctx.resources_mut().insert(self.music_state());
        ctx.resources_mut().insert(self.snapshot_state());
        Ok(())
    }
}

fn sanitize_weight(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn sanitize_transition(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 60.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_audio_api::{
        AudioMixBusSpec, AudioMixPatch, AudioMixSnapshotSpec, AudioMusicLayerSpec,
        AudioMusicSelectorCondition, AudioMusicSelectorSpec, AudioMusicStateSpec,
        AudioMusicStemSpec, AudioMusicTransitionSpec, AudioVoiceStealRule,
    };

    #[test]
    fn command_queue_is_bounded_and_reports_drops() {
        let handle = AudioOrchestrationHandle::with_capacity(1);
        let first = handle
            .create_object(AudioObjectState::default())
            .expect("first");
        assert_eq!(first.0, 1);
        assert!(handle.create_object(AudioObjectState::default()).is_err());
        assert_eq!(handle.dropped_commands(), 1);
    }

    #[test]
    fn snapshot_transition_changes_project_route_gain_without_provider_bus_semantics() {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime.mix_graph = Some(AudioMixGraph {
            buses: vec![AudioMixBusSpec {
                id: AudioRouteId::new("my.project.any.route"),
                parent: None,
                gain_db: 0.0,
            }],
            snapshots: vec![AudioMixSnapshotSpec {
                id: "duck".to_owned(),
                transition_seconds: 1.0,
                patches: vec![AudioMixPatch {
                    route: AudioRouteId::new("my.project.any.route"),
                    gain_db: -12.0,
                }],
            }],
            ..Default::default()
        });
        runtime.activate_snapshot("duck", 1.0, Some(1.0));
        runtime.advance_snapshots(0.5);
        let gain = runtime.route_gain(&AudioRouteId::new("my.project.any.route"));
        assert!(gain < 1.0 && gain > 0.25, "half-transition gain={gain}");
        runtime.advance_snapshots(0.5);
        let gain = runtime.route_gain(&AudioRouteId::new("my.project.any.route"));
        assert!((gain - 10.0_f32.powf(-12.0 / 20.0)).abs() < 1.0e-4);
    }

    #[test]
    fn sample_domain_snapshot_transition_is_exact_at_authored_samples() {
        let mut snapshot = ActiveSnapshot::new(0.0, 0.0);
        snapshot.retarget_samples(1.0, 100, 200);
        snapshot.advance_to_sample(100);
        assert!((snapshot.current - 0.0).abs() < 1.0e-6);
        snapshot.advance_to_sample(200);
        assert!((snapshot.current - 0.5).abs() < 1.0e-6);
        snapshot.advance_to_sample(300);
        assert!((snapshot.current - 1.0).abs() < 1.0e-6);
        assert!(snapshot.sample_transition.is_none());
    }

    #[test]
    fn transport_instance_state_uses_intended_start_not_dispatch_frame() {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime.transport = AudioTransportRuntime::default();
        let _ = runtime.transport.advance_seconds(0.020);
        runtime.instances.insert(
            AudioInstanceId(9),
            RuntimeInstance {
                object_id: AudioObjectId(1),
                voice_ids: vec![1],
                route: AudioRouteId::default(),
                tags: Vec::new(),
                gain: 1.0,
                spatial: false,
                parameters: AudioParameterSet::default(),
                transport_start_sample: 480,
                transport_dispatch_sample: 720,
            },
        );
        let state = runtime.snapshot_state();
        let timing = state
            .transport_instances
            .get(&AudioInstanceId(9))
            .expect("timing");
        assert_eq!(timing.start_sample, 480);
        assert_eq!(timing.dispatch_sample, 720);
        assert_eq!(timing.dispatch_lateness_samples, 240);
        assert_eq!(
            timing.logical_sample,
            runtime.transport.sample().saturating_sub(480)
        );
    }

    #[test]
    fn sample_domain_scalar_transition_uses_existing_rtpc_and_exact_samples() {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime
            .global_parameters
            .set_scalar("project.transport.rtpc", 0.0)
            .unwrap();
        runtime.transition_scalar_samples(
            AudioParameterTarget::Global,
            "project.transport.rtpc".to_owned(),
            1.0,
            100,
            200,
        );
        runtime.advance_scalar_transitions_to_sample(100);
        assert_eq!(
            runtime.global_parameters.scalars["project.transport.rtpc"],
            0.0
        );
        runtime.advance_scalar_transitions_to_sample(200);
        assert!((runtime.global_parameters.scalars["project.transport.rtpc"] - 0.5).abs() < 1.0e-6);
        runtime.advance_scalar_transitions_to_sample(300);
        assert!((runtime.global_parameters.scalars["project.transport.rtpc"] - 1.0).abs() < 1.0e-6);
        assert!(runtime.scalar_transitions.is_empty());
    }

    #[test]
    fn sample_scalar_transition_does_not_invent_missing_parameter_default() {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime.transition_scalar_samples(
            AudioParameterTarget::Global,
            "project.missing".to_owned(),
            1.0,
            0,
            100,
        );
        assert!(runtime.scalar_transitions.is_empty());
        assert!(!runtime
            .global_parameters
            .scalars
            .contains_key("project.missing"));
    }

    #[test]
    fn schedule_stream_allocates_transport_and_instance_identity_without_stream_slots() {
        let handle = AudioOrchestrationHandle::default();
        let mut request = AudioPlayStreamInstanceRequest::new("shared/audio/music/stem.ogg");
        request.route = AudioRouteId::new("project.music.stems");
        request.tags = vec!["project.stem.rhythm".to_owned()];
        request.stream.voice_budget = "project.music".to_owned();
        let (instance_id, action_id) = handle
            .schedule_stream(
                AudioObjectId(7),
                request,
                AudioTransportSchedulePoint::NextBar,
            )
            .expect("schedule stream");
        assert_eq!(instance_id.0, 1);
        assert_eq!(action_id.0, 1);
        let commands = handle.queue.lock().drain();
        assert_eq!(commands.len(), 1);
        match &commands[0] {
            AudioOrchestrationCommand::ScheduleTransport {
                action_id: queued_id,
                when: AudioTransportSchedulePoint::NextBar,
                action:
                    AudioTransportAction::PlayStream {
                        instance_id: queued_instance,
                        object_id,
                        request,
                    },
            } => {
                assert_eq!(*queued_id, action_id);
                assert_eq!(*queued_instance, instance_id);
                assert_eq!(*object_id, AudioObjectId(7));
                assert_eq!(request.route.0, "project.music.stems");
                assert_eq!(request.stream.voice_budget, "project.music");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    fn test_music_graph() -> InteractiveMusicGraph {
        let mut base = AudioPlayStreamInstanceRequest::new("shared/audio/music/base.ogg");
        base.route = AudioRouteId::new("project.music");
        base.stream.voice_budget = "project.music".to_owned();
        base.stream.concurrency_group = "project.music.stems".to_owned();
        base.stream.concurrency_limit = 8;
        base.stream.steal_rule = AudioVoiceStealRule::RejectNew;
        let mut high = base.clone();
        high.stream.clip.uri = "shared/audio/music/high.ogg".to_owned();
        InteractiveMusicGraph {
            id: "project.score".to_owned(),
            initial_state: "calm".to_owned(),
            stems: vec![
                AudioMusicStemSpec {
                    id: "base".to_owned(),
                    request: base,
                },
                AudioMusicStemSpec {
                    id: "high".to_owned(),
                    request: high,
                },
            ],
            states: vec![
                AudioMusicStateSpec {
                    id: "calm".to_owned(),
                    layers: vec![AudioMusicLayerSpec {
                        stem: "base".to_owned(),
                        gain: 1.0,
                    }],
                },
                AudioMusicStateSpec {
                    id: "intense".to_owned(),
                    layers: vec![
                        AudioMusicLayerSpec {
                            stem: "base".to_owned(),
                            gain: 0.6,
                        },
                        AudioMusicLayerSpec {
                            stem: "high".to_owned(),
                            gain: 1.0,
                        },
                    ],
                },
            ],
            transitions: vec![AudioMusicTransitionSpec {
                from: "calm".to_owned(),
                to: "intense".to_owned(),
                quantization: AudioTransportSchedulePoint::NextBar,
                crossfade_samples: 100,
            }],
            selectors: vec![AudioMusicSelectorSpec {
                condition: AudioMusicSelectorCondition::ScalarRange {
                    name: "project.score.intensity".to_owned(),
                    min: 0.5,
                    max: 1.0,
                },
                target_state: "intense".to_owned(),
            }],
            ..Default::default()
        }
    }

    fn music_runtime() -> AudioOrchestrationRuntimeModule {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime.objects.insert(
            AudioObjectId(7),
            RuntimeObject {
                state: AudioObjectState::default(),
            },
        );
        runtime.install_music_graph(test_music_graph());
        runtime
    }

    #[test]
    fn interactive_music_initial_state_uses_transport_stream_and_preserves_voice_policy() {
        let mut runtime = music_runtime();
        runtime.create_music_session(
            AudioMusicSessionId(1),
            "project.score".to_owned(),
            AudioObjectId(7),
        );
        let (_, due) = runtime.transport.advance_samples(1);
        assert_eq!(due.len(), 1);
        match &due[0].action {
            AudioTransportAction::PlayStream {
                request, object_id, ..
            } => {
                assert_eq!(*object_id, AudioObjectId(7));
                assert_eq!(request.stream.clip.uri, "shared/audio/music/base.ogg");
                assert_eq!(request.stream.voice_budget, "project.music");
                assert_eq!(request.stream.concurrency_group, "project.music.stems");
                assert_eq!(request.stream.concurrency_limit, 8);
            }
            other => panic!("unexpected initial music action: {other:?}"),
        }
        runtime.finalize_music_transitions();
        let session = runtime.music_sessions.get(&AudioMusicSessionId(1)).unwrap();
        assert_eq!(session.active_state, "calm");
        assert!(session.pending.is_none());
    }

    #[test]
    fn next_bar_music_transition_reuses_common_stem_and_crossfades_new_stem() {
        let mut runtime = music_runtime();
        runtime.create_music_session(
            AudioMusicSessionId(1),
            "project.score".to_owned(),
            AudioObjectId(7),
        );
        let (_, initial_due) = runtime.transport.advance_samples(1);
        let base_instance = match initial_due[0].action {
            AudioTransportAction::PlayStream { instance_id, .. } => instance_id,
            _ => panic!("initial stem must be stream"),
        };
        runtime.finalize_music_transitions();
        runtime.instances.insert(
            base_instance,
            RuntimeInstance {
                object_id: AudioObjectId(7),
                voice_ids: vec![101],
                route: AudioRouteId::new("project.music"),
                tags: Vec::new(),
                gain: 1.0,
                spatial: false,
                parameters: AudioParameterSet::default(),
                transport_start_sample: 0,
                transport_dispatch_sample: 0,
            },
        );
        runtime.request_music_state(AudioMusicSessionId(1), "intense".to_owned());
        let pending = runtime
            .music_sessions
            .get(&AudioMusicSessionId(1))
            .and_then(|session| session.pending.as_ref())
            .expect("pending transition");
        assert_eq!(pending.start_sample, 96_000);
        assert_eq!(pending.complete_sample, 96_100);
        assert_eq!(
            pending.target_stems["base"], base_instance,
            "common stem identity must survive state change"
        );
        let high_instance = pending.target_stems["high"];
        assert_ne!(high_instance, base_instance);

        let (_, due) = runtime.transport.advance_samples(95_999);
        assert_eq!(runtime.transport.sample(), 96_000);
        assert_eq!(due.len(), 3);
        assert!(due.iter().any(|item| matches!(
            item.action,
            AudioTransportAction::TransitionInstanceGain { instance_id, target_gain, duration_samples: 100 }
                if instance_id == base_instance && (target_gain - 0.6).abs() < 1.0e-6
        )));
        assert!(due.iter().any(|item| matches!(
            &item.action,
            AudioTransportAction::PlayStream { instance_id, request, .. }
                if *instance_id == high_instance && request.gain == 0.0 && request.stream.voice_budget == "project.music"
        )));
        assert!(due.iter().any(|item| matches!(
            item.action,
            AudioTransportAction::TransitionInstanceGain { instance_id, target_gain, duration_samples: 100 }
                if instance_id == high_instance && (target_gain - 1.0).abs() < 1.0e-6
        )));
    }

    #[test]
    fn project_scalar_selector_requests_authored_music_state_without_engine_known_semantics() {
        let mut runtime = music_runtime();
        runtime.create_music_session(
            AudioMusicSessionId(1),
            "project.score".to_owned(),
            AudioObjectId(7),
        );
        let _ = runtime.transport.advance_samples(1);
        runtime.finalize_music_transitions();
        runtime.set_music_scalar(
            AudioMusicSessionId(1),
            "project.score.intensity".to_owned(),
            0.75,
        );
        let session = runtime.music_sessions.get(&AudioMusicSessionId(1)).unwrap();
        assert_eq!(session.parameters.scalars["project.score.intensity"], 0.75);
        assert_eq!(session.pending.as_ref().unwrap().target_state, "intense");
        assert_eq!(
            runtime.objects[&AudioObjectId(7)].state.parameters.scalars["project.score.intensity"],
            0.75
        );
    }

    #[test]
    fn instance_gain_transition_is_sample_domain_exact() {
        let handle = AudioOrchestrationHandle::default();
        let mut runtime = AudioOrchestrationRuntimeModule::new(handle);
        runtime.instances.insert(
            AudioInstanceId(3),
            RuntimeInstance {
                object_id: AudioObjectId(1),
                voice_ids: vec![1],
                route: AudioRouteId::default(),
                tags: Vec::new(),
                gain: 0.0,
                spatial: false,
                parameters: AudioParameterSet::default(),
                transport_start_sample: 0,
                transport_dispatch_sample: 0,
            },
        );
        runtime.transition_instance_gain_samples(AudioInstanceId(3), 1.0, 100, 200);
        runtime.advance_instance_gain_transitions_to_sample(100);
        assert!((runtime.instances[&AudioInstanceId(3)].gain - 0.0).abs() < 1.0e-6);
        runtime.advance_instance_gain_transitions_to_sample(200);
        assert!((runtime.instances[&AudioInstanceId(3)].gain - 0.5).abs() < 1.0e-6);
        runtime.advance_instance_gain_transitions_to_sample(300);
        assert!((runtime.instances[&AudioInstanceId(3)].gain - 1.0).abs() < 1.0e-6);
        assert!(runtime.instance_gain_transitions.is_empty());
    }
}
