use super::*;

use std::collections::{HashMap, HashSet};

use newengine_model_client::ModelGatewayClient;
use newengine_model_domain_api::{ModelAssetRequest, ModelSkinBinding, ModelSkinVertex};
use newengine_model_skeleton_api::ModelSkeletonMetadata;
use newengine_primitives::PrimitiveMesh;

const FPP_NEAR_HEAD_VERTEX_WEIGHT: f32 = 0.08;
const FPP_NEAR_HEAD_TRIANGLE_AVERAGE_WEIGHT: f32 = 0.10;
const FPP_MIN_REMOVED_TRIANGLES: usize = 12;
const FPP_MAX_REMOVED_TRIANGLE_FRACTION: f32 = 0.35;
const FPP_CAP_MAX_VERTICAL_SPAN_M: f32 = 0.18;
const FPP_CAP_MIN_HORIZONTAL_RADIUS_M: f32 = 0.015;
const FPP_CAP_MAX_HORIZONTAL_RADIUS_M: f32 = 0.20;

#[derive(Clone, Debug)]
struct FirstPersonOwnerBodyBuild {
    mesh: PrimitiveMesh,
    removed_triangles: usize,
    cap_triangles: usize,
    cap_loops: usize,
}

#[inline]
fn skeleton_joint_is_descendant_of(
    skeleton: &ModelSkeletonMetadata,
    mut joint_index: usize,
    ancestor_index: usize,
) -> bool {
    let mut guard = 0usize;
    loop {
        if joint_index == ancestor_index {
            return true;
        }
        if guard >= skeleton.joints.len() {
            return false;
        }
        let Some(parent) = skeleton
            .joints
            .get(joint_index)
            .and_then(|joint| joint.parent_index)
            .map(|index| index as usize)
            .filter(|index| *index < skeleton.joints.len())
        else {
            return false;
        };
        if parent == joint_index {
            return false;
        }
        joint_index = parent;
        guard += 1;
    }
}

#[inline]
fn joint_is_first_person_near_head(
    skeleton: &ModelSkeletonMetadata,
    joint_index: usize,
    head_index: usize,
) -> bool {
    let Some(joint) = skeleton.joints.get(joint_index) else {
        return false;
    };
    let name = joint.name.to_ascii_lowercase();
    name.contains("neck")
        || name.contains("head")
        || skeleton_joint_is_descendant_of(skeleton, joint_index, head_index)
}

#[inline]
fn first_person_near_head_weight(
    vertex: &ModelSkinVertex,
    skeleton: &ModelSkeletonMetadata,
    head_index: usize,
) -> f32 {
    vertex
        .joints
        .iter()
        .zip(vertex.weights.iter())
        .chain(vertex.joints_extra.iter().zip(vertex.weights_extra.iter()))
        .filter_map(|(&joint, &weight)| {
            (weight.is_finite()
                && weight > 0.0
                && joint_is_first_person_near_head(skeleton, usize::from(joint), head_index))
            .then_some(weight)
        })
        .sum::<f32>()
        .clamp(0.0, 1.0)
}

