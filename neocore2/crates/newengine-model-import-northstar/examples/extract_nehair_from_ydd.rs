use std::{
    collections::{HashMap, VecDeque},
    env, fs,
    path::PathBuf,
};

use newengine_asset_format_nef8::ydd_binary::{YddBinaryMesh, YddBinarySkinVertex};
use newengine_asset_format_nehair::{
    compile_authored_groom_json, decode_nehair, encode_nehair_v1, AuthoredHairCollisionCapsuleV1,
    AuthoredHairGroomV1, AuthoredHairGuidePointV1, AuthoredHairGuideStrandV1,
};
use newengine_math::{Mat4, Vec3};
use newengine_model_skeleton_api::{
    ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
};

#[derive(Clone, Debug)]
struct Triangle {
    vertices: [u32; 3],
}

fn main() -> Result<(), String> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 6 {
        return Err(
            "usage: extract_nehair_from_ydd <hair-or-character.ydd> <skeleton.ymt> <source.groom.json> <output.nehair> <logical-groom-ref> [mesh-prefixes] [followers]"
                .to_owned(),
        );
    }
    let ydd_path = PathBuf::from(&args[1]);
    let ymt_path = PathBuf::from(&args[2]);
    let source_json_path = PathBuf::from(&args[3]);
    let output_path = PathBuf::from(&args[4]);
    let logical_groom_ref = args[5].trim().replace('\\', "/");
    let prefixes = args
        .get(6)
        .map(|raw| {
            raw.split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| {
            vec![
                "main_hair_".to_owned(),
                "wet_strands_".to_owned(),
                "transitions_".to_owned(),
            ]
        });
    let followers = args
        .get(7)
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(3)
        .min(16);

    let skeleton = load_skeleton_metadata(&ymt_path)?;
    let bytes = fs::read(&ydd_path).map_err(|e| format!("read {}: {e}", ydd_path.display()))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        ydd_path.to_string_lossy().as_ref(),
    )?;
    let document = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;

    let mut authored = AuthoredHairGroomV1 {
        groom: Some(logical_groom_ref.clone()),
        guide_points: Vec::new(),
        guide_strands: Vec::new(),
        collision_capsules: Vec::<AuthoredHairCollisionCapsuleV1>::new(),
        follow_strands_per_guide: followers,
    };
    let mut selected_meshes = 0usize;
    let mut accepted_components = 0usize;
    let mut rejected_components = 0usize;

    for entry in &document.entries {
        for (mesh_index, mesh) in entry.meshes.iter().enumerate() {
            let lower = mesh.name.to_ascii_lowercase();
            if !prefixes.iter().any(|prefix| lower.starts_with(prefix)) {
                continue;
            }
            selected_meshes += 1;
            let Some(skin) = mesh.skin.as_deref() else {
                eprintln!("skip unskinned selected hair mesh '{}'", mesh.name);
                continue;
            };
            if skin.len() != mesh.vertices.len() {
                return Err(format!(
                    "hair source mesh '{}' skin/vertex count mismatch {} != {}",
                    mesh.name,
                    skin.len(),
                    mesh.vertices.len()
                ));
            }
            let source_to_model = entry.skin_source_to_model.ok_or_else(|| {
                format!(
                    "selected skinned hair mesh '{}' belongs to YDD entry '{}' without skin_source_to_model",
                    mesh.name, entry.name
                )
            })?;
            let (accepted, rejected) = append_mesh_guides(
                &mut authored,
                mesh,
                skin,
                &skeleton,
                source_to_model,
                mesh_index.min(u16::MAX as usize) as u16,
            )?;
            accepted_components += accepted;
            rejected_components += rejected;
        }
    }

    if selected_meshes == 0 {
        return Err(format!(
            "no YDD meshes matched prefixes {:?} in {}",
            prefixes,
            ydd_path.display()
        ));
    }
    if authored.guide_strands.is_empty() {
        return Err("hair-card extraction produced zero valid guide strands".to_owned());
    }

    let source_json = serde_json::to_vec_pretty(&authored)
        .map_err(|e| format!("serialize authored groom JSON: {e}"))?;
    let compiled = compile_authored_groom_json(&source_json, &logical_groom_ref, &skeleton)?;
    let binary = encode_nehair_v1(&compiled)?;
    let roundtrip = decode_nehair(&binary)?;
    if roundtrip != compiled {
        return Err("NEHAIR encode/decode roundtrip mismatch".to_owned());
    }

    if let Some(parent) = source_json_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    write_atomic(&source_json_path, &source_json)?;
    write_atomic(&output_path, &binary)?;

    println!("source_ydd={}", ydd_path.display());
    println!("skeleton_ymt={}", ymt_path.display());
    println!("selected_meshes={selected_meshes}");
    println!("accepted_components={accepted_components}");
    println!("rejected_components={rejected_components}");
    println!("guide_strands={}", compiled.guide_strands.len());
    println!("guide_points={}", compiled.guide_points.len());
    println!("guide_segments={}", compiled.guide_segment_count());
    println!("followers_per_guide={}", compiled.follow_strands_per_guide);
    println!("source_json={}", source_json_path.display());
    println!("output_nehair={}", output_path.display());
    println!("output_bytes={}", binary.len());
    Ok(())
}

