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

#[inline]
fn canonical_graph_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_speed(value: f32, label: &str) -> Result<f32, String> {
    if !value.is_finite() || value <= 0.0 || value > 100.0 {
        return Err(format!(
            "animation graph {label} speed must be finite in (0,100] speed={value}"
        ));
    }
    Ok(value)
}

fn resolve_float_graph_parameter(
    graph_name: &str,
    node_label: &str,
    parameter_name: &str,
    parameter_index: &HashMap<String, usize>,
    parameters: &[AnimationGraphParameterDefinition],
) -> Result<usize, String> {
    let index = parameter_index
        .get(&canonical_graph_key(parameter_name))
        .copied()
        .ok_or_else(|| {
            format!(
                "animation graph '{graph_name}' {node_label} references unknown parameter '{parameter_name}'"
            )
        })?;
    if !matches!(parameters[index].default, AnimationGraphParameterValue::Float(_)) {
        return Err(format!(
            "animation graph '{graph_name}' {node_label} parameter '{parameter_name}' is not float"
        ));
    }
    Ok(index)
}

fn canonical_sync_group(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
}

fn compile_motion_sync(
    value: &Option<String>,
    sync_group_index: &HashMap<String, usize>,
) -> Option<CompiledMotionSync> {
    canonical_sync_group(value).map(|key| CompiledMotionSync {
        marker_group_index: sync_group_index.get(&key).copied(),
        key,
    })
}

const MAX_BLEND2D_SAMPLES: usize = 128;
const BLEND2D_POINT_EPSILON: f32 = 1.0e-5;

fn blend2d_position_distance_squared(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn validate_blend2d_positions(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<(), String> {
    if samples.len() < 2 {
        return Err(format!(
            "animation graph '{graph_name}' blend2d requires at least two samples"
        ));
    }
    if samples.len() > MAX_BLEND2D_SAMPLES {
        return Err(format!(
            "animation graph '{graph_name}' blend2d exceeds sample budget samples={} limit={MAX_BLEND2D_SAMPLES}",
            samples.len()
        ));
    }
    let epsilon2 = BLEND2D_POINT_EPSILON * BLEND2D_POINT_EPSILON;
    for left in 0..samples.len() {
        for right in left + 1..samples.len() {
            if blend2d_position_distance_squared(samples[left].position, samples[right].position)
                <= epsilon2
            {
                return Err(format!(
                    "animation graph '{graph_name}' blend2d contains duplicate sample position [{}, {}]",
                    samples[right].position[0], samples[right].position[1]
                ));
            }
        }
    }
    Ok(())
}

fn compile_cartesian_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    let mut x_values = samples.iter().map(|sample| sample.position[0]).collect::<Vec<_>>();
    let mut y_values = samples.iter().map(|sample| sample.position[1]).collect::<Vec<_>>();
    x_values.sort_by(f32::total_cmp);
    y_values.sort_by(f32::total_cmp);
    x_values.dedup_by(|a, b| *a == *b);
    y_values.dedup_by(|a, b| *a == *b);
    if x_values.len() < 2 || y_values.len() < 2 {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d requires at least two unique values on each axis"
        ));
    }
    let expected = x_values
        .len()
        .checked_mul(y_values.len())
        .ok_or_else(|| format!("animation graph '{graph_name}' cartesian blend2d grid overflow"))?;
    if expected != samples.len() {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d requires a complete rectangular lattice samples={} expected={} x={} y={}",
            samples.len(), expected, x_values.len(), y_values.len()
        ));
    }
    let mut grid = vec![usize::MAX; expected];
    for (sample_index, sample) in samples.iter().enumerate() {
        let x = x_values
            .binary_search_by(|value| value.total_cmp(&sample.position[0]))
            .map_err(|_| format!("animation graph '{graph_name}' cartesian blend2d x lookup failed"))?;
        let y = y_values
            .binary_search_by(|value| value.total_cmp(&sample.position[1]))
            .map_err(|_| format!("animation graph '{graph_name}' cartesian blend2d y lookup failed"))?;
        let cell = y * x_values.len() + x;
        if grid[cell] != usize::MAX {
            return Err(format!(
                "animation graph '{graph_name}' cartesian blend2d contains duplicate lattice cell"
            ));
        }
        grid[cell] = sample_index;
    }
    if grid.contains(&usize::MAX) {
        return Err(format!(
            "animation graph '{graph_name}' cartesian blend2d lattice contains a missing cell"
        ));
    }
    Ok(CompiledBlend2DDomain::Cartesian {
        x_values,
        y_values,
        grid,
    })
}