#[inline]
fn normalized_edge(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[inline]
fn boundary_weld_key(position: [f32; 3]) -> (i32, i32, i32) {
    // Half-millimetre welding is narrow enough to preserve authored silhouette while reconnecting
    // duplicate UV/normal seam vertices emitted by imported character meshes.
    const INV_TOLERANCE_M: f32 = 2_000.0;
    (
        (position[0] * INV_TOLERANCE_M).round() as i32,
        (position[1] * INV_TOLERANCE_M).round() as i32,
        (position[2] * INV_TOLERANCE_M).round() as i32,
    )
}

fn weld_boundary_edges(mesh: &PrimitiveMesh, edges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut representatives = HashMap::<(i32, i32, i32), u32>::new();
    for &(a, b) in edges {
        for index in [a, b] {
            let Some(vertex) = mesh.vertices.get(index as usize) else {
                continue;
            };
            representatives
                .entry(boundary_weld_key(vertex.pos))
                .or_insert(index);
        }
    }
    let mut welded = HashSet::<(u32, u32)>::new();
    for &(a, b) in edges {
        let (Some(a_vertex), Some(b_vertex)) =
            (mesh.vertices.get(a as usize), mesh.vertices.get(b as usize))
        else {
            continue;
        };
        let Some(&a_rep) = representatives.get(&boundary_weld_key(a_vertex.pos)) else {
            continue;
        };
        let Some(&b_rep) = representatives.get(&boundary_weld_key(b_vertex.pos)) else {
            continue;
        };
        if a_rep != b_rep {
            welded.insert(normalized_edge(a_rep, b_rep));
        }
    }
    welded.into_iter().collect()
}

fn simple_boundary_paths(edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut adjacency = HashMap::<u32, Vec<u32>>::new();
    for &(a, b) in edges {
        adjacency.entry(a).or_default().push(b);
        adjacency.entry(b).or_default().push(a);
    }

    let mut consumed = HashSet::<u32>::new();
    let mut paths = Vec::new();
    for &seed in adjacency.keys() {
        if consumed.contains(&seed) {
            continue;
        }
        let mut stack = vec![seed];
        let mut component = Vec::new();
        let mut component_seen = HashSet::<u32>::new();
        while let Some(vertex) = stack.pop() {
            if !component_seen.insert(vertex) {
                continue;
            }
            component.push(vertex);
            if let Some(neighbors) = adjacency.get(&vertex) {
                stack.extend(neighbors.iter().copied());
            }
        }
        consumed.extend(component.iter().copied());

        if component.len() < 3
            || component.iter().any(|vertex| {
                adjacency
                    .get(vertex)
                    .is_none_or(|neighbors| neighbors.is_empty() || neighbors.len() > 2)
            })
        {
            continue;
        }
        let endpoints = component
            .iter()
            .copied()
            .filter(|vertex| adjacency[vertex].len() == 1)
            .collect::<Vec<_>>();
        if !matches!(endpoints.len(), 0 | 2) {
            continue;
        }

        // Imported garment/skin meshes commonly duplicate vertices at UV or normal seams. After
        // clipping the neck shell this can leave a geometrically valid neckline represented as an
        // open chain rather than a topological loop. Two endpoints are therefore accepted and the
        // cap closes them directly; branched/non-manifold boundaries are rejected.
        let start = endpoints.first().copied().unwrap_or(component[0]);
        let closed = endpoints.is_empty();
        let mut ordered = Vec::with_capacity(component.len());
        let mut previous = None;
        let mut current = start;
        for _ in 0..=component.len() {
            ordered.push(current);
            let neighbors = &adjacency[&current];
            let next = neighbors
                .iter()
                .copied()
                .find(|candidate| Some(*candidate) != previous);
            let Some(next) = next else {
                break;
            };
            previous = Some(current);
            current = next;
            if closed && current == start {
                break;
            }
        }
        if ordered.len() == component.len() && (!closed || current == start) {
            paths.push(ordered);
        }
    }
    paths
}

fn append_double_sided_cap(
    mesh: &PrimitiveMesh,
    loop_vertices: &[u32],
    out_indices: &mut Vec<u32>,
) -> usize {
    if loop_vertices.len() < 6 {
        return 0;
    }

    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut center_x = 0.0_f32;
    let mut center_z = 0.0_f32;
    for &index in loop_vertices {
        let Some(vertex) = mesh.vertices.get(index as usize) else {
            return 0;
        };
        min_y = min_y.min(vertex.pos[1]);
        max_y = max_y.max(vertex.pos[1]);
        center_x += vertex.pos[0];
        center_z += vertex.pos[2];
    }
    if !min_y.is_finite() || !max_y.is_finite() || max_y - min_y > FPP_CAP_MAX_VERTICAL_SPAN_M {
        return 0;
    }
    let count = loop_vertices.len() as f32;
    center_x /= count;
    center_z /= count;
    let mut radius = 0.0_f32;
    for &index in loop_vertices {
        let vertex = &mesh.vertices[index as usize];
        let dx = vertex.pos[0] - center_x;
        let dz = vertex.pos[2] - center_z;
        radius = radius.max((dx * dx + dz * dz).sqrt());
    }
    if !(FPP_CAP_MIN_HORIZONTAL_RADIUS_M..=FPP_CAP_MAX_HORIZONTAL_RADIUS_M).contains(&radius) {
        return 0;
    }

    // A neck/collar boundary is close to convex in the horizontal plane. Preserve the manifold
    // loop order and fan-triangulate without adding vertices, so the original skin stream remains
    // valid for both world and first-person topology. Emit both windings because the cap is an
    // intentional local occluder and must not disappear under material culling policy.
    let root = loop_vertices[0];
    let mut added = 0usize;
    for pair in loop_vertices[1..].windows(2) {
        let b = pair[0];
        let c = pair[1];
        let a_pos = mesh.vertices[root as usize].pos;
        let b_pos = mesh.vertices[b as usize].pos;
        let c_pos = mesh.vertices[c as usize].pos;
        let projected_area = (b_pos[0] - a_pos[0]) * (c_pos[2] - a_pos[2])
            - (b_pos[2] - a_pos[2]) * (c_pos[0] - a_pos[0]);
        if !projected_area.is_finite() || projected_area.abs() <= 1.0e-8 {
            continue;
        }
        out_indices.extend_from_slice(&[root, b, c, root, c, b]);
        added += 2;
    }
    added
}

fn derive_first_person_owner_body_mesh(
    mesh: &PrimitiveMesh,
    skin: &ModelSkinBinding,
    skeleton: &ModelSkeletonMetadata,
) -> Option<FirstPersonOwnerBodyBuild> {
    if mesh.indices.len() < 3
        || !mesh.indices.len().is_multiple_of(3)
        || skin.vertices.len() != mesh.vertices.len()
    {
        return None;
    }
    let head_index = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.head)?;
    let root_index = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == skeleton.anchors.root);
    if root_index == Some(head_index) {
        return None;
    }

    let weights = skin
        .vertices
        .iter()
        .map(|vertex| first_person_near_head_weight(vertex, skeleton, head_index))
        .collect::<Vec<_>>();
    let triangle_count = mesh.indices.len() / 3;
    let mut removed = Vec::with_capacity(triangle_count);
    let mut removed_count = 0usize;
    for tri in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        if a >= weights.len() || b >= weights.len() || c >= weights.len() {
            return None;
        }
        let wa = weights[a];
        let wb = weights[b];
        let wc = weights[c];
        let strong_vertices = [wa, wb, wc]
            .into_iter()
            .filter(|weight| *weight >= FPP_NEAR_HEAD_VERTEX_WEIGHT)
            .count();
        let average = (wa + wb + wc) / 3.0;
        let cut = average >= FPP_NEAR_HEAD_TRIANGLE_AVERAGE_WEIGHT || strong_vertices >= 2;
        removed.push(cut);
        removed_count += usize::from(cut);
    }
    if removed_count < FPP_MIN_REMOVED_TRIANGLES
        || removed_count as f32 / triangle_count as f32 > FPP_MAX_REMOVED_TRIANGLE_FRACTION
    {
        return None;
    }

    let mut edge_domains = HashMap::<(u32, u32), u8>::new();
    let mut indices = Vec::with_capacity(mesh.indices.len());
    for (triangle_index, tri) in mesh.indices.chunks_exact(3).enumerate() {
        let domain = if removed[triangle_index] { 0b10 } else { 0b01 };
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            *edge_domains.entry(normalized_edge(a, b)).or_default() |= domain;
        }
        if !removed[triangle_index] {
            indices.extend_from_slice(tri);
        }
    }
    let boundary_edges = edge_domains
        .into_iter()
        .filter_map(|(edge, domains)| (domains == 0b11).then_some(edge))
        .collect::<Vec<_>>();
    let welded_boundary_edges = weld_boundary_edges(mesh, &boundary_edges);
    let loops = simple_boundary_paths(&welded_boundary_edges);
    let mut cap_triangles = 0usize;
    let mut cap_loops = 0usize;
    for loop_vertices in loops {
        let added = append_double_sided_cap(mesh, &loop_vertices, &mut indices);
        if added > 0 {
            cap_triangles += added;
            cap_loops += 1;
        }
    }
    // Never publish an open cut as the FPP variant: that would merely replace the original neck
    // cavity with another hole. A valid variant must both remove the camera-near shell and seal at
    // least one bounded neckline loop.
    if cap_triangles == 0 {
        return None;
    }

    Some(FirstPersonOwnerBodyBuild {
        mesh: PrimitiveMesh {
            vertices: mesh.vertices.clone(),
            indices,
            bounds_center: mesh.bounds_center,
            bounds_radius: mesh.bounds_radius,
        },
        removed_triangles: removed_count,
        cap_triangles,
        cap_loops,
    })
}

