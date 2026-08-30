use std::collections::BTreeMap;

/// Authoring-time graph parameter value. Runtime instances keep a dense copy of these values.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationGraphParameterValue {
    Float(f32),
    Bool(bool),
}

impl AnimationGraphParameterValue {
    #[inline]
    pub fn as_float(self) -> Option<f32> {
        match self {
            Self::Float(value) => Some(value),
            Self::Bool(_) => None,
        }
    }

    #[inline]
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(value),
            Self::Float(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphParameterDefinition {
    pub name: String,
    pub default: AnimationGraphParameterValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationFloatComparison {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationTransitionCondition {
    Bool {
        parameter: String,
        equals: bool,
    },
    Float {
        parameter: String,
        comparison: AnimationFloatComparison,
        value: f32,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationClipMotionDefinition {
    pub clip_ref: String,
    pub speed: f32,
    pub sync_group: Option<String>,
}

impl AnimationClipMotionDefinition {
    #[inline]
    pub fn new(clip_ref: impl Into<String>) -> Self {
        Self {
            clip_ref: clip_ref.into(),
            speed: 1.0,
            sync_group: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationBlendSample1D {
    pub threshold: f32,
    pub clip_ref: String,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationBlendTree1DDefinition {
    pub parameter: String,
    pub samples: Vec<AnimationBlendSample1D>,
    pub sync_group: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationBlend2DMode {
    /// Complete rectangular lattice evaluated with bilinear weights.
    #[default]
    Cartesian,
    /// Polar/angular blend. A sample at the origin is treated as an optional radial center.
    Directional,
    /// Arbitrary scattered domain compiled into a deterministic Delaunay triangulation.
    Triangulated,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationBlendSample2D {
    pub position: [f32; 2],
    pub clip_ref: String,
    pub speed: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationBlendTree2DDefinition {
    pub parameter_x: String,
    pub parameter_y: String,
    pub mode: AnimationBlend2DMode,
    pub samples: Vec<AnimationBlendSample2D>,
    pub sync_group: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationMotionDefinition {
    Clip(AnimationClipMotionDefinition),
    Blend1D(AnimationBlendTree1DDefinition),
    Blend2D(AnimationBlendTree2DDefinition),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationRootMotionMode {
    #[default]
    Disabled,
    Extract,
    ExtractAndRemove,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphStateDefinition {
    pub name: String,
    pub motion: AnimationMotionDefinition,
    pub speed: f32,
    pub root_motion: AnimationRootMotionMode,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AnimationTransitionInterruptionPolicy {
    /// The authored transition must complete before automatic transitions from its destination
    /// state may take ownership. Explicit `BlendToState` requests remain authoritative.
    #[default]
    Never,
    /// Automatic interruption is allowed only by a ready transition in the same authored group.
    SameGroup,
    /// Any ready transition from the active destination state may interrupt this transition.
    Any,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphTransitionDefinition {
    pub from: String,
    pub to: String,
    pub conditions: Vec<AnimationTransitionCondition>,
    /// Optional normalized source-state exit threshold. `0.75` means the transition cannot begin
    /// before the source motion reaches 75% of its reference cycle.
    pub exit_time_normalized: Option<f32>,
    pub blend_seconds: f32,
    /// Higher priorities win. Equal priorities are resolved by authored declaration order.
    #[serde(default)]
    pub priority: i32,
    /// Optional case-insensitive arbitration group used by `SameGroup` interruption.
    #[serde(default)]
    pub group: Option<String>,
    /// Controls whether this transition may be interrupted automatically while it is active.
    #[serde(default)]
    pub interruption: AnimationTransitionInterruptionPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationLayerBlendMode {
    #[default]
    Override,
    Additive,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationBoneMaskRoot {
    pub joint_tag: u32,
    pub weight: f32,
    pub include_descendants: bool,
}

#[derive(Clone, Debug, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct AnimationBoneMaskDefinition {
    pub roots: Vec<AnimationBoneMaskRoot>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphLayerDefinition {
    pub name: String,
    pub motion: AnimationMotionDefinition,
    pub mode: AnimationLayerBlendMode,
    pub weight: f32,
    /// Optional float graph parameter multiplied into `weight`.
    pub weight_parameter: Option<String>,
    pub mask: Option<AnimationBoneMaskDefinition>,
    /// Timeline events from this layer are emitted only at or above this effective weight.
    pub event_weight_threshold: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationSyncGroupDefinition {
    pub name: String,
    /// Ordered cyclic semantic marker vocabulary. Concrete marker times come from matching
    /// `AnimationClip::events` tags on every motion participating in this group.
    pub markers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnimationGraphDefinition {
    pub name: String,
    pub entry_state: String,
    pub parameters: Vec<AnimationGraphParameterDefinition>,
    pub states: Vec<AnimationGraphStateDefinition>,
    pub transitions: Vec<AnimationGraphTransitionDefinition>,
    pub layers: Vec<AnimationGraphLayerDefinition>,
    /// Optional marker-aware synchronization contracts. Motions may still use `sync_group`
    /// without a declaration here; that preserves the V1 normalized-phase behavior.
    #[serde(default)]
    pub sync_groups: Vec<AnimationSyncGroupDefinition>,
    /// Skeleton joint tag used for root-motion extraction. Required only when any state extracts
    /// root motion.
    pub root_motion_joint_tag: Option<u32>,
}

#[derive(Clone, Debug)]
struct CompiledAnimationGraphClip {
    reference: String,
    clip: Arc<AnimationClip>,
    binding: AnimationClipBinding,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledBlendSample1D {
    threshold: f32,
    clip_index: usize,
    speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledBlendSample2D {
    position: [f32; 2],
    clip_index: usize,
    speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompiledBlendTriangle {
    samples: [usize; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledDirectionalSample {
    sample_index: usize,
    angle_radians: f32,
    radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum CompiledBlend2DDomain {
    Cartesian {
        x_values: Vec<f32>,
        y_values: Vec<f32>,
        /// Row-major `y * x_values.len() + x` -> sample index.
        grid: Vec<usize>,
    },
    Directional {
        center_sample: Option<usize>,
        ring: Vec<CompiledDirectionalSample>,
    },
    Triangulated {
        triangles: Vec<CompiledBlendTriangle>,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledAnimationSyncGroup {
    name: String,
    marker_tags: Vec<String>,
    marker_index: HashMap<String, usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CompiledSyncMarker {
    marker_index: usize,
    time_seconds: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledSyncMarkerTrack {
    markers: Vec<CompiledSyncMarker>,
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledMotionSync {
    key: String,
    marker_group_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
enum CompiledAnimationMotion {
    Clip {
        clip_index: usize,
        speed: f32,
        sync: Option<CompiledMotionSync>,
    },
    Blend1D {
        parameter_index: usize,
        samples: Vec<CompiledBlendSample1D>,
        sync: Option<CompiledMotionSync>,
    },
    Blend2D {
        parameter_x_index: usize,
        parameter_y_index: usize,
        samples: Vec<CompiledBlendSample2D>,
        domain: CompiledBlend2DDomain,
        sync: Option<CompiledMotionSync>,
    },
}

impl CompiledAnimationMotion {
    fn sample_count(&self) -> usize {
        match self {
            Self::Clip { .. } => 1,
            Self::Blend1D { samples, .. } => samples.len(),
            Self::Blend2D { samples, .. } => samples.len(),
        }
    }

    fn sync(&self) -> Option<&CompiledMotionSync> {
        match self {
            Self::Clip { sync, .. }
            | Self::Blend1D { sync, .. }
            | Self::Blend2D { sync, .. } => sync.as_ref(),
        }
    }

    #[inline]
    fn marker_sync_group_index(&self) -> Option<usize> {
        self.sync().and_then(|sync| sync.marker_group_index)
    }

    fn clip_indices(&self) -> Vec<usize> {
        match self {
            Self::Clip { clip_index, .. } => vec![*clip_index],
            Self::Blend1D { samples, .. } => {
                samples.iter().map(|sample| sample.clip_index).collect()
            }
            Self::Blend2D { samples, .. } => {
                samples.iter().map(|sample| sample.clip_index).collect()
            }
        }
    }

    fn first_clip_index(&self) -> usize {
        match self {
            Self::Clip { clip_index, .. } => *clip_index,
            Self::Blend1D { samples, .. } => samples[0].clip_index,
            Self::Blend2D { samples, .. } => samples[0].clip_index,
        }
    }

    fn first_speed(&self) -> f32 {
        match self {
            Self::Clip { speed, .. } => *speed,
            Self::Blend1D { samples, .. } => samples[0].speed,
            Self::Blend2D { samples, .. } => samples[0].speed,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledAnimationGraphState {
    name: String,
    motion: CompiledAnimationMotion,
    speed: f32,
    root_motion: AnimationRootMotionMode,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CompiledTransitionCondition {
    Bool {
        parameter_index: usize,
        equals: bool,
    },
    Float {
        parameter_index: usize,
        comparison: AnimationFloatComparison,
        value: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledAnimationGraphTransition {
    from_state: usize,
    to_state: usize,
    conditions: Vec<CompiledTransitionCondition>,
    exit_time_normalized: Option<f32>,
    blend_seconds: f32,
    priority: i32,
    authored_order: usize,
    group_id: Option<usize>,
    interruption: AnimationTransitionInterruptionPolicy,
}

#[derive(Clone, Debug, PartialEq)]
struct CompiledAnimationGraphLayer {
    name: String,
    motion: CompiledAnimationMotion,
    mode: AnimationLayerBlendMode,
    weight: f32,
    weight_parameter_index: Option<usize>,
    mask: Vec<f32>,
    event_weight_threshold: f32,
}

/// Immutable graph evaluation plan. All clip references, skeleton bindings, parameter addresses,
/// state addresses, transition conditions and bone masks are resolved at compile time.
#[derive(Clone, Debug)]
pub struct CompiledAnimationGraph {
    name: String,
    entry_state: usize,
    parameters: Vec<AnimationGraphParameterDefinition>,
    parameter_index: HashMap<String, usize>,
    states: Vec<CompiledAnimationGraphState>,
    state_index: HashMap<String, usize>,
    transitions_by_state: Vec<Vec<CompiledAnimationGraphTransition>>,
    layers: Vec<CompiledAnimationGraphLayer>,
    clips: Vec<CompiledAnimationGraphClip>,
    sync_marker_tracks: Vec<Vec<Option<CompiledSyncMarkerTrack>>>,
    root_motion_joint_index: Option<usize>,
}