fn append_mesh_guides(
    authored: &mut AuthoredHairGroomV1,
    mesh: &YddBinaryMesh,
    skin: &[YddBinarySkinVertex],
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    group: u16,
) -> Result<(usize, usize), String> {
    let triangles = valid_triangles(mesh)?;
    if triangles.is_empty() {
        return Ok((0, 0));
    }
    let adjacency = triangle_adjacency(&triangles);
    let components = connected_components(&adjacency);
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for component in components {
        if component.len() < 4 {
            rejected += 1;
            continue;
        }
        let diameter = graph_diameter_path(&component, &adjacency);
        if diameter.len() < 4 {
            rejected += 1;
            continue;
        }
        // Highly branching surface patches are poor strand sources. A card/strip's dual-graph
        // diameter consumes a meaningful fraction of its triangles even when mildly curved.
        if diameter.len() * 100 < component.len() * 28 {
            rejected += 1;
            continue;
        }

        let source_to_model = Mat4::from_cols_array(&source_to_model);
        let mut centers = diameter
            .iter()
            .map(|&tri| {
                let source = triangle_centroid(mesh, &triangles[tri]);
                let model =
                    source_to_model.transform_point3(Vec3::new(source[0], source[1], source[2]));
                [model.x, model.y, model.z]
            })
            .collect::<Vec<_>>();
        let total_length = polyline_length(&centers);
        if !total_length.is_finite() || !(0.012..=2.5).contains(&total_length) {
            rejected += 1;
            continue;
        }

        let root_joint_index = dominant_root_joint(&diameter, &triangles, skin, false);
        let tip_joint_index = dominant_root_joint(&diameter, &triangles, skin, true);
        let root_score = endpoint_joint_score(&diameter, &triangles, skin, root_joint_index, false);
        let tip_score = endpoint_joint_score(&diameter, &triangles, skin, tip_joint_index, true);

        // Compare each endpoint against the joint that best anchors its own neighborhood.
        // Card roots are normally more rigidly weighted than tips. The tie-breaker uses UV-v,
        // which is stable for the source cards but is deliberately secondary to skin semantics.
        let reverse = if (root_score - tip_score).abs() > 1.0e-5 {
            tip_score > root_score
        } else {
            endpoint_uv_v(mesh, &triangles[*diameter.first().unwrap()])
                > endpoint_uv_v(mesh, &triangles[*diameter.last().unwrap()])
        };
        let mut path = diameter;
        if reverse {
            path.reverse();
            centers.reverse();
        }

        let root_joint = dominant_root_joint(&path, &triangles, skin, false);
        let Some(root_joint_name) = skeleton
            .joints
            .get(root_joint as usize)
            .map(|joint| joint.name.clone())
        else {
            rejected += 1;
            continue;
        };
        let point_count = ((total_length / 0.035).ceil() as usize + 1).clamp(4, 12);
        let sampled = resample_polyline(&centers, point_count);
        if sampled.len() < 2 {
            rejected += 1;
            continue;
        }
        let first_point = authored.guide_points.len();
        if first_point > u32::MAX as usize {
            return Err("authored groom guide point index exceeds u32".to_owned());
        }
        for (index, point) in sampled.iter().enumerate() {
            let inverse_mass = match index {
                0 => 0.0,
                1 => 0.35,
                _ => 1.0,
            };
            authored.guide_points.push(AuthoredHairGuidePointV1 {
                rest_position: *point,
                inverse_mass,
            });
        }
        let root_uv = triangle_uv_centroid(mesh, &triangles[path[0]]);
        authored.guide_strands.push(AuthoredHairGuideStrandV1 {
            first_point: first_point as u32,
            point_count: sampled.len() as u16,
            group,
            root_uv,
            root_joint: root_joint_name,
        });
        accepted += 1;
    }
    Ok((accepted, rejected))
}

