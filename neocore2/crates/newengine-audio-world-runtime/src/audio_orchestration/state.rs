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