fn normalize_angle_radians(value: f32) -> f32 {
    value.rem_euclid(std::f32::consts::TAU)
}

fn compile_directional_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    let mut center_sample = None;
    let mut ring = Vec::with_capacity(samples.len());
    for (sample_index, sample) in samples.iter().enumerate() {
        let radius = sample.position[0].hypot(sample.position[1]);
        if radius <= BLEND2D_POINT_EPSILON {
            if center_sample.replace(sample_index).is_some() {
                return Err(format!(
                    "animation graph '{graph_name}' directional blend2d contains multiple center samples"
                ));
            }
            continue;
        }
        ring.push(CompiledDirectionalSample {
            sample_index,
            angle_radians: normalize_angle_radians(sample.position[1].atan2(sample.position[0])),
            radius,
        });
    }
    if ring.len() < 3 {
        return Err(format!(
            "animation graph '{graph_name}' directional blend2d requires at least three non-center directions"
        ));
    }
    ring.sort_by(|a, b| {
        a.angle_radians
            .total_cmp(&b.angle_radians)
            .then_with(|| a.sample_index.cmp(&b.sample_index))
    });
    for index in 0..ring.len() {
        let a = ring[index].angle_radians;
        let b = if index + 1 == ring.len() {
            ring[0].angle_radians + std::f32::consts::TAU
        } else {
            ring[index + 1].angle_radians
        };
        if b - a <= 1.0e-4 {
            return Err(format!(
                "animation graph '{graph_name}' directional blend2d contains duplicate direction angle"
            ));
        }
    }
    Ok(CompiledBlend2DDomain::Directional {
        center_sample,
        ring,
    })
}

#[derive(Clone, Copy, Debug)]
struct Blend2DTriangleWork {
    vertices: [usize; 3],
}

fn orient2d(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn ccw_triangle(
    a: usize,
    b: usize,
    c: usize,
    points: &[[f64; 2]],
) -> Option<Blend2DTriangleWork> {
    let orientation = orient2d(points[a], points[b], points[c]);
    if orientation.abs() <= 1.0e-12 {
        return None;
    }
    Some(if orientation > 0.0 {
        Blend2DTriangleWork { vertices: [a, b, c] }
    } else {
        Blend2DTriangleWork { vertices: [a, c, b] }
    })
}

fn circumcircle_contains(
    triangle: Blend2DTriangleWork,
    point_index: usize,
    points: &[[f64; 2]],
) -> bool {
    let [ia, ib, ic] = triangle.vertices;
    let p = points[point_index];
    let a = [points[ia][0] - p[0], points[ia][1] - p[1]];
    let b = [points[ib][0] - p[0], points[ib][1] - p[1]];
    let c = [points[ic][0] - p[0], points[ic][1] - p[1]];
    let aa = a[0] * a[0] + a[1] * a[1];
    let bb = b[0] * b[0] + b[1] * b[1];
    let cc = c[0] * c[0] + c[1] * c[1];
    let determinant = aa * (b[0] * c[1] - b[1] * c[0])
        - bb * (a[0] * c[1] - a[1] * c[0])
        + cc * (a[0] * b[1] - a[1] * b[0]);
    determinant > 1.0e-10
}

fn compile_triangulated_blend2d_domain(
    graph_name: &str,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    if samples.len() < 3 {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d requires at least three samples"
        ));
    }
    let mut points = samples
        .iter()
        .map(|sample| [f64::from(sample.position[0]), f64::from(sample.position[1])])
        .collect::<Vec<_>>();
    let min_x = points.iter().map(|point| point[0]).fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|point| point[1]).fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (max_x - min_x).max(max_y - min_y);
    if span <= 1.0e-10 {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d domain is degenerate"
        ));
    }
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;
    let super_start = points.len();
    points.push([mid_x - span * 32.0, mid_y - span * 2.0]);
    points.push([mid_x, mid_y + span * 32.0]);
    points.push([mid_x + span * 32.0, mid_y - span * 2.0]);
    let super_triangle = ccw_triangle(super_start, super_start + 1, super_start + 2, &points)
        .ok_or_else(|| format!("animation graph '{graph_name}' blend2d super triangle failed"))?;
    let mut triangles = vec![super_triangle];
    let mut insertion_order = (0..samples.len()).collect::<Vec<_>>();
    insertion_order.sort_by(|left, right| {
        samples[*left].position[0]
            .total_cmp(&samples[*right].position[0])
            .then_with(|| samples[*left].position[1].total_cmp(&samples[*right].position[1]))
            .then_with(|| left.cmp(right))
    });
    for point_index in insertion_order {
        let mut edge_counts = HashMap::<(usize, usize), usize>::new();
        let mut bad = vec![false; triangles.len()];
        for (triangle_index, triangle) in triangles.iter().copied().enumerate() {
            if !circumcircle_contains(triangle, point_index, &points) {
                continue;
            }
            bad[triangle_index] = true;
            let [a, b, c] = triangle.vertices;
            for (left, right) in [(a, b), (b, c), (c, a)] {
                let edge = if left < right { (left, right) } else { (right, left) };
                *edge_counts.entry(edge).or_default() += 1;
            }
        }
        let mut next_triangles = triangles
            .into_iter()
            .enumerate()
            .filter_map(|(index, triangle)| (!bad[index]).then_some(triangle))
            .collect::<Vec<_>>();
        let mut boundary = edge_counts
            .into_iter()
            .filter_map(|(edge, count)| (count == 1).then_some(edge))
            .collect::<Vec<_>>();
        boundary.sort_unstable();
        for (left, right) in boundary {
            if let Some(triangle) = ccw_triangle(left, right, point_index, &points) {
                next_triangles.push(triangle);
            }
        }
        triangles = next_triangles;
    }
    let mut compiled = triangles
        .into_iter()
        .filter(|triangle| triangle.vertices.iter().all(|vertex| *vertex < super_start))
        .map(|triangle| CompiledBlendTriangle {
            samples: triangle.vertices,
        })
        .collect::<Vec<_>>();
    if compiled.is_empty() {
        return Err(format!(
            "animation graph '{graph_name}' triangulated blend2d samples are collinear"
        ));
    }
    compiled.sort_by_key(|triangle| {
        let mut key = triangle.samples;
        key.sort_unstable();
        key
    });
    compiled.dedup_by(|left, right| {
        let mut left_key = left.samples;
        let mut right_key = right.samples;
        left_key.sort_unstable();
        right_key.sort_unstable();
        left_key == right_key
    });
    Ok(CompiledBlend2DDomain::Triangulated { triangles: compiled })
}