fn valid_triangles(mesh: &YddBinaryMesh) -> Result<Vec<Triangle>, String> {
    if !mesh.indices.len().is_multiple_of(3) {
        return Err(format!(
            "mesh '{}' index count is not divisible by 3",
            mesh.name
        ));
    }
    let mut out = Vec::with_capacity(mesh.indices.len() / 3);
    for tri in mesh.indices.as_chunks::<3>().0 {
        if tri
            .iter()
            .any(|&index| index as usize >= mesh.vertices.len())
        {
            return Err(format!(
                "mesh '{}' contains out-of-range triangle index",
                mesh.name
            ));
        }
        if tri[0] == tri[1] || tri[1] == tri[2] || tri[0] == tri[2] {
            continue;
        }
        out.push(Triangle {
            vertices: [tri[0], tri[1], tri[2]],
        });
    }
    Ok(out)
}

fn triangle_adjacency(triangles: &[Triangle]) -> Vec<Vec<usize>> {
    let mut edge_owner = HashMap::<(u32, u32), usize>::new();
    let mut adjacency = vec![Vec::new(); triangles.len()];
    for (index, tri) in triangles.iter().enumerate() {
        for (a, b) in [
            (tri.vertices[0], tri.vertices[1]),
            (tri.vertices[1], tri.vertices[2]),
            (tri.vertices[2], tri.vertices[0]),
        ] {
            let edge = if a < b { (a, b) } else { (b, a) };
            if let Some(&other) = edge_owner.get(&edge) {
                adjacency[index].push(other);
                adjacency[other].push(index);
            } else {
                edge_owner.insert(edge, index);
            }
        }
    }
    adjacency
}

fn connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut visited = vec![false; adjacency.len()];
    let mut out = Vec::new();
    for seed in 0..adjacency.len() {
        if visited[seed] {
            continue;
        }
        visited[seed] = true;
        let mut queue = VecDeque::from([seed]);
        let mut component = Vec::new();
        while let Some(node) = queue.pop_front() {
            component.push(node);
            for &next in &adjacency[node] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        out.push(component);
    }
    out
}

fn graph_diameter_path(component: &[usize], adjacency: &[Vec<usize>]) -> Vec<usize> {
    let mut allowed = vec![false; adjacency.len()];
    for &node in component {
        allowed[node] = true;
    }
    let a = bfs_farthest(component[0], adjacency, &allowed).0;
    let (b, predecessor) = bfs_farthest(a, adjacency, &allowed);
    let mut path = vec![b];
    let mut current = b;
    while current != a {
        let Some(parent) = predecessor[current] else {
            break;
        };
        current = parent;
        path.push(current);
    }
    path.reverse();
    path
}

fn bfs_farthest(
    seed: usize,
    adjacency: &[Vec<usize>],
    allowed: &[bool],
) -> (usize, Vec<Option<usize>>) {
    let mut distance = vec![usize::MAX; adjacency.len()];
    let mut predecessor = vec![None; adjacency.len()];
    let mut queue = VecDeque::from([seed]);
    distance[seed] = 0;
    let mut farthest = seed;
    while let Some(node) = queue.pop_front() {
        if distance[node] > distance[farthest] {
            farthest = node;
        }
        for &next in &adjacency[node] {
            if allowed[next] && distance[next] == usize::MAX {
                distance[next] = distance[node] + 1;
                predecessor[next] = Some(node);
                queue.push_back(next);
            }
        }
    }
    (farthest, predecessor)
}

fn triangle_centroid(mesh: &YddBinaryMesh, tri: &Triangle) -> [f32; 3] {
    let mut out = [0.0_f32; 3];
    for &index in &tri.vertices {
        let p = mesh.vertices[index as usize].position;
        for axis in 0..3 {
            out[axis] += p[axis] / 3.0;
        }
    }
    out
}

fn triangle_uv_centroid(mesh: &YddBinaryMesh, tri: &Triangle) -> [f32; 2] {
    let mut out = [0.0_f32; 2];
    for &index in &tri.vertices {
        let uv = mesh.vertices[index as usize].uv0;
        out[0] += uv[0] / 3.0;
        out[1] += uv[1] / 3.0;
    }
    out
}

fn endpoint_uv_v(mesh: &YddBinaryMesh, tri: &Triangle) -> f32 {
    triangle_uv_centroid(mesh, tri)[1]
}

