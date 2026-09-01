use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use newengine_audio_api::{
    AudioInstanceId, AudioMixGraph, AudioMusicSessionId, AudioMusicSessionState, AudioObjectId,
    AudioObjectState, AudioOrchestrationCommand, AudioParameterSet, AudioParameterTarget,
    AudioPlayInstanceRequest, AudioPlayStreamInstanceRequest, AudioRenderClock,
    AudioRouteGainRequest, AudioRouteId, AudioTransportAction, AudioTransportActionId,
    AudioTransportConfig, AudioTransportInstanceState, AudioTransportMarkerOccurrence,
    AudioTransportRuntimeState, AudioTransportSchedulePoint, AudioVoiceBudgetConfig,
    AudioVoiceRenderAction, AudioVoiceRenderScheduleRequest, AudioVoiceUpdateRequest,
    InteractiveMusicGraph, InteractiveMusicRuntimeState,
};
use newengine_audio_client::{
    audio_render_clock, play_audio_cue, play_audio_stream, schedule_audio_voice_render,
    set_audio_route_gain, set_audio_voice_budgets, stop_audio_voice, update_audio_voice,
};
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::audio_transport::{
    AudioTransportHandle, AudioTransportRuntime, DueTransportAction, PendingTransportAction,
};
use crate::interactive_music::InteractiveMusicHandle;

mod config;
mod handle;
mod music;
mod transition;

include!("audio_orchestration/transport_runtime.rs");
include!("audio_orchestration/mix_runtime.rs");

pub use config::AudioOrchestrationRuntimeConfig;
pub use handle::AudioOrchestrationHandle;
use transition::SampleTransition;

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
    render_armed: bool,
}

#[derive(Clone, Copy, Debug)]
struct ProviderClockAnchor {
    transport_sample: u64,
    provider_sample: u64,
    provider_rate: u32,
}

#[derive(Clone, Debug)]
enum PrearmedTransportAction {
    Play {
        instance_id: AudioInstanceId,
    },
    PlayStream {
        instance_id: AudioInstanceId,
    },
    Gain {
        instance_id: AudioInstanceId,
        voice_ids: Vec<u64>,
        schedule_id: u64,
        end_transport_sample: u64,
    },
    Stop {
        instance_id: AudioInstanceId,
        voice_ids: Vec<u64>,
        schedule_id: u64,
    },
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
    sample_transition: Option<SampleTransition>,
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
        self.sample_transition = Some(SampleTransition::new(
            self.current,
            target,
            start_sample,
            duration_samples,
        ));
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
        let (value, finished) = transition.evaluate(sample);
        self.current = value;
        if finished {
            self.target = transition.target;
            self.sample_transition = None;
        }
    }
}

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
                self.play_instance(instance_id, object_id, request, sample, sample, None)
            }
            AudioOrchestrationCommand::PlayStream {
                instance_id,
                object_id,
                request,
            } => {
                let sample = self.transport.sample();
                self.play_stream_instance(instance_id, object_id, request, sample, sample, None)
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

    fn play_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
        transport_start_sample: u64,
        transport_dispatch_sample: u64,
        render_start_sample: Option<u64>,
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

        let mut play =
            newengine_audio_api::AudioCuePlayRequest::new(request.cue.logical_path.clone());
        play.route = request.route.clone();
        play.position = request.spatial.then_some(object.state.position);
        play.gain = object.state.gain * request.gain;
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
        play.render_start_sample = render_start_sample;

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
                        render_armed: render_start_sample.is_some(),
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
        render_start_sample: Option<u64>,
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

        let mut stream = request.stream.clone();
        stream.route = request.route.clone();
        let instance_gain = newengine_audio_api::sanitize_gain(request.gain * stream.gain);
        stream.gain = newengine_audio_api::sanitize_gain(object.state.gain * instance_gain);
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
        stream.render_start_sample = render_start_sample;
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
                        render_armed: render_start_sample.is_some(),
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
        self.provider_gain_ramp_until.clear();
        self.prearmed_transport_actions.clear();
        self.provider_clock = None;
        self.provider_clock_anchor = None;
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
        self.refresh_provider_clock();
        self.process_commands();
        if self.provider_clock.is_none() && self.transport.has_pending_actions() {
            self.refresh_provider_clock();
        }
        self.prearm_pending_transport_actions();
        let dt = ctx.frame().map(|frame| frame.dt).unwrap_or(1.0 / 60.0);
        let (markers, due_actions) = self.advance_transport_clock(dt);
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
mod tests;
