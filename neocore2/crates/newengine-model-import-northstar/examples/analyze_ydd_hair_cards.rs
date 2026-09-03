use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    env, fs,
};

fn edge(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: analyze_ydd_hair_cards FILE.ydd [logical]")?;
    let logical = env::args().nth(2).unwrap_or_else(|| "model.ydd".to_owned());
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        &logical,
    )?;
    let doc = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;

    for entry in doc.entries {
        println!(
            "ENTRY '{}' source_to_model={:?}",
            entry.name, entry.skin_source_to_model
        );
        for (mesh_index, mesh) in entry.meshes.iter().enumerate() {
            let mut uv_min = [f32::INFINITY; 2];
            let mut uv_max = [f32::NEG_INFINITY; 2];
            for v in &mesh.vertices {
                for axis in 0..2 {
                    uv_min[axis] = uv_min[axis].min(v.uv0[axis]);
                    uv_max[axis] = uv_max[axis].max(v.uv0[axis]);
                }
            }

            let tri_count = mesh.indices.len() / 3;
            let mut vertex_tris = vec![Vec::<usize>::new(); mesh.vertices.len()];
            let mut edge_counts = HashMap::<(u32, u32), usize>::new();
            for tri in 0..tri_count {
                let a = mesh.indices[tri * 3];
                let b = mesh.indices[tri * 3 + 1];
                let c = mesh.indices[tri * 3 + 2];
                for &v in &[a, b, c] {
                    if let Some(list) = vertex_tris.get_mut(v as usize) {
                        list.push(tri);
                    }
                }
                *edge_counts.entry(edge(a, b)).or_default() += 1;
                *edge_counts.entry(edge(b, c)).or_default() += 1;
                *edge_counts.entry(edge(c, a)).or_default() += 1;
            }
            let boundary_edges = edge_counts.values().filter(|&&n| n == 1).count();

            let mut visited = vec![false; tri_count];
            let mut component_sizes = Vec::new();
            for seed in 0..tri_count {
                if visited[seed] {
                    continue;
                }
                let mut q = VecDeque::from([seed]);
                visited[seed] = true;
                let mut count = 0usize;
                while let Some(t) = q.pop_front() {
                    count += 1;
                    let tri_vertices = [
                        mesh.indices[t * 3],
                        mesh.indices[t * 3 + 1],
                        mesh.indices[t * 3 + 2],
                    ];
                    for &v in &tri_vertices {
                        for &other in &vertex_tris[v as usize] {
                            if !visited[other] {
                                visited[other] = true;
                                q.push_back(other);
                            }
                        }
                    }
                }
                component_sizes.push(count);
            }
            component_sizes.sort_unstable_by(|a, b| b.cmp(a));

            let mut dominant = BTreeMap::<u16, usize>::new();
            let mut weighted = BTreeMap::<u16, f64>::new();
            if let Some(skin) = mesh.skin.as_ref() {
                for vertex in skin {
                    let mut best_joint = 0u16;
                    let mut best_weight = -1.0f32;
                    for (&joint, &weight) in vertex
                        .joints
                        .iter()
                        .zip(vertex.weights.iter())
                        .chain(vertex.joints_extra.iter().zip(vertex.weights_extra.iter()))
                    {
                        if weight > best_weight {
                            best_joint = joint;
                            best_weight = weight;
                        }
                        if weight > 0.0 {
                            *weighted.entry(joint).or_default() += weight as f64;
                        }
                    }
                    *dominant.entry(best_joint).or_default() += 1;
                }
            }
            let mut dominant_sorted = dominant.into_iter().collect::<Vec<_>>();
            dominant_sorted.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            let mut weighted_sorted = weighted.into_iter().collect::<Vec<_>>();
            weighted_sorted
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let unique_indices = mesh.indices.iter().copied().collect::<HashSet<_>>().len();
            println!(
                "MESH {mesh_index} '{}' verts={} indexed_verts={} tris={} components={} largest_components={:?} boundary_edges={} uv=({:.4},{:.4})..({:.4},{:.4}) skinned={} dominant_joints={:?} weighted_joints={:?}",
                mesh.name,
                mesh.vertices.len(), unique_indices, tri_count, component_sizes.len(),
                component_sizes.iter().take(12).copied().collect::<Vec<_>>(), boundary_edges,
                uv_min[0],uv_min[1],uv_max[0],uv_max[1], mesh.is_skinned(),
                dominant_sorted.iter().take(12).copied().collect::<Vec<_>>(),
                weighted_sorted.iter().take(12).copied().collect::<Vec<_>>(),
            );
        }
    }
    Ok(())
}
