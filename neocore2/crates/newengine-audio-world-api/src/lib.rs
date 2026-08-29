#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral transient contracts at the physics/world-audio boundary.
//!
//! These values may live in ECS/resource storage, but their lifecycle remains owned by
//! the producer/consumer runtimes. Keeping the types here prevents either side from
//! depending on the other's implementation crate.

use std::collections::BTreeMap;

use newengine_audio_api::{AcousticMaterialProfile, AudioListenerState};

/// Presentation-cadence listener pose projected into ECS for fixed-step acoustic probes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioListenerRuntimeState {
    pub listener: AudioListenerState,
    pub frame_index: u64,
}

/// Provider-neutral OBB used by first-order acoustic reflection probes. Quaternion order is
/// `[x, y, z, w]`, matching the engine transform contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioRoomObbGeometry {
    pub center: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub half_extents: [f32; 3],
}

/// Exact first-order specular path against one room OBB face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioFirstOrderReflectionGeometry {
    pub face_index: u8,
    pub reflection_point: [f32; 3],
    /// Unit vector from listener toward the apparent reflection arrival point.
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
}

/// Exact specular second-order path against two ordered room OBB faces. The face sequence follows
/// acoustic travel from source to listener.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioSecondOrderReflectionGeometry {
    pub face_indices: [u8; 2],
    pub reflection_points: [[f32; 3]; 2],
    /// Unit vector from listener toward the final apparent arrival point.
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
}

/// Provider-neutral diffraction edge extracted from a canonical triangle mesh. Endpoints and
/// normals remain in the mesh coordinate system supplied to `mesh_diffraction_edges`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioMeshDiffractionEdgeGeometry {
    pub vertex_indices: [u32; 2],
    pub endpoints: [[f32; 3]; 2],
    pub adjacent_normals: [[f32; 3]; 2],
    pub adjacent_faces: u8,
    /// Angle between the first two adjacent face normals. Boundary edges use PI.
    pub wedge_angle_radians: f32,
}

/// Shortest broken acoustic path constrained to pass through one finite mesh edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEdgeDiffractionGeometry {
    pub diffraction_point: [f32; 3],
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
    /// Zero is straight-through; PI is a complete reversal around the edge.
    pub bend_angle_radians: f32,
}

#[derive(Clone, Copy)]
struct MeshEdgeAccumulator {
    normals: [[f32; 3]; 2],
    face_count: u8,
}

impl Default for MeshEdgeAccumulator {
    fn default() -> Self {
        Self {
            normals: [[0.0; 3]; 2],
            face_count: 0,
        }
    }
}

/// Extracts boundary and non-coplanar triangle adjacency edges. Coplanar triangulation diagonals are
/// deliberately removed, so runtime diffraction works from actual geometric discontinuities rather
/// than tessellation artifacts.
pub fn mesh_diffraction_edges(
    vertices: &[[f32; 3]],
    triangles: &[[u32; 3]],
) -> Vec<AudioMeshDiffractionEdgeGeometry> {
    let mut adjacency = BTreeMap::<(u32, u32), MeshEdgeAccumulator>::new();
    for triangle in triangles {
        let [ia, ib, ic] = *triangle;
        let Some(a) = vertices.get(ia as usize).copied() else {
            continue;
        };
        let Some(b) = vertices.get(ib as usize).copied() else {
            continue;
        };
        let Some(c) = vertices.get(ic as usize).copied() else {
            continue;
        };
        let normal = normalize3(cross3(sub3(b, a), sub3(c, a)));
        if length3(normal) <= 1.0e-5 {
            continue;
        }
        for [left, right] in [[ia, ib], [ib, ic], [ic, ia]] {
            let key = if left <= right {
                (left, right)
            } else {
                (right, left)
            };
            let entry = adjacency.entry(key).or_default();
            if usize::from(entry.face_count) < entry.normals.len() {
                entry.normals[usize::from(entry.face_count)] = normal;
            }
            entry.face_count = entry.face_count.saturating_add(1);
        }
    }

    const COPLANAR_ANGLE_EPSILON_RADIANS: f32 = 0.03;
    let mut edges = Vec::new();
    for ((a, b), adjacency) in adjacency {
        let Some(first) = vertices.get(a as usize).copied() else {
            continue;
        };
        let Some(second) = vertices.get(b as usize).copied() else {
            continue;
        };
        if length3(sub3(second, first)) <= 1.0e-5 {
            continue;
        }
        let wedge_angle_radians = if adjacency.face_count <= 1 {
            std::f32::consts::PI
        } else {
            dot3(adjacency.normals[0], adjacency.normals[1])
                .clamp(-1.0, 1.0)
                .acos()
        };
        if adjacency.face_count == 2 && wedge_angle_radians < COPLANAR_ANGLE_EPSILON_RADIANS {
            continue;
        }
        edges.push(AudioMeshDiffractionEdgeGeometry {
            vertex_indices: [a, b],
            endpoints: [first, second],
            adjacent_normals: adjacency.normals,
            adjacent_faces: adjacency.face_count,
            wedge_angle_radians,
        });
    }
    edges
}