fn compile_blend2d_domain(
    graph_name: &str,
    mode: AnimationBlend2DMode,
    samples: &[CompiledBlendSample2D],
) -> Result<CompiledBlend2DDomain, String> {
    validate_blend2d_positions(graph_name, samples)?;
    match mode {
        AnimationBlend2DMode::Cartesian => compile_cartesian_blend2d_domain(graph_name, samples),
        AnimationBlend2DMode::Directional => compile_directional_blend2d_domain(graph_name, samples),
        AnimationBlend2DMode::Triangulated => {
            compile_triangulated_blend2d_domain(graph_name, samples)
        }
    }
}

fn compile_sync_marker_track(
    graph_name: &str,
    group: &CompiledAnimationSyncGroup,
    clip: &AnimationClip,
) -> Result<CompiledSyncMarkerTrack, String> {
    if !clip.looped {
        return Err(format!(
            "animation graph '{graph_name}' marker sync group '{}' requires looped clip '{}'",
            group.name, clip.name
        ));
    }
    let mut markers = Vec::with_capacity(group.marker_tags.len());
    let mut counts = vec![0usize; group.marker_tags.len()];
    for event in &clip.events {
        let key = canonical_graph_key(&event.tag);
        let Some(marker_index) = group.marker_index.get(&key).copied() else {
            continue;
        };
        counts[marker_index] += 1;
        markers.push(CompiledSyncMarker {
            marker_index,
            time_seconds: event.time_seconds,
        });
    }
    for (marker_index, count) in counts.into_iter().enumerate() {
        if count != 1 {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' requires exactly one '{}' marker per cycle found={count}",
                group.name, clip.name, group.marker_tags[marker_index]
            ));
        }
    }
    markers.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    for pair in markers.windows(2) {
        if pair[0].time_seconds + 1.0e-6 >= pair[1].time_seconds {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' contains coincident marker times",
                group.name, clip.name
            ));
        }
    }
    let first = markers[0].marker_index;
    for (offset, marker) in markers.iter().enumerate() {
        let expected = (first + offset) % group.marker_tags.len();
        if marker.marker_index != expected {
            return Err(format!(
                "animation graph '{graph_name}' marker sync group '{}' clip '{}' marker order is not a cyclic rotation of authored vocabulary",
                group.name, clip.name
            ));
        }
    }
    Ok(CompiledSyncMarkerTrack { markers })
}

