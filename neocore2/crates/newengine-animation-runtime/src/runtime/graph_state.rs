#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationGraphEventSource {
    State(usize),
    Layer(usize),
}

/// Semantic graph-level occurrence. `clip_index` and `event_index` address immutable compiled data;
/// no clip/event payload strings are copied in the hot path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationGraphEventOccurrence {
    pub source: AnimationGraphEventSource,
    pub clip_index: usize,
    pub event_index: usize,
    pub playback_time_seconds: f32,
    pub loop_index: u64,
    pub blend_weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationRootMotionDelta {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub valid: bool,
}

impl Default for AnimationRootMotionDelta {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            valid: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationGraphTransitionSnapshot {
    pub from_state: usize,
    pub to_state: usize,
    pub alpha: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimationGraphEvaluation {
    pub local_pose: Vec<JointLocalPose>,
    pub events: Vec<AnimationGraphEventOccurrence>,
    pub root_motion: AnimationRootMotionDelta,
    pub active_state: usize,
    pub transition: Option<AnimationGraphTransitionSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MotionRootSourceMeta {
    sample_index: usize,
    clip_index: usize,
    playback_time_seconds: f32,
    weight: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MotionEvaluationMeta {
    root_sources: [MotionRootSourceMeta; 4],
    root_source_count: usize,
}

impl MotionEvaluationMeta {
    fn add_root_source(
        &mut self,
        sample_index: usize,
        clip_index: usize,
        playback_time_seconds: f32,
        weight: f32,
    ) {
        if weight <= 1.0e-7 {
            return;
        }
        debug_assert!(self.root_source_count < self.root_sources.len());
        if self.root_source_count < self.root_sources.len() {
            self.root_sources[self.root_source_count] = MotionRootSourceMeta {
                sample_index,
                clip_index,
                playback_time_seconds,
                weight,
            };
            self.root_source_count += 1;
        }
    }
}

#[derive(Clone, Debug)]
struct MotionPlaybackRuntime {
    cursors: Vec<AnimationEventCursor>,
}

impl MotionPlaybackRuntime {
    fn new(sample_count: usize) -> Self {
        Self {
            cursors: vec![AnimationEventCursor::default(); sample_count],
        }
    }

    fn restart(&mut self) {
        for cursor in &mut self.cursors {
            cursor.restart();
        }
    }
}

#[derive(Clone, Debug)]
struct StatePlaybackRuntime {
    time_seconds: f32,
    motion: MotionPlaybackRuntime,
}

#[derive(Clone, Debug)]
struct LayerPlaybackRuntime {
    time_seconds: f32,
    motion: MotionPlaybackRuntime,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveTransitionRuntime {
    from_state: usize,
    to_state: usize,
    elapsed_seconds: f32,
    blend_seconds: f32,
    source_is_frozen: bool,
    group_id: Option<usize>,
    interruption: AnimationTransitionInterruptionPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RootMotionRuntimeSource {
    state_index: usize,
    motion_time_seconds: f32,
}

/// Mutable per-actor graph state. The compiled graph remains shareable and immutable.
#[derive(Clone, Debug)]
pub struct AnimationGraphInstance {
    parameters: Vec<AnimationGraphParameterValue>,
    active_state: usize,
    states: Vec<StatePlaybackRuntime>,
    layers: Vec<LayerPlaybackRuntime>,
    transition: Option<ActiveTransitionRuntime>,
    root_motion_source: Option<RootMotionRuntimeSource>,
    frozen_transition_pose: Vec<JointLocalPose>,
    last_base_pose: Vec<JointLocalPose>,
    scratch_a: Vec<JointLocalPose>,
    scratch_b: Vec<JointLocalPose>,
    scratch_layer: Vec<JointLocalPose>,
    event_scratch: Vec<AnimationEventOccurrence>,
}

impl AnimationGraphInstance {
    pub fn new(graph: &CompiledAnimationGraph) -> Self {
        let mut states = graph
            .states
            .iter()
            .map(|state| StatePlaybackRuntime {
                time_seconds: 0.0,
                motion: MotionPlaybackRuntime::new(state.motion.sample_count()),
            })
            .collect::<Vec<_>>();
        states[graph.entry_state].motion.restart();
        let mut layers = graph
            .layers
            .iter()
            .map(|layer| LayerPlaybackRuntime {
                time_seconds: 0.0,
                motion: MotionPlaybackRuntime::new(layer.motion.sample_count()),
            })
            .collect::<Vec<_>>();
        for layer in &mut layers {
            layer.motion.restart();
        }
        Self {
            parameters: graph.parameters.iter().map(|parameter| parameter.default).collect(),
            active_state: graph.entry_state,
            states,
            layers,
            transition: None,
            root_motion_source: None,
            frozen_transition_pose: Vec::new(),
            last_base_pose: Vec::new(),
            scratch_a: Vec::new(),
            scratch_b: Vec::new(),
            scratch_layer: Vec::new(),
            event_scratch: Vec::new(),
        }
    }

    #[inline]
    pub fn active_state_index(&self) -> usize {
        self.active_state
    }

    #[inline]
    pub fn transition(&self) -> Option<AnimationGraphTransitionSnapshot> {
        self.transition.map(|transition| AnimationGraphTransitionSnapshot {
            from_state: transition.from_state,
            to_state: transition.to_state,
            alpha: transition_alpha(transition),
        })
    }

    pub fn reset(&mut self, graph: &CompiledAnimationGraph) {
        *self = Self::new(graph);
    }

    pub fn set_float(
        &mut self,
        graph: &CompiledAnimationGraph,
        name: &str,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err(format!(
                "animation graph '{}' float parameter '{}' is non-finite",
                graph.name, name
            ));
        }
        let index = graph.parameter_index(name).ok_or_else(|| {
            format!(
                "animation graph '{}' has no parameter '{}'",
                graph.name, name
            )
        })?;
        match self.parameters.get_mut(index) {
            Some(AnimationGraphParameterValue::Float(current)) => {
                *current = value;
                Ok(())
            }
            Some(AnimationGraphParameterValue::Bool(_)) => Err(format!(
                "animation graph '{}' parameter '{}' is not float",
                graph.name, name
            )),
            None => Err("animation graph parameter/runtime shape mismatch".to_owned()),
        }
    }

    pub fn set_bool(
        &mut self,
        graph: &CompiledAnimationGraph,
        name: &str,
        value: bool,
    ) -> Result<(), String> {
        let index = graph.parameter_index(name).ok_or_else(|| {
            format!(
                "animation graph '{}' has no parameter '{}'",
                graph.name, name
            )
        })?;
        match self.parameters.get_mut(index) {
            Some(AnimationGraphParameterValue::Bool(current)) => {
                *current = value;
                Ok(())
            }
            Some(AnimationGraphParameterValue::Float(_)) => Err(format!(
                "animation graph '{}' parameter '{}' is not bool",
                graph.name, name
            )),
            None => Err("animation graph parameter/runtime shape mismatch".to_owned()),
        }
    }

    #[inline]
    pub fn parameter(
        &self,
        graph: &CompiledAnimationGraph,
        name: &str,
    ) -> Option<AnimationGraphParameterValue> {
        graph
            .parameter_index(name)
            .and_then(|index| self.parameters.get(index).copied())
    }
}

fn transition_alpha(transition: ActiveTransitionRuntime) -> f32 {
    if transition.blend_seconds <= 1.0e-8 {
        1.0
    } else {
        (transition.elapsed_seconds / transition.blend_seconds).clamp(0.0, 1.0)
    }
}

fn motion_reference_duration(graph: &CompiledAnimationGraph, motion: &CompiledAnimationMotion) -> f32 {
    graph.clips[motion.first_clip_index()]
        .clip
        .duration_seconds
        .max(1.0e-6)
}

fn motion_unwrapped_phase(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    state_time_seconds: f32,
) -> f32 {
    let duration = motion_reference_duration(graph, motion);
    state_time_seconds * motion.first_speed() / duration
}

fn motion_normalized_phase(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    state_time_seconds: f32,
) -> f32 {
    motion_unwrapped_phase(graph, motion, state_time_seconds).rem_euclid(1.0)
}

fn condition_matches(
    condition: CompiledTransitionCondition,
    parameters: &[AnimationGraphParameterValue],
) -> bool {
    match condition {
        CompiledTransitionCondition::Bool {
            parameter_index,
            equals,
        } => parameters
            .get(parameter_index)
            .and_then(|value| value.as_bool())
            == Some(equals),
        CompiledTransitionCondition::Float {
            parameter_index,
            comparison,
            value,
        } => parameters
            .get(parameter_index)
            .and_then(|parameter| parameter.as_float())
            .is_some_and(|parameter| match comparison {
                AnimationFloatComparison::Less => parameter < value,
                AnimationFloatComparison::LessOrEqual => parameter <= value,
                AnimationFloatComparison::Greater => parameter > value,
                AnimationFloatComparison::GreaterOrEqual => parameter >= value,
            }),
    }
}

fn transition_is_ready(
    graph: &CompiledAnimationGraph,
    transition: &CompiledAnimationGraphTransition,
    parameters: &[AnimationGraphParameterValue],
    source_time_seconds: f32,
) -> bool {
    let source = &graph.states[transition.from_state];
    if let Some(exit_time) = transition.exit_time_normalized {
        let phase = motion_normalized_phase(graph, &source.motion, source_time_seconds);
        if phase + 1.0e-6 < exit_time {
            return false;
        }
    }
    transition
        .conditions
        .iter()
        .copied()
        .all(|condition| condition_matches(condition, parameters))
}

fn layer_effective_weight(
    layer: &CompiledAnimationGraphLayer,
    parameters: &[AnimationGraphParameterValue],
) -> f32 {
    let parameter = layer
        .weight_parameter_index
        .and_then(|index| parameters.get(index))
        .and_then(|value| value.as_float())
        .unwrap_or(1.0);
    (layer.weight * parameter).clamp(0.0, 1.0)
}

/// Converts graph-authored declarations into a deterministic map useful to tools/debuggers without
/// exposing the evaluator's internal indices.
pub fn animation_graph_state_map(graph: &CompiledAnimationGraph) -> BTreeMap<String, usize> {
    graph
        .states
        .iter()
        .enumerate()
        .map(|(index, state)| (state.name.clone(), index))
        .collect()
}