/// Finds the shortest source-edge-listener path on a finite line segment. The objective is convex,
/// so a bounded ternary solve is deterministic and sufficiently precise for acoustic query planning.
pub fn edge_diffraction_geometry(
    endpoints: [[f32; 3]; 2],
    source: [f32; 3],
    listener: [f32; 3],
) -> Option<AudioEdgeDiffractionGeometry> {
    if endpoints
        .into_iter()
        .chain([source, listener])
        .flatten()
        .any(|value| !value.is_finite())
    {
        return None;
    }
    let edge = sub3(endpoints[1], endpoints[0]);
    if length3(edge) <= 1.0e-5 {
        return None;
    }
    let direct = length3(sub3(source, listener));
    if direct <= 1.0e-5 || !direct.is_finite() {
        return None;
    }
    let path_at = |t: f32| {
        let point = add3(endpoints[0], scale3(edge, t));
        length3(sub3(source, point)) + length3(sub3(listener, point))
    };
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for _ in 0..24 {
        let third = (hi - lo) / 3.0;
        let left = lo + third;
        let right = hi - third;
        if path_at(left) <= path_at(right) {
            hi = right;
        } else {
            lo = left;
        }
    }
    let t = (lo + hi) * 0.5;
    let point = add3(endpoints[0], scale3(edge, t));
    let path_length_m = path_at(t);
    if !path_length_m.is_finite() {
        return None;
    }
    let to_source = normalize3(sub3(source, point));
    let to_listener = normalize3(sub3(listener, point));
    let included = dot3(to_source, to_listener).clamp(-1.0, 1.0).acos();
    Some(AudioEdgeDiffractionGeometry {
        diffraction_point: point,
        arrival_direction: normalize3(sub3(point, listener)),
        path_length_m,
        excess_length_m: (path_length_m - direct).max(0.0),
        bend_angle_radians: (std::f32::consts::PI - included).clamp(0.0, std::f32::consts::PI),
    })
}

/// Fixed-step visibility/material result for one first-order reflection path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEarlyReflectionPathObservation {
    pub face_index: u8,
    pub visible: bool,
    pub boundary_entity: Option<u64>,
    pub reflection_point: [f32; 3],
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
    pub material_known: bool,
    pub material: AcousticMaterialProfile,
}

impl Default for AudioEarlyReflectionPathObservation {
    fn default() -> Self {
        Self {
            face_index: 0,
            visible: false,
            boundary_entity: None,
            reflection_point: [0.0; 3],
            arrival_direction: [0.0; 3],
            path_length_m: 0.0,
            excess_length_m: 0.0,
            material_known: false,
            material: AcousticMaterialProfile::transparent(),
        }
    }
}

/// Fixed-step visibility/material result for one second-order reflection path. Materials are
/// ordered with the bounce faces and remain independent so energy/spectrum can accumulate per bounce.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioSecondOrderReflectionPathObservation {
    pub face_indices: [u8; 2],
    pub visible: bool,
    pub boundary_entities: [Option<u64>; 2],
    pub reflection_points: [[f32; 3]; 2],
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
    pub material_known: [bool; 2],
    pub materials: [AcousticMaterialProfile; 2],
}

