use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

use newengine_audio_api::{
    AudioInstanceId, AudioMixGraph, AudioObjectId, AudioObjectState, AudioOrchestrationCommand,
    AudioParameterSet, AudioParameterTarget, AudioPlayInstanceRequest, AudioRouteId,
    AudioVoiceBudgetConfig, AudioVoiceUpdateRequest,
};
use newengine_audio_client::{
    play_audio_cue, set_audio_voice_budgets, stop_audio_voice, update_audio_voice,
};
use newengine_core::{EngineResult, Module, ModuleCtx};

const DEFAULT_COMMAND_CAPACITY: usize = 2_048;

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
}

#[derive(Clone, Copy, Debug)]
struct ActiveSnapshot {
    current: f32,
    target: f32,
    remaining_seconds: f32,
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
        }
    }

    fn retarget(&mut self, target: f32, transition_seconds: f32) {
        self.target = sanitize_weight(target);
        self.remaining_seconds = sanitize_transition(transition_seconds);
        if self.remaining_seconds <= 0.0 {
            self.current = self.target;
        }
    }

    fn advance(&mut self, dt: f32) {
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
}

pub struct AudioOrchestrationRuntimeModule {
    handle: AudioOrchestrationHandle,
    objects: HashMap<AudioObjectId, RuntimeObject>,
    instances: HashMap<AudioInstanceId, RuntimeInstance>,
    global_parameters: AudioParameterSet,
    mix_graph: Option<AudioMixGraph>,
    snapshots: BTreeMap<String, ActiveSnapshot>,
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
        }
    }

    fn drain_commands(&self) -> Vec<AudioOrchestrationCommand> {
        self.handle.queue.lock().drain()
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
                self.stop_object_instances(object_id);
                self.objects.remove(&object_id);
            }
            AudioOrchestrationCommand::UpdateObject { object_id, state } => {
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
            } => self.play_instance(instance_id, object_id, request),
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
        }
    }

    fn play_instance(
        &mut self,
        instance_id: AudioInstanceId,
        object_id: AudioObjectId,
        request: AudioPlayInstanceRequest,
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

    fn stop_voice_ids(voice_ids: &[u64]) {
        for voice_id in voice_ids {
            let _ = stop_audio_voice(*voice_id);
        }
    }

    fn stop_instance(&mut self, instance_id: AudioInstanceId) {
        if let Some(instance) = self.instances.remove(&instance_id) {
            Self::stop_voice_ids(&instance.voice_ids);
        }
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

    fn set_scalar(&mut self, target: AudioParameterTarget, name: String, value: f32) {
        let result = match target {
            AudioParameterTarget::Global => self.global_parameters.set_scalar(name, value),
            AudioParameterTarget::Object(object_id) => self
                .objects
                .get_mut(&object_id)
                .ok_or_else(|| format!("unknown audio object {}", object_id.0))
                .and_then(|object| object.state.parameters.set_scalar(name, value)),
            AudioParameterTarget::Instance(instance_id) => self
                .instances
                .get_mut(&instance_id)
                .ok_or_else(|| format!("unknown audio instance {}", instance_id.0))
                .and_then(|instance| instance.parameters.set_scalar(name, value)),
        };
        if let Err(error) = result {
            newengine_ulog_api::ulog::warn!("audio orchestration: scalar rejected err='{}'", error);
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

    fn advance_snapshots(&mut self, dt: f32) {
        for snapshot in self.snapshots.values_mut() {
            snapshot.advance(dt);
        }
        self.snapshots.retain(|_, snapshot| {
            snapshot.current > 1.0e-5
                || snapshot.target > 1.0e-5
                || snapshot.remaining_seconds > 0.0
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
            self.instances.remove(&instance_id);
        }
    }

    fn snapshot_state(&self) -> AudioOrchestrationRuntimeState {
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
    }
}

impl Module<()> for AudioOrchestrationRuntimeModule {
    fn id(&self) -> &'static str {
        "engine.audio.orchestration.runtime"
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        ctx.resources_mut().insert(self.handle.clone());
        ctx.resources_mut().insert(self.snapshot_state());
        Ok(())
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        for command in self.drain_commands() {
            self.apply_command(command);
        }
        let dt = ctx.frame().map(|frame| frame.dt).unwrap_or(1.0 / 60.0);
        self.advance_snapshots(dt);
        self.sync_instances();
        ctx.resources_mut().insert(self.snapshot_state());
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        self.stop_all();
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
    use newengine_audio_api::{AudioMixBusSpec, AudioMixPatch, AudioMixSnapshotSpec};

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
}