fn register_motion_sync_tracks(
    graph_name: &str,
    motion: &CompiledAnimationMotion,
    clips: &[CompiledAnimationGraphClip],
    sync_groups: &[CompiledAnimationSyncGroup],
    tracks: &mut [Vec<Option<CompiledSyncMarkerTrack>>],
) -> Result<(), String> {
    let Some(group_index) = motion.marker_sync_group_index() else {
        return Ok(());
    };
    let group = &sync_groups[group_index];
    for clip_index in motion.clip_indices() {
        if tracks[group_index][clip_index].is_none() {
            tracks[group_index][clip_index] = Some(compile_sync_marker_track(
                graph_name,
                group,
                &clips[clip_index].clip,
            )?);
        }
    }
    Ok(())
}

impl CompiledAnimationGraph {
    pub fn compile<F>(
        definition: AnimationGraphDefinition,
        skeleton: &AnimationSkeletonRuntime,
        mut load_clip: F,
    ) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<Arc<AnimationClip>, String>,
    {
        let name = definition.name.trim().to_owned();
        if name.is_empty() {
            return Err("animation graph name is empty".to_owned());
        }
        if definition.states.is_empty() {
            return Err(format!("animation graph '{name}' contains no states"));
        }

        let mut parameter_index = HashMap::with_capacity(definition.parameters.len());
        let mut parameters = Vec::with_capacity(definition.parameters.len());
        for mut parameter in definition.parameters {
            parameter.name = parameter.name.trim().to_owned();
            if parameter.name.is_empty() {
                return Err(format!("animation graph '{name}' contains an empty parameter name"));
            }
            if let AnimationGraphParameterValue::Float(value) = parameter.default {
                if !value.is_finite() {
                    return Err(format!(
                        "animation graph '{name}' parameter '{}' has non-finite default",
                        parameter.name
                    ));
                }
            }
            let key = canonical_graph_key(&parameter.name);
            if parameter_index.insert(key, parameters.len()).is_some() {
                return Err(format!(
                    "animation graph '{name}' contains duplicate parameter '{}'",
                    parameter.name
                ));
            }
            parameters.push(parameter);
        }

        let mut state_index = HashMap::with_capacity(definition.states.len());
        for (index, state) in definition.states.iter().enumerate() {
            let state_name = state.name.trim();
            if state_name.is_empty() {
                return Err(format!("animation graph '{name}' contains an empty state name"));
            }
            let key = canonical_graph_key(state_name);
            if state_index.insert(key, index).is_some() {
                return Err(format!(
                    "animation graph '{name}' contains duplicate state '{state_name}'"
                ));
            }
        }
        let entry_state = state_index
            .get(&canonical_graph_key(&definition.entry_state))
            .copied()
            .ok_or_else(|| {
                format!(
                    "animation graph '{name}' entry state '{}' is not declared",
                    definition.entry_state
                )
            })?;

        let mut sync_groups = Vec::with_capacity(definition.sync_groups.len());
        let mut sync_group_index = HashMap::with_capacity(definition.sync_groups.len());
        for group in &definition.sync_groups {
            let group_name = group.name.trim();
            if group_name.is_empty() {
                return Err(format!("animation graph '{name}' contains an empty sync group name"));
            }
            let key = canonical_graph_key(group_name);
            if sync_group_index.contains_key(&key) {
                return Err(format!(
                    "animation graph '{name}' contains duplicate sync group '{group_name}'"
                ));
            }
            if group.markers.len() < 2 {
                return Err(format!(
                    "animation graph '{name}' sync group '{group_name}' requires at least two markers"
                ));
            }
            let mut marker_tags = Vec::with_capacity(group.markers.len());
            let mut marker_index = HashMap::with_capacity(group.markers.len());
            for marker in &group.markers {
                let marker = marker.trim();
                if marker.is_empty() {
                    return Err(format!(
                        "animation graph '{name}' sync group '{group_name}' contains an empty marker tag"
                    ));
                }
                let marker_key = canonical_graph_key(marker);
                if marker_index.insert(marker_key.clone(), marker_tags.len()).is_some() {
                    return Err(format!(
                        "animation graph '{name}' sync group '{group_name}' contains duplicate marker '{marker}'"
                    ));
                }
                marker_tags.push(marker_key);
            }
            let index = sync_groups.len();
            sync_group_index.insert(key, index);
            sync_groups.push(CompiledAnimationSyncGroup {
                name: group_name.to_owned(),
                marker_tags,
                marker_index,
            });
        }

        let mut clips = Vec::<CompiledAnimationGraphClip>::new();
        let mut clip_index = HashMap::<String, usize>::new();
        let mut resolve_clip = |reference: &str| -> Result<usize, String> {
            let parsed = AnimationClipReference::parse(reference)?;
            let key = parsed.canonical_clip_key;
            if let Some(index) = clip_index.get(&key).copied() {
                return Ok(index);
            }
            let clip = load_clip(reference).map_err(|error| {
                format!("animation graph '{name}' clip load failed ref='{reference}': {error}")
            })?;
            clip.validate_structure()?;
            let binding = skeleton.bind_clip(&clip)?;
            let index = clips.len();
            clips.push(CompiledAnimationGraphClip {
                reference: reference.trim().replace('\\', "/"),
                clip,
                binding,
            });
            clip_index.insert(key, index);
            Ok(index)
        };

        fn compile_motion<F>(
            graph_name: &str,
            motion: AnimationMotionDefinition,
            parameter_index: &HashMap<String, usize>,
            parameters: &[AnimationGraphParameterDefinition],
            sync_group_index: &HashMap<String, usize>,
            resolve_clip: &mut F,
        ) -> Result<CompiledAnimationMotion, String>
        where
            F: FnMut(&str) -> Result<usize, String>,
        {
            match motion {
                AnimationMotionDefinition::Clip(mut motion) => {
                    motion.clip_ref = motion.clip_ref.trim().to_owned();
                    if motion.clip_ref.is_empty() {
                        return Err(format!(
                            "animation graph '{graph_name}' clip motion has an empty reference"
                        ));
                    }
                    let speed = validate_speed(motion.speed, "clip")?;
                    Ok(CompiledAnimationMotion::Clip {
                        clip_index: resolve_clip(&motion.clip_ref)?,
                        speed,
                        sync: compile_motion_sync(&motion.sync_group, sync_group_index),
                    })
                }
                AnimationMotionDefinition::Blend1D(mut tree) => {
                    let parameter_index = resolve_float_graph_parameter(
                        graph_name,
                        "blend1d",
                        &tree.parameter,
                        parameter_index,
                        parameters,
                    )?;
                    if tree.samples.is_empty() {
                        return Err(format!(
                            "animation graph '{graph_name}' blend1d '{}' contains no samples",
                            tree.parameter
                        ));
                    }
                    let mut samples = Vec::with_capacity(tree.samples.len());
                    for sample in tree.samples.drain(..) {
                        if !sample.threshold.is_finite() {
                            return Err(format!(
                                "animation graph '{graph_name}' blend1d '{}' has non-finite threshold",
                                tree.parameter
                            ));
                        }
                        let clip_ref = sample.clip_ref.trim();
                        if clip_ref.is_empty() {
                            return Err(format!(
                                "animation graph '{graph_name}' blend1d '{}' has an empty clip reference",
                                tree.parameter
                            ));
                        }
                        samples.push(CompiledBlendSample1D {
                            threshold: sample.threshold,
                            clip_index: resolve_clip(clip_ref)?,
                            speed: validate_speed(sample.speed, "blend sample")?,
                        });
                    }
                    samples.sort_by(|a, b| a.threshold.total_cmp(&b.threshold));
                    for pair in samples.windows(2) {
                        if pair[0].threshold == pair[1].threshold {
                            return Err(format!(
                                "animation graph '{graph_name}' blend1d '{}' contains duplicate threshold {}",
                                tree.parameter, pair[0].threshold
                            ));
                        }
                    }
                    if canonical_sync_group(&tree.sync_group).is_some() {
                        let reference_speed = samples[0].speed;
                        if samples
                            .iter()
                            .any(|sample| (sample.speed - reference_speed).abs() > 1.0e-6)
                        {
                            return Err(format!(
                                "animation graph '{graph_name}' synchronized blend1d '{}' requires equal sample speeds",
                                tree.parameter
                            ));
                        }
                    }
                    Ok(CompiledAnimationMotion::Blend1D {
                        parameter_index,
                        samples,
                        sync: compile_motion_sync(&tree.sync_group, sync_group_index),
                    })
                }
                AnimationMotionDefinition::Blend2D(mut tree) => {
                    let parameter_x_index = resolve_float_graph_parameter(
                        graph_name,
                        "blend2d x-axis",
                        &tree.parameter_x,
                        parameter_index,
                        parameters,
                    )?;
                    let parameter_y_index = resolve_float_graph_parameter(
                        graph_name,
                        "blend2d y-axis",
                        &tree.parameter_y,
                        parameter_index,
                        parameters,
                    )?;
                    if parameter_x_index == parameter_y_index {
                        return Err(format!(
                            "animation graph '{graph_name}' blend2d requires distinct x/y parameters"
                        ));
                    }
                    if tree.samples.is_empty() {
                        return Err(format!(
                            "animation graph '{graph_name}' blend2d contains no samples"
                        ));
                    }
                    let mut samples = Vec::with_capacity(tree.samples.len());
                    for sample in tree.samples.drain(..) {
                        if sample.position.iter().any(|value| !value.is_finite()) {
                            return Err(format!(
                                "animation graph '{graph_name}' blend2d has non-finite sample position"
                            ));
                        }
                        let clip_ref = sample.clip_ref.trim();
                        if clip_ref.is_empty() {
                            return Err(format!(
                                "animation graph '{graph_name}' blend2d has an empty clip reference"
                            ));
                        }
                        samples.push(CompiledBlendSample2D {
                            position: sample.position,
                            clip_index: resolve_clip(clip_ref)?,
                            speed: validate_speed(sample.speed, "blend2d sample")?,
                        });
                    }
                    let domain = compile_blend2d_domain(graph_name, tree.mode, &samples)?;
                    if canonical_sync_group(&tree.sync_group).is_some() {
                        let reference_speed = samples[0].speed;
                        if samples
                            .iter()
                            .any(|sample| (sample.speed - reference_speed).abs() > 1.0e-6)
                        {
                            return Err(format!(
                                "animation graph '{graph_name}' synchronized blend2d requires equal sample speeds"
                            ));
                        }
                    }
                    Ok(CompiledAnimationMotion::Blend2D {
                        parameter_x_index,
                        parameter_y_index,
                        samples,
                        domain,
                        sync: compile_motion_sync(&tree.sync_group, sync_group_index),
                    })
                }
            }
        }

        let mut states = Vec::with_capacity(definition.states.len());
        for state in definition.states {
            states.push(CompiledAnimationGraphState {
                name: state.name.trim().to_owned(),
                motion: compile_motion(
                    &name,
                    state.motion,
                    &parameter_index,
                    &parameters,
                    &sync_group_index,
                    &mut resolve_clip,
                )?,
                speed: validate_speed(state.speed, "state")?,
                root_motion: state.root_motion,
            });
        }

        let mut transitions_by_state = vec![Vec::new(); states.len()];
        let mut transition_group_index = HashMap::<String, usize>::new();
        for (authored_order, transition) in definition.transitions.into_iter().enumerate() {
            let from_state = state_index
                .get(&canonical_graph_key(&transition.from))
                .copied()
                .ok_or_else(|| {
                    format!(
                        "animation graph '{name}' transition references unknown source state '{}'",
                        transition.from
                    )
                })?;
            let to_state = state_index
                .get(&canonical_graph_key(&transition.to))
                .copied()
                .ok_or_else(|| {
                    format!(
                        "animation graph '{name}' transition references unknown target state '{}'",
                        transition.to
                    )
                })?;
            if from_state == to_state {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' is self-referential",
                    transition.from, transition.to
                ));
            }
            if !transition.blend_seconds.is_finite()
                || transition.blend_seconds < 0.0
                || transition.blend_seconds > 60.0
            {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' has invalid blend duration {}",
                    transition.from, transition.to, transition.blend_seconds
                ));
            }
            if transition.exit_time_normalized.is_some_and(|value| {
                !value.is_finite() || !(0.0..=1.0).contains(&value)
            }) {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' has invalid normalized exit time",
                    transition.from, transition.to
                ));
            }
            let group_id = match transition.group.as_deref() {
                None => None,
                Some(group) => {
                    let group = group.trim();
                    if group.is_empty() {
                        return Err(format!(
                            "animation graph '{name}' transition '{}' -> '{}' has an empty interruption group",
                            transition.from, transition.to
                        ));
                    }
                    let key = canonical_graph_key(group);
                    let next = transition_group_index.len();
                    Some(*transition_group_index.entry(key).or_insert(next))
                }
            };
            if transition.interruption == AnimationTransitionInterruptionPolicy::SameGroup
                && group_id.is_none()
            {
                return Err(format!(
                    "animation graph '{name}' transition '{}' -> '{}' uses same_group interruption without a group",
                    transition.from, transition.to
                ));
            }
            let mut conditions = Vec::with_capacity(transition.conditions.len());
            for condition in transition.conditions {
                let compiled = match condition {
                    AnimationTransitionCondition::Bool { parameter, equals } => {
                        let index = parameter_index
                            .get(&canonical_graph_key(&parameter))
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "animation graph '{name}' transition references unknown bool parameter '{parameter}'"
                                )
                            })?;
                        if !matches!(parameters[index].default, AnimationGraphParameterValue::Bool(_))
                        {
                            return Err(format!(
                                "animation graph '{name}' transition expects bool parameter '{parameter}'"
                            ));
                        }
                        CompiledTransitionCondition::Bool {
                            parameter_index: index,
                            equals,
                        }
                    }
                    AnimationTransitionCondition::Float {
                        parameter,
                        comparison,
                        value,
                    } => {
                        if !value.is_finite() {
                            return Err(format!(
                                "animation graph '{name}' transition float threshold is non-finite parameter='{parameter}'"
                            ));
                        }
                        let index = parameter_index
                            .get(&canonical_graph_key(&parameter))
                            .copied()
                            .ok_or_else(|| {
                                format!(
                                    "animation graph '{name}' transition references unknown float parameter '{parameter}'"
                                )
                            })?;
                        if !matches!(parameters[index].default, AnimationGraphParameterValue::Float(_))
                        {
                            return Err(format!(
                                "animation graph '{name}' transition expects float parameter '{parameter}'"
                            ));
                        }
                        CompiledTransitionCondition::Float {
                            parameter_index: index,
                            comparison,
                            value,
                        }
                    }
                };
                conditions.push(compiled);
            }
            transitions_by_state[from_state].push(CompiledAnimationGraphTransition {
                from_state,
                to_state,
                conditions,
                exit_time_normalized: transition.exit_time_normalized,
                blend_seconds: transition.blend_seconds,
                priority: transition.priority,
                authored_order,
                group_id,
                interruption: transition.interruption,
            });
        }
        for transitions in &mut transitions_by_state {
            transitions.sort_by(|a, b| {
                b.priority
                    .cmp(&a.priority)
                    .then_with(|| a.authored_order.cmp(&b.authored_order))
            });
        }

        let mut layers = Vec::with_capacity(definition.layers.len());
        let mut layer_names = HashSet::new();
        for layer in definition.layers {
            let layer_name = layer.name.trim().to_owned();
            if layer_name.is_empty() {
                return Err(format!("animation graph '{name}' contains an empty layer name"));
            }
            if !layer_names.insert(canonical_graph_key(&layer_name)) {
                return Err(format!(
                    "animation graph '{name}' contains duplicate layer '{layer_name}'"
                ));
            }
            if !layer.weight.is_finite() || !(0.0..=1.0).contains(&layer.weight) {
                return Err(format!(
                    "animation graph '{name}' layer '{layer_name}' weight must be in [0,1]"
                ));
            }
            if !layer.event_weight_threshold.is_finite()
                || !(0.0..=1.0).contains(&layer.event_weight_threshold)
            {
                return Err(format!(
                    "animation graph '{name}' layer '{layer_name}' event threshold must be in [0,1]"
                ));
            }
            let weight_parameter_index = layer
                .weight_parameter
                .as_deref()
                .map(|parameter| {
                    let index = parameter_index
                        .get(&canonical_graph_key(parameter))
                        .copied()
                        .ok_or_else(|| {
                            format!(
                                "animation graph '{name}' layer '{layer_name}' references unknown weight parameter '{parameter}'"
                            )
                        })?;
                    if !matches!(parameters[index].default, AnimationGraphParameterValue::Float(_)) {
                        return Err(format!(
                            "animation graph '{name}' layer '{layer_name}' weight parameter '{parameter}' is not float"
                        ));
                    }
                    Ok(index)
                })
                .transpose()?;
            let mask = compile_bone_mask(&name, &layer_name, layer.mask, skeleton)?;
            layers.push(CompiledAnimationGraphLayer {
                name: layer_name,
                motion: compile_motion(
                    &name,
                    layer.motion,
                    &parameter_index,
                    &parameters,
                    &sync_group_index,
                    &mut resolve_clip,
                )?,
                mode: layer.mode,
                weight: layer.weight,
                weight_parameter_index,
                mask,
                event_weight_threshold: layer.event_weight_threshold,
            });
        }

        let mut sync_marker_tracks = vec![vec![None; clips.len()]; sync_groups.len()];
        for state in &states {
            register_motion_sync_tracks(
                &name,
                &state.motion,
                &clips,
                &sync_groups,
                &mut sync_marker_tracks,
            )?;
        }
        for layer in &layers {
            register_motion_sync_tracks(
                &name,
                &layer.motion,
                &clips,
                &sync_groups,
                &mut sync_marker_tracks,
            )?;
        }

        let needs_root_motion = states
            .iter()
            .any(|state| state.root_motion != AnimationRootMotionMode::Disabled);
        let root_motion_joint_index = if needs_root_motion {
            let tag = definition.root_motion_joint_tag.ok_or_else(|| {
                format!(
                    "animation graph '{name}' extracts root motion but root_motion_joint_tag is absent"
                )
            })?;
            Some(skeleton.resolve_joint_tag(tag).map_err(|error| {
                format!("animation graph '{name}' root-motion joint invalid: {error}")
            })?)
        } else {
            definition
                .root_motion_joint_tag
                .map(|tag| skeleton.resolve_joint_tag(tag))
                .transpose()?
        };

        Ok(Self {
            name,
            entry_state,
            parameters,
            parameter_index,
            states,
            state_index,
            transitions_by_state,
            layers,
            clips,
            sync_marker_tracks,
            root_motion_joint_index,
        })
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn entry_state_index(&self) -> usize {
        self.entry_state
    }

    #[inline]
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    #[inline]
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    #[inline]
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    #[inline]
    pub fn state_name(&self, index: usize) -> Option<&str> {
        self.states.get(index).map(|state| state.name.as_str())
    }

    #[inline]
    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.state_index.get(&canonical_graph_key(name)).copied()
    }

    #[inline]
    pub fn layer_name(&self, index: usize) -> Option<&str> {
        self.layers.get(index).map(|layer| layer.name.as_str())
    }

    #[inline]
    pub fn clip_reference(&self, index: usize) -> Option<&str> {
        self.clips.get(index).map(|clip| clip.reference.as_str())
    }

    #[inline]
    pub fn clip(&self, index: usize) -> Option<&Arc<AnimationClip>> {
        self.clips.get(index).map(|clip| &clip.clip)
    }

    #[inline]
    pub fn parameter_index(&self, name: &str) -> Option<usize> {
        self.parameter_index.get(&canonical_graph_key(name)).copied()
    }
}