fn joint_weight(vertex: &YddBinarySkinVertex, joint: u16) -> f32 {
    vertex
        .joints
        .iter()
        .copied()
        .zip(vertex.weights.iter().copied())
        .chain(
            vertex
                .joints_extra
                .iter()
                .copied()
                .zip(vertex.weights_extra.iter().copied()),
        )
        .filter_map(|(candidate, weight)| (candidate == joint && weight > 0.0).then_some(weight))
        .sum()
}

fn dominant_root_joint(
    path: &[usize],
    triangles: &[Triangle],
    skin: &[YddBinarySkinVertex],
    from_end: bool,
) -> u16 {
    let window = ((path.len() as f32 * 0.22).ceil() as usize).clamp(1, path.len());
    let range: Box<dyn Iterator<Item = &usize>> = if from_end {
        Box::new(path.iter().rev().take(window))
    } else {
        Box::new(path.iter().take(window))
    };
    let mut weights = HashMap::<u16, f32>::new();
    for &tri_index in range {
        for &vertex_index in &triangles[tri_index].vertices {
            let vertex = &skin[vertex_index as usize];
            for (joint, weight) in vertex
                .joints
                .iter()
                .copied()
                .zip(vertex.weights.iter().copied())
                .chain(
                    vertex
                        .joints_extra
                        .iter()
                        .copied()
                        .zip(vertex.weights_extra.iter().copied()),
                )
            {
                if weight.is_finite() && weight > 0.0 {
                    *weights.entry(joint).or_default() += weight;
                }
            }
        }
    }
    weights
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(joint, _)| joint)
        .unwrap_or(0)
}

fn endpoint_joint_score(
    path: &[usize],
    triangles: &[Triangle],
    skin: &[YddBinarySkinVertex],
    joint: u16,
    from_end: bool,
) -> f32 {
    let window = ((path.len() as f32 * 0.16).ceil() as usize).clamp(1, path.len());
    let iter: Box<dyn Iterator<Item = &usize>> = if from_end {
        Box::new(path.iter().rev().take(window))
    } else {
        Box::new(path.iter().take(window))
    };
    let mut score = 0.0;
    let mut samples = 0usize;
    for &tri_index in iter {
        for &vertex_index in &triangles[tri_index].vertices {
            score += joint_weight(&skin[vertex_index as usize], joint);
            samples += 1;
        }
    }
    if samples == 0 {
        0.0
    } else {
        score / samples as f32
    }
}

fn polyline_length(points: &[[f32; 3]]) -> f32 {
    points
        .windows(2)
        .map(|pair| distance(pair[0], pair[1]))
        .sum()
}