impl Default for AudioSecondOrderReflectionPathObservation {
    fn default() -> Self {
        Self {
            face_indices: [0; 2],
            visible: false,
            boundary_entities: [None; 2],
            reflection_points: [[0.0; 3]; 2],
            arrival_direction: [0.0; 3],
            path_length_m: 0.0,
            excess_length_m: 0.0,
            material_known: [false; 2],
            materials: [AcousticMaterialProfile::transparent(); 2],
        }
    }
}

/// Ephemeral result of bounded reflection visibility probes for one spatial audio emitter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioEarlyReflectionObservation {
    pub fixed_tick: u64,
    pub source_position: [f32; 3],
    pub listener_position: [f32; 3],
    /// First-order reflection paths retained for compatibility and diagnostics.
    pub paths: Vec<AudioEarlyReflectionPathObservation>,
    /// Bounded second-order paths. The physics producer decides the runtime budget.
    pub second_order_paths: Vec<AudioSecondOrderReflectionPathObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioEdgeDiffractionPathObservation {
    pub edge_vertex_indices: [u32; 2],
    pub visible: bool,
    pub diffraction_point: [f32; 3],
    pub arrival_direction: [f32; 3],
    pub path_length_m: f32,
    pub excess_length_m: f32,
    pub bend_angle_radians: f32,
    pub wedge_angle_radians: f32,
    pub material_known: bool,
    pub material: AcousticMaterialProfile,
}

impl Default for AudioEdgeDiffractionPathObservation {
    fn default() -> Self {
        Self {
            edge_vertex_indices: [0; 2],
            visible: false,
            diffraction_point: [0.0; 3],
            arrival_direction: [0.0; 3],
            path_length_m: 0.0,
            excess_length_m: 0.0,
            bend_angle_radians: 0.0,
            wedge_angle_radians: std::f32::consts::PI,
            material_known: false,
            material: AcousticMaterialProfile::transparent(),
        }
    }
}

/// Bounded edge-diffraction candidates tied to the actual direct-path blocker entity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioEdgeDiffractionObservation {
    pub fixed_tick: u64,
    pub source_position: [f32; 3],
    pub listener_position: [f32; 3],
    pub blocker_entity: Option<u64>,
    pub paths: Vec<AudioEdgeDiffractionPathObservation>,
}

/// Computes exact specular first-order reflection points against the six faces of an oriented
/// room box. This function is pure geometry: no ECS, physics backend, or material policy enters it.
pub fn first_order_reflection_geometry(
    room: AudioRoomObbGeometry,
    source_world: [f32; 3],
    listener_world: [f32; 3],
) -> Vec<AudioFirstOrderReflectionGeometry> {
    let q = quat_normalized(room.rotation_xyzw);
    let qi = [-q[0], -q[1], -q[2], q[3]];
    let source = quat_rotate(qi, sub3(source_world, room.center));
    let listener = quat_rotate(qi, sub3(listener_world, room.center));
    let ext = room.half_extents.map(|v| {
        if v.is_finite() {
            v.abs().max(1.0e-4)
        } else {
            1.0
        }
    });
    if !inside_obb_local(source, ext) || !inside_obb_local(listener, ext) {
        return Vec::new();
    }
    let direct = length3(sub3(source, listener)).max(1.0e-5);
    let mut paths = Vec::with_capacity(6);
    for axis in 0..3usize {
        for sign_index in 0..2usize {
            let sign = if sign_index == 0 { -1.0 } else { 1.0 };
            let plane = sign * ext[axis];
            let mut image = source;
            image[axis] = 2.0 * plane - source[axis];
            let denom = image[axis] - listener[axis];
            if denom.abs() <= 1.0e-6 {
                continue;
            }
            let t = (plane - listener[axis]) / denom;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let mut point = [0.0; 3];
            for component in 0..3 {
                point[component] =
                    listener[component] + (image[component] - listener[component]) * t;
            }
            let mut valid = true;
            for component in 0..3 {
                if component != axis && point[component].abs() > ext[component] + 1.0e-4 {
                    valid = false;
                }
            }
            if !valid {
                continue;
            }
            let source_leg = length3(sub3(source, point));
            let listener_leg = length3(sub3(point, listener));
            let path_length = source_leg + listener_leg;
            let world_point = add3(room.center, quat_rotate(q, point));
            let arrival_direction = normalize3(sub3(world_point, listener_world));
            paths.push(AudioFirstOrderReflectionGeometry {
                face_index: (axis * 2 + sign_index) as u8,
                reflection_point: world_point,
                arrival_direction,
                path_length_m: path_length,
                excess_length_m: (path_length - direct).max(0.0),
            });
        }
    }
    paths.sort_by(|a, b| {
        a.path_length_m
            .total_cmp(&b.path_length_m)
            .then_with(|| a.face_index.cmp(&b.face_index))
    });
    paths
}