fn ensure_runtime_model_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    derive_first_person: bool,
) -> Result<
    (
        String,
        Vec<PlayerRuntimeModelPart>,
        Option<ModelSkeletonMetadata>,
    ),
    String,
> {
    let mut request = ModelAssetRequest::new(assignment.source.clone())
        .with_human_scale(assignment.target_height, assignment.eye_height_ratio);
    if let Some(properties_ref) = assignment.properties_ref.as_deref() {
        request = request.with_properties_ref(properties_ref);
    }
    if let Some(dictionary) = assignment.texture_dictionary.as_deref() {
        request = request.with_texture_dictionary(dictionary);
    }
    if let Some(skeleton) = assignment.skeleton_source.as_deref() {
        request = request.with_skeleton(skeleton);
    }

    let constructor = ModelGatewayClient::new(newengine_plugin_host::default_host_api());
    let bundle = constructor.assemble_bundle(&request)?;
    let skeleton = bundle.skeleton.clone();

    if let Some(metadata) = skeleton.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player skeleton metadata bound source='{}' skeleton='{}' format='{}' bytes={} joints={} status='{}'",
            bundle.source,
            metadata.source,
            metadata.source_format,
            metadata.byte_len,
            metadata.joints.len(),
            metadata.decode_status
        );
    }

    let mut out = Vec::with_capacity(bundle.parts.len());
    let mut registered_parts = 0usize;
    let mut registered_vertices = 0usize;
    let mut registered_indices = 0usize;
    for (part_index, part) in bundle.parts.into_iter().enumerate() {
        let skin = part.skin.clone();
        let first_person_build = derive_first_person.then(|| ()).and_then(|_| {
            skin.as_ref().and_then(|skin| {
                skeleton.as_ref().and_then(|skeleton| {
                    derive_first_person_owner_body_mesh(&part.mesh, skin, skeleton)
                })
            })
        });
        // A character may legitimately have multiple geometries using the same material slot
        // (a character may legitimately have multiple geometries for one material slot). Part index is therefore part of the stable mesh id.
        let primitive_id = PrimitiveId(fnv1a_64(&format!(
            "player-model:{}:revision={}:{}:{}",
            bundle.source, assignment.revision, part_index, part.material_slot
        )));
        let first_person_primitive_id = first_person_build.as_ref().map(|_| {
            PrimitiveId(fnv1a_64(&format!(
                "player-model-fpp-owner-body:{}:revision={}:{}:{}",
                bundle.source, assignment.revision, part_index, part.material_slot
            )))
        });
        let material_name = part
            .material
            .material_ref
            .clone()
            .unwrap_or_else(|| format!("Player/Avatar/{}", part.material_slot));
        let material_id = mats.upsert_named_with_textures(
            &material_name,
            part.material.descriptor,
            part.material.textures.clone().sanitized(),
        );
        if !prims.is_registered(primitive_id) {
            let vertex_count = part.mesh.vertices.len();
            let index_count = part.mesh.indices.len();
            prims.register_mesh(
                primitive_id,
                format!(
                    "PlayerModel/Part{}:{} ({})",
                    part_index, part.material_slot, bundle.source
                ),
                part.mesh,
            );
            registered_parts += 1;
            registered_vertices += vertex_count;
            registered_indices += index_count;
            newengine_ulog_api::ulog::debug!(
                "game-ready: player model part registered source='{}' part={} slot='{}' vertices={} indices={} material='{}' policy='ydd->nemat->ytd'",
                bundle.source,
                part_index,
                part.material_slot,
                vertex_count,
                index_count,
                material_name
            );
        }
        if let (Some(first_person_id), Some(first_person)) =
            (first_person_primitive_id, first_person_build)
        {
            if !prims.is_registered(first_person_id) {
                let first_person_indices = first_person.mesh.indices.len();
                prims.register_mesh(
                    first_person_id,
                    format!(
                        "PlayerModel/FPP/Part{}:{} ({})",
                        part_index, part.material_slot, bundle.source
                    ),
                    first_person.mesh,
                );
                newengine_ulog_api::ulog::info!(
                    "game-ready: first-person owner-body topology derived source='{}' part={} slot='{}' world_primitive={} fpp_primitive={} removed_triangles={} cap_triangles={} cap_loops={} fpp_indices={} policy='chest-visible neck-shell-removed neckline-sealed same-skin-stream'",
                    bundle.source,
                    part_index,
                    part.material_slot,
                    primitive_id.0,
                    first_person_id.0,
                    first_person.removed_triangles,
                    first_person.cap_triangles,
                    first_person.cap_loops,
                    first_person_indices,
                );
            }
        }

        out.push(PlayerRuntimeModelPart {
            source_mesh_name: part.source_mesh_name,
            primitive_id,
            first_person_primitive_id,
            material_id,
            material_slot: part.material_slot,
            color: part.material.fallback_color,
            skin,
        });
    }

    if registered_parts > 0 {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model registered source='{}' parts={} vertices={} indices={} materials={}",
            bundle.source,
            registered_parts,
            registered_vertices,
            registered_indices,
            out.len(),
        );
    }

    if let Some(dictionary) = bundle.texture_dictionary.as_deref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model texture dictionary bound source='{}' dictionary='{}' materials={}",
            bundle.source,
            dictionary,
            out.len()
        );
    }

    if let Some(properties_ref) = bundle.properties_ref.as_deref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model properties descriptor bound source='{}' properties_ref='{}' policy='.ydd slots -> .ytyp material bindings -> .nemat/.ytd'",
            bundle.source,
            properties_ref
        );
    }

    if !bundle.collisions.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: player model collision bindings derived source='{}' collisions={}",
            bundle.source,
            bundle.collisions.len()
        );
    }

    Ok((bundle.source, out, skeleton))
}