fn resample_polyline(points: &[[f32; 3]], count: usize) -> Vec<[f32; 3]> {
    if points.len() < 2 || count < 2 {
        return points.to_vec();
    }
    let mut cumulative = Vec::with_capacity(points.len());
    cumulative.push(0.0_f32);
    for pair in points.windows(2) {
        cumulative.push(cumulative.last().copied().unwrap_or(0.0) + distance(pair[0], pair[1]));
    }
    let total = *cumulative.last().unwrap_or(&0.0);
    if total <= 1.0e-7 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(count);
    let mut segment = 0usize;
    for sample in 0..count {
        let target = total * sample as f32 / (count - 1) as f32;
        while segment + 1 < cumulative.len() - 1 && cumulative[segment + 1] < target {
            segment += 1;
        }
        let a = cumulative[segment];
        let b = cumulative[segment + 1];
        let t = if b > a { (target - a) / (b - a) } else { 0.0 };
        out.push(lerp(
            points[segment],
            points[segment + 1],
            t.clamp(0.0, 1.0),
        ));
    }
    // One low-cost smoothing pass removes triangle-centroid zig-zag without moving roots/tips.
    if out.len() > 3 {
        let original = out.clone();
        for index in 1..out.len() - 1 {
            for axis in 0..3 {
                out[index][axis] = original[index - 1][axis] * 0.2
                    + original[index][axis] * 0.6
                    + original[index + 1][axis] * 0.2;
            }
        }
    }
    out
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn load_skeleton_metadata(path: &PathBuf) -> Result<ModelSkeletonMetadata, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YMT,
        path.to_string_lossy().as_ref(),
    )?;
    let text = String::from_utf8(decoded.body).map_err(|e| format!("YMT body UTF-8: {e}"))?;
    let document = roxmltree::Document::parse(&text).map_err(|e| format!("YMT XML parse: {e}"))?;
    let skeleton = document
        .descendants()
        .find(|node| node.has_tag_name("Skeleton"))
        .ok_or("YMT contains no Skeleton")?;
    let source_format = skeleton
        .attribute("source_format")
        .unwrap_or("newengine.ymt.skeleton.v1");
    let mut joints = Vec::new();
    for node in skeleton
        .children()
        .filter(|node| node.has_tag_name("Joint"))
    {
        let raw_index = attr_i32(node, "index", joints.len() as i32)?;
        if raw_index < 0 {
            return Err("YMT joint index cannot be negative".to_owned());
        }
        let parent_index = attr_i32(node, "parent_index", -1)?;
        joints.push(ModelSkeletonJointMetadata {
            index: raw_index as u32,
            tag: attr_i32(node, "tag", 0)?.max(0) as u32,
            name: required_attr(node, "name")?.to_owned(),
            parent: node.attribute("parent").map(str::to_owned),
            parent_index: (parent_index >= 0).then_some(parent_index as u32),
            position_ls: [
                attr_f32(node, "tx", 0.0)?,
                attr_f32(node, "ty", 0.0)?,
                attr_f32(node, "tz", 0.0)?,
            ],
            rotation_ls: [
                attr_f32(node, "qx", 0.0)?,
                attr_f32(node, "qy", 0.0)?,
                attr_f32(node, "qz", 0.0)?,
                attr_f32(node, "qw", 1.0)?,
            ],
            scale_ls: [
                attr_f32(node, "sx", 1.0)?,
                attr_f32(node, "sy", 1.0)?,
                attr_f32(node, "sz", 1.0)?,
            ],
            flags: node
                .attribute("flags")
                .map(|raw| {
                    raw.split(',')
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        });
    }
    joints.sort_by_key(|joint| joint.index);
    for (expected, joint) in joints.iter().enumerate() {
        if joint.index as usize != expected {
            return Err(format!(
                "YMT skeleton joint table is not dense at expected={} actual={}",
                expected, joint.index
            ));
        }
    }
    let anchors = skeleton
        .children()
        .find(|node| node.has_tag_name("Anchors"))
        .ok_or("YMT Skeleton contains no Anchors")?;
    let anchors = ModelSkeletonAnchors {
        root: required_attr(anchors, "root")?.to_owned(),
        hips: required_attr(anchors, "hips")?.to_owned(),
        head: required_attr(anchors, "head")?.to_owned(),
        left_hand: required_attr(anchors, "left_hand")?.to_owned(),
        right_hand: required_attr(anchors, "right_hand")?.to_owned(),
        left_foot: required_attr(anchors, "left_foot")?.to_owned(),
        right_foot: required_attr(anchors, "right_foot")?.to_owned(),
        eye: required_attr(anchors, "eye")?.to_owned(),
        eye_height: attr_f32(anchors, "eye_height", 0.0)?,
    };
    Ok(ModelSkeletonMetadata {
        source: path.to_string_lossy().replace('\\', "/"),
        source_format: source_format.to_owned(),
        container_magic: "NEF8".to_owned(),
        byte_len: bytes.len(),
        content_hash: format!("blake3:{}", blake3::hash(text.as_bytes()).to_hex()),
        decode_status: "offline NEHAIR compiler decoded authored YMT skeleton".to_owned(),
        joints,
        anchors,
    })
}

fn required_attr<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    key: &str,
) -> Result<&'a str, String> {
    node.attribute(key)
        .ok_or_else(|| format!("YMT <{}> missing '{}'", node.tag_name().name(), key))
}

fn attr_i32(node: roxmltree::Node<'_, '_>, key: &str, default: i32) -> Result<i32, String> {
    node.attribute(key)
        .map(|raw| {
            raw.parse::<i32>()
                .map_err(|e| format!("YMT {}='{}': {e}", key, raw))
        })
        .unwrap_or(Ok(default))
}

fn attr_f32(node: roxmltree::Node<'_, '_>, key: &str, default: f32) -> Result<f32, String> {
    let value = node
        .attribute(key)
        .map(|raw| {
            raw.parse::<f32>()
                .map_err(|e| format!("YMT {}='{}': {e}", key, raw))
        })
        .unwrap_or(Ok(default))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("YMT {key} is non-finite"))
    }
}

fn write_atomic(path: &PathBuf, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|v| v.to_str()).unwrap_or("asset")
    ));
    fs::write(&temp, bytes).map_err(|e| format!("write {}: {e}", temp.display()))?;
    fs::rename(&temp, path)
        .map_err(|e| format!("rename {} -> {}: {e}", temp.display(), path.display()))
}