/// Computes exact specular second-order reflection paths against ordered pairs of distinct OBB
/// faces. Geometry is solved in room-local space with the image-source method, then transformed
/// back to world space. No visibility, material or runtime policy enters this function.
pub fn second_order_reflection_geometry(
    room: AudioRoomObbGeometry,
    source_world: [f32; 3],
    listener_world: [f32; 3],
) -> Vec<AudioSecondOrderReflectionGeometry> {
    let q = quat_normalized(room.rotation_xyzw);
    let qi = [-q[0], -q[1], -q[2], q[3]];
    let source = quat_rotate(qi, sub3(source_world, room.center));
    let listener = quat_rotate(qi, sub3(listener_world, room.center));
    let ext = room.half_extents.map(|value| {
        if value.is_finite() {
            value.abs().max(1.0e-4)
        } else {
            1.0
        }
    });
    if !inside_obb_local(source, ext) || !inside_obb_local(listener, ext) {
        return Vec::new();
    }

    let direct = length3(sub3(source, listener)).max(1.0e-5);
    let mut paths = Vec::with_capacity(30);
    for first_face in 0_u8..6 {
        let first_image = reflect_across_face(source, ext, first_face);
        for second_face in 0_u8..6 {
            if second_face == first_face {
                continue;
            }
            let second_image = reflect_across_face(first_image, ext, second_face);
            let Some(second_point) =
                segment_face_intersection(listener, second_image, ext, second_face)
            else {
                continue;
            };
            let Some(first_point) =
                segment_face_intersection(second_point, first_image, ext, first_face)
            else {
                continue;
            };

            let source_leg = length3(sub3(source, first_point));
            let middle_leg = length3(sub3(first_point, second_point));
            let listener_leg = length3(sub3(second_point, listener));
            let path_length = source_leg + middle_leg + listener_leg;
            if !path_length.is_finite() || path_length + 1.0e-4 < direct {
                continue;
            }
            let first_world = add3(room.center, quat_rotate(q, first_point));
            let second_world = add3(room.center, quat_rotate(q, second_point));
            paths.push(AudioSecondOrderReflectionGeometry {
                face_indices: [first_face, second_face],
                reflection_points: [first_world, second_world],
                arrival_direction: normalize3(sub3(second_world, listener_world)),
                path_length_m: path_length,
                excess_length_m: (path_length - direct).max(0.0),
            });
        }
    }
    paths.sort_by(|a, b| {
        a.path_length_m
            .total_cmp(&b.path_length_m)
            .then_with(|| a.face_indices.cmp(&b.face_indices))
    });
    paths
}

#[inline]
fn reflect_across_face(mut point: [f32; 3], ext: [f32; 3], face: u8) -> [f32; 3] {
    let axis = usize::from(face / 2).min(2);
    let sign = if face & 1 == 0 { -1.0 } else { 1.0 };
    let plane = sign * ext[axis];
    point[axis] = 2.0 * plane - point[axis];
    point
}

fn segment_face_intersection(
    origin: [f32; 3],
    target: [f32; 3],
    ext: [f32; 3],
    face: u8,
) -> Option<[f32; 3]> {
    let axis = usize::from(face / 2).min(2);
    let sign = if face & 1 == 0 { -1.0 } else { 1.0 };
    let plane = sign * ext[axis];
    let denom = target[axis] - origin[axis];
    if !denom.is_finite() || denom.abs() <= 1.0e-6 {
        return None;
    }
    let t = (plane - origin[axis]) / denom;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    let point = std::array::from_fn(|component| {
        origin[component] + (target[component] - origin[component]) * t
    });
    for component in 0..3 {
        if component != axis && point[component].abs() > ext[component] + 1.0e-4 {
            return None;
        }
    }
    Some(point)
}