pub(super) fn ensure_player_runtime_model_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
) -> Result<
    (
        String,
        Vec<PlayerRuntimeModelPart>,
        Option<ModelSkeletonMetadata>,
    ),
    String,
> {
    ensure_runtime_model_parts(prims, mats, assignment, true)
}

pub(super) fn ensure_player_runtime_sidecar_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    definition: &newengine_engine_runtime::gameplay::PlayerSkinSidecarDefinition,
) -> Result<
    (
        String,
        Vec<PlayerRuntimeModelPart>,
        Option<ModelSkeletonMetadata>,
    ),
    String,
> {
    let mut sidecar_assignment = assignment.clone();
    sidecar_assignment.source = definition.model.clone();
    sidecar_assignment.skeleton_source = Some(definition.skeleton.clone());
    sidecar_assignment.properties_ref = None;
    ensure_runtime_model_parts(prims, mats, &sidecar_assignment, false)
}

#[cfg(test)]
mod first_person_owner_body_tests {
    use super::*;
    use newengine_model_skeleton_api::{
        skeleton_joint_indexed, ModelSkeletonAnchors, ModelSkeletonMetadata,
    };
    use newengine_primitives::PrimitiveVertex;

    fn test_skeleton() -> ModelSkeletonMetadata {
        let joints = vec![
            skeleton_joint_indexed(0, 0, "root", None::<String>, None, [0.0, 0.0, 0.0]),
            skeleton_joint_indexed(1, 1, "spine", Some("root"), Some(0), [0.0, 0.2, 0.0]),
            skeleton_joint_indexed(2, 2, "neck", Some("spine"), Some(1), [0.0, 0.2, 0.0]),
            skeleton_joint_indexed(3, 3, "head", Some("neck"), Some(2), [0.0, 0.1, 0.0]),
        ];
        ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: String::new(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints,
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "head".to_owned(),
                left_hand: "root".to_owned(),
                right_hand: "root".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "head".to_owned(),
                eye_height: 1.6,
            },
        }
    }

    fn ring_vertex(angle: f32, y: f32) -> PrimitiveVertex {
        PrimitiveVertex {
            pos: [angle.cos() * 0.12, y, angle.sin() * 0.12],
            nrm: [angle.cos(), 0.0, angle.sin()],
            uv: [angle / core::f32::consts::TAU, y],
        }
    }

    #[test]
    fn derived_fpp_body_keeps_vertex_skin_contract_and_seals_neckline() {
        const RING: usize = 8;
        const RINGS: usize = 5;
        let mut vertices = Vec::new();
        let mut skin_vertices = Vec::new();
        for ring in 0..RINGS {
            for i in 0..RING {
                let angle = i as f32 / RING as f32 * core::f32::consts::TAU;
                vertices.push(ring_vertex(angle, ring as f32 * 0.12));
                let near_head = ring == RINGS - 1;
                skin_vertices.push(ModelSkinVertex {
                    joints: [if near_head { 3 } else { 1 }, 0, 0, 0],
                    weights: [1.0, 0.0, 0.0, 0.0],
                    joints_extra: [0; 4],
                    weights_extra: [0.0; 4],
                });
            }
        }
        let mut indices = Vec::new();
        for ring in 0..RINGS - 1 {
            for i in 0..RING {
                let next = (i + 1) % RING;
                let a = (ring * RING + i) as u32;
                let b = (ring * RING + next) as u32;
                let c = ((ring + 1) * RING + i) as u32;
                let d = ((ring + 1) * RING + next) as u32;
                indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        let mesh = PrimitiveMesh {
            vertices,
            indices,
            bounds_center: Vec3::new(0.0, 0.24, 0.0),
            bounds_radius: 0.6,
        };
        let skin = ModelSkinBinding {
            vertices: skin_vertices,
            ..ModelSkinBinding::default()
        };
        let built = derive_first_person_owner_body_mesh(&mesh, &skin, &test_skeleton())
            .expect("mixed torso/neck mesh should get a sealed FPP topology");

        assert_eq!(built.mesh.vertices.len(), mesh.vertices.len());
        assert!(built.removed_triangles >= FPP_MIN_REMOVED_TRIANGLES);
        assert!(built.cap_triangles > 0);
        assert!(built.cap_loops > 0);
        assert!(built
            .mesh
            .indices
            .iter()
            .all(|index| (*index as usize) < built.mesh.vertices.len()));
    }
}