fn compile_bone_mask(
    graph_name: &str,
    layer_name: &str,
    definition: Option<AnimationBoneMaskDefinition>,
    skeleton: &AnimationSkeletonRuntime,
) -> Result<Vec<f32>, String> {
    let Some(definition) = definition else {
        return Ok(vec![1.0; skeleton.joint_count()]);
    };
    let mut mask = vec![0.0_f32; skeleton.joint_count()];
    for root in definition.roots {
        if !root.weight.is_finite() || !(0.0..=1.0).contains(&root.weight) {
            return Err(format!(
                "animation graph '{graph_name}' layer '{layer_name}' mask weight is invalid tag={} weight={}",
                root.joint_tag, root.weight
            ));
        }
        let joint = skeleton.resolve_joint_tag(root.joint_tag).map_err(|error| {
            format!(
                "animation graph '{graph_name}' layer '{layer_name}' mask joint invalid: {error}"
            )
        })?;
        mask[joint] = root.weight;
        if root.include_descendants {
            for (candidate, candidate_weight) in mask.iter_mut().enumerate() {
                let mut parent = skeleton.parent_indices[candidate];
                while let Some(parent_index) = parent {
                    if parent_index == joint {
                        *candidate_weight = root.weight;
                        break;
                    }
                    parent = skeleton.parent_indices[parent_index];
                }
            }
        }
    }
    Ok(mask)
}

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