#[inline]
fn inside_obb_local(point: [f32; 3], ext: [f32; 3]) -> bool {
    point[0].abs() <= ext[0] && point[1].abs() <= ext[1] && point[2].abs() <= ext[2]
}

#[inline]
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn scale3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn length3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(0.0).sqrt()
}

#[inline]
fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = length3(v);
    if len <= 1.0e-6 || !len.is_finite() {
        [0.0; 3]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

#[inline]
fn quat_normalized(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len <= 1.0e-6 || !len.is_finite() {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    }
}

#[inline]
fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let qv = [q[0], q[1], q[2]];
    let uv = cross3(qv, v);
    let uuv = cross3(qv, uv);
    [
        v[0] + 2.0 * (q[3] * uv[0] + uuv[0]),
        v[1] + 2.0 * (q[3] * uv[1] + uuv[1]),
        v[2] + 2.0 * (q[3] * uv[2] + uuv[2]),
    ]
}

#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Raw fixed-step obstruction observation produced by a physics-query contributor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioOcclusionObservation {
    pub fixed_tick: u64,
    pub samples: u8,
    pub blocked_samples: u8,
    pub obstruction: f32,
    pub occlusion: f32,
    /// Bidirectional center-ray estimate for a single closed blocker in world meters.
    /// Zero means unavailable/not applicable, not necessarily zero physical thickness.
    pub estimated_thickness_m: f32,
    /// Number of distinct blocker layers proven by the two center rays (0..=2).
    pub center_blocker_layers: u8,
    /// Stable entity key of the center-path blocker when one is proven. Diffraction uses only
    /// geometry owned by this entity, preventing unrelated scene edges from becoming bypasses.
    pub dominant_blocker_entity: Option<u64>,
    pub dominant_material: String,
    pub material: AcousticMaterialProfile,
}

/// Ephemeral inspection mirror of a world-audio emitter.
/// The audio-world runtime owns creation/update/removal; this crate owns only the DTO shape.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioEmitterRuntime {
    pub voice_id: Option<u64>,
    pub cue: String,
    pub provider: String,
    pub obstruction: f32,
    pub occlusion: f32,
    pub estimated_occluder_thickness_m: f32,
    pub center_blocker_layers: u8,
    pub transmission_gain: f32,
    pub high_frequency_gain: f32,
    pub low_pass_hz: f32,
    pub acoustic_material: String,
    pub acoustic_fixed_tick: u64,
    pub emitter_environment: String,
    pub listener_environment: String,
    pub portal_gain: f32,
    pub direct_path_gain: f32,
    pub direct_path_high_frequency_gain: f32,
    pub direct_path_low_pass_hz: f32,
    pub direct_path_extra_delay_ms: f32,
    pub source_reverb_send: f32,
    pub listener_reverb_send: f32,
    pub source_reverb_decay_seconds: f32,
    pub listener_reverb_decay_seconds: f32,
}

impl Default for AudioEmitterRuntime {
    fn default() -> Self {
        Self {
            voice_id: None,
            cue: String::new(),
            provider: String::new(),
            obstruction: 0.0,
            occlusion: 0.0,
            estimated_occluder_thickness_m: 0.0,
            center_blocker_layers: 0,
            transmission_gain: 1.0,
            high_frequency_gain: 1.0,
            low_pass_hz: 20_000.0,
            acoustic_material: "surface.clear".to_owned(),
            acoustic_fixed_tick: 0,
            emitter_environment: String::new(),
            listener_environment: String::new(),
            portal_gain: 1.0,
            direct_path_gain: 1.0,
            direct_path_high_frequency_gain: 1.0,
            direct_path_low_pass_hz: 20_000.0,
            direct_path_extra_delay_ms: 0.0,
            source_reverb_send: 0.0,
            listener_reverb_send: 0.0,
            source_reverb_decay_seconds: 0.1,
            listener_reverb_decay_seconds: 0.1,
        }
    }
}

/// Ephemeral inspection mirror of an authored ambience bed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioAmbienceBedRuntime {
    pub voice_id: Option<u64>,
    pub bed_id: String,
    pub stream: String,
    pub current_gain: f32,
    pub target_gain: f32,
    pub listener_zone: String,
    pub listener_outdoor: bool,
    pub portal_gain: f32,
    pub provider: String,
}

#[cfg(test)]
mod reflection_geometry_tests {
    use super::*;

    fn room() -> AudioRoomObbGeometry {
        AudioRoomObbGeometry {
            center: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            half_extents: [5.0, 4.0, 6.0],
        }
    }

    #[test]
    fn second_order_paths_have_two_distinct_bounces_and_are_deterministic() {
        let first = second_order_reflection_geometry(room(), [1.0, 0.5, 0.0], [-1.0, 0.0, 0.5]);
        let second = second_order_reflection_geometry(room(), [1.0, 0.5, 0.0], [-1.0, 0.0, 0.5]);
        assert!(!first.is_empty());
        assert_eq!(first, second);
        assert!(first
            .iter()
            .all(|path| path.face_indices[0] != path.face_indices[1]));
        assert!(first
            .windows(2)
            .all(|pair| { pair[0].path_length_m <= pair[1].path_length_m + 1.0e-6 }));
    }

    #[test]
    fn second_order_bounce_points_lie_on_declared_room_faces() {
        let paths = second_order_reflection_geometry(room(), [0.7, 0.4, -0.8], [-0.6, 0.2, 0.9]);
        assert!(!paths.is_empty());
        let ext = room().half_extents;
        for path in paths {
            for (face, point) in path.face_indices.into_iter().zip(path.reflection_points) {
                let axis = usize::from(face / 2);
                let expected = if face & 1 == 0 { -ext[axis] } else { ext[axis] };
                assert!((point[axis] - expected).abs() < 1.0e-3);
                for component in 0..3 {
                    if component != axis {
                        assert!(point[component].abs() <= ext[component] + 1.0e-3);
                    }
                }
            }
        }
    }

    #[test]
    fn second_order_paths_are_longer_than_direct_path() {
        let source = [1.0, 0.0, 0.0];
        let listener = [-1.0, 0.0, 0.0];
        let direct = length3(sub3(source, listener));
        let paths = second_order_reflection_geometry(room(), source, listener);
        assert!(!paths.is_empty());
        assert!(paths
            .iter()
            .all(|path| path.path_length_m + 1.0e-4 >= direct));
        assert!(paths.iter().any(|path| path.excess_length_m > 0.1));
    }

    #[test]
    fn mesh_diffraction_edges_remove_coplanar_triangulation_diagonal() {
        let vertices = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let triangles = [[0, 1, 2], [0, 2, 3]];
        let edges = mesh_diffraction_edges(&vertices, &triangles);
        assert_eq!(edges.len(), 4);
        assert!(!edges.iter().any(|edge| edge.vertex_indices == [0, 2]));
        assert!(edges.iter().all(|edge| edge.adjacent_faces == 1));
    }

    #[test]
    fn mesh_diffraction_edges_keep_cube_dihedral_edges_but_not_face_diagonals() {
        let vertices = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        let triangles = [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let edges = mesh_diffraction_edges(&vertices, &triangles);
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|edge| edge.adjacent_faces == 2));
        assert!(edges.iter().all(|edge| {
            (edge.wedge_angle_radians - std::f32::consts::FRAC_PI_2).abs() < 1.0e-4
        }));
    }

    #[test]
    fn finite_edge_diffraction_geometry_finds_symmetric_shortest_detour() {
        let geometry = edge_diffraction_geometry(
            [[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [-1.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        )
        .expect("edge path");
        assert!(geometry.diffraction_point[0].abs() < 1.0e-3);
        assert!(geometry.path_length_m > 2.8 && geometry.path_length_m < 2.9);
        assert!(geometry.excess_length_m > 0.8);
        assert!(geometry.bend_angle_radians > 1.4 && geometry.bend_angle_radians < 1.7);
    }
}
