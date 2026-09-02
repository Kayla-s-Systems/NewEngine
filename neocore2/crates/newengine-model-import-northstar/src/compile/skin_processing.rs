use super::*;

pub(super) fn validate_rigid_source_to_model(matrix: [f32; 16]) -> Result<Mat4, String> {
    if matrix.iter().any(|value| !value.is_finite()) {
        return Err("source_to_model contains non-finite values".to_owned());
    }
    let transform = Mat4::from_cols_array(&matrix);
    let x = transform.transform_vector3(Vec3::X);
    let y = transform.transform_vector3(Vec3::Y);
    let z = transform.transform_vector3(Vec3::Z);
    let epsilon = 2.0e-4;
    for (label, axis) in [("x", x), ("y", y), ("z", z)] {
        if (axis.length() - 1.0).abs() > epsilon {
            return Err(format!(
                "source_to_model must be rigid: {label}-axis length={}",
                axis.length()
            ));
        }
    }
    if x.dot(y).abs() > epsilon || x.dot(z).abs() > epsilon || y.dot(z).abs() > epsilon {
        return Err("source_to_model basis is not orthogonal".to_owned());
    }
    let origin = transform.transform_point3(Vec3::ZERO);
    if !origin.is_finite() {
        return Err("source_to_model translation is non-finite".to_owned());
    }
    Ok(transform)
}

pub(super) fn transform_mesh_to_model_space(
    mesh: &mut crate::geometry::ImportMesh,
    transform: Mat4,
) -> Result<(), String> {
    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in &mut mesh.vertices {
        let source_position = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
        let source_normal = Vec3::new(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
        let position = transform.transform_point3(source_position);
        let normal = transform
            .transform_vector3(source_normal)
            .normalize_or_zero();
        if !position.is_finite() || !normal.is_finite() || normal.length_squared() <= 1.0e-10 {
            return Err(format!(
                "source_to_model produced invalid vertex mesh='{}'",
                mesh.name
            ));
        }
        vertex.position = [position.x, position.y, position.z];
        vertex.normal = [normal.x, normal.y, normal.z];
        bounds_min = bounds_min.min(position);
        bounds_max = bounds_max.max(position);
    }
    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return Err(format!(
            "source_to_model produced invalid bounds mesh='{}'",
            mesh.name
        ));
    }
    mesh.bounds_min = [bounds_min.x, bounds_min.y, bounds_min.z];
    mesh.bounds_max = [bounds_max.x, bounds_max.y, bounds_max.z];
    Ok(())
}

pub(super) fn resolve_master_fallback_joints(
    skeleton: &DecodedSkeleton,
    names: &[String],
    package: &Path,
) -> Result<Vec<u16>, String> {
    if names.is_empty() {
        return Err(format!(
            "non-skeletal skin fallback has no master joints package='{}'",
            package.display()
        ));
    }
    let mut resolved = Vec::with_capacity(names.len());
    for name in names {
        let name = name.trim();
        if name.is_empty() {
            return Err(format!(
                "non-skeletal skin fallback contains an empty master joint package='{}'",
                package.display()
            ));
        }
        let index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .ok_or_else(|| {
                format!(
                    "non-skeletal skin fallback master joint not found package='{}' joint='{name}'",
                    package.display()
                )
            })?;
        let index = u16::try_from(index).map_err(|_| {
            format!(
                "non-skeletal skin fallback joint index exceeds u16 package='{}' joint='{name}' index={index}",
                package.display()
            )
        })?;
        if !resolved.contains(&index) {
            resolved.push(index);
        }
    }
    Ok(resolved)
}

pub(super) fn rebind_mesh_skin_to_master_joints(
    mesh: &mut crate::geometry::ImportMesh,
    skeleton_globals: &[Mat4],
    joint_indices: &[u16],
) -> Result<(), String> {
    let Some(source_skin) = mesh.skin.as_ref() else {
        return Err(format!(
            "non-skeletal skin fallback requested for unskinned mesh='{}'",
            mesh.name
        ));
    };
    if source_skin.len() != mesh.vertices.len() {
        return Err(format!(
            "non-skeletal skin fallback vertex/skin mismatch mesh='{}' vertices={} skin={}",
            mesh.name,
            mesh.vertices.len(),
            source_skin.len()
        ));
    }
    if joint_indices.is_empty() {
        return Err(format!(
            "non-skeletal skin fallback has no resolved joints mesh='{}'",
            mesh.name
        ));
    }

    let anchors = joint_indices
        .iter()
        .map(|joint| {
            let transform = skeleton_globals.get(*joint as usize).ok_or_else(|| {
                format!(
                    "non-skeletal skin fallback joint outside master palette mesh='{}' joint={joint}",
                    mesh.name
                )
            })?;
            Ok((*joint, transform.transform_point3(Vec3::ZERO)))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut rebound = Vec::with_capacity(mesh.vertices.len());
    for vertex in &mesh.vertices {
        let position = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
        let mut ranked = anchors
            .iter()
            .map(|(joint, anchor)| (*joint, (position - *anchor).length_squared()))
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
        ranked.truncate(4);

        let mut joints = [0_u16; 4];
        let mut weights = [0.0_f32; 4];
        if ranked[0].1 <= 1.0e-10 {
            joints[0] = ranked[0].0;
            weights[0] = 1.0;
        } else {
            // Five-centimetre regularization keeps the approximation smooth across jacket seams
            // while still making sleeves follow shoulder/elbow anchors rather than the torso.
            const DISTANCE_REGULARIZER_SQ: f32 = 0.0025;
            let mut total = 0.0_f32;
            for (slot, (joint, distance_sq)) in ranked.iter().enumerate() {
                let weight = 1.0 / (distance_sq + DISTANCE_REGULARIZER_SQ);
                joints[slot] = *joint;
                weights[slot] = weight;
                total += weight;
            }
            if !total.is_finite() || total <= 0.0 {
                return Err(format!(
                    "non-skeletal skin fallback produced invalid weights mesh='{}'",
                    mesh.name
                ));
            }
            for weight in &mut weights {
                *weight /= total;
            }
        }
        rebound.push(YddBinarySkinVertex {
            joints,
            weights,
            joints_extra: [0; 4],
            weights_extra: [0.0; 4],
        });
    }
    mesh.skin = Some(rebound);
    Ok(())
}

fn side_deform_joint_name(name: &str, suffix: &str) -> Option<String> {
    if name.starts_with("l_") {
        Some(format!("l_{suffix}"))
    } else if name.starts_with("r_") {
        Some(format!("r_{suffix}"))
    } else {
        None
    }
}

fn finger_deform_joint_name(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let suffix = [
        ("index_meta", "index_meta"),
        ("indexmeta", "index_meta"),
        ("middle_meta", "middle_meta"),
        ("middlemeta", "middle_meta"),
        ("ring_meta", "ring_meta"),
        ("ringmeta", "ring_meta"),
        ("pinky_meta", "pinky_meta"),
        ("pinkymeta", "pinky_meta"),
        ("indexc", "indexc"),
        ("indexb", "indexb"),
        ("indexa", "indexa"),
        ("middlec", "middlec"),
        ("middleb", "middleb"),
        ("middlea", "middlea"),
        ("ringc", "ringc"),
        ("ringb", "ringb"),
        ("ringa", "ringa"),
        ("pinkyc", "pinkyc"),
        ("pinkyb", "pinkyb"),
        ("pinkya", "pinkya"),
        ("thumbc", "thumbc"),
        ("thumbb", "thumbb"),
        ("thumba", "thumba"),
    ]
    .into_iter()
    .find_map(|(needle, canonical)| lower.contains(needle).then_some(canonical))?;
    side_deform_joint_name(&lower, suffix)
}

pub(super) fn corrective_skin_target_map(skeleton: &DecodedSkeleton) -> Result<Vec<u16>, String> {
    let by_name = skeleton
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.to_ascii_lowercase(), index))
        .collect::<BTreeMap<_, _>>();
    let mut memo = vec![None::<u16>; skeleton.joints.len()];
    let mut active = BTreeSet::<usize>::new();

    fn resolve(
        index: usize,
        skeleton: &DecodedSkeleton,
        by_name: &BTreeMap<String, usize>,
        memo: &mut [Option<u16>],
        active: &mut BTreeSet<usize>,
    ) -> Result<u16, String> {
        if let Some(target) = memo[index] {
            return Ok(target);
        }
        if !active.insert(index) {
            return Err(format!("corrective skin mapping cycle at joint={index}"));
        }
        let joint = &skeleton.joints[index];
        let name = joint.name.to_ascii_lowercase();
        let lookup = |candidate: &str| by_name.get(candidate).copied();
        let mut explicit = None::<usize>;

        if let Some(candidate) = name.strip_suffix("_helper") {
            explicit = lookup(candidate);
        }
        if explicit.is_none() {
            if let Some(candidate) = name.strip_suffix("_offset") {
                explicit = lookup(candidate);
            }
        }
        if explicit.is_none() {
            if let Some(candidate) = finger_deform_joint_name(&name) {
                explicit = lookup(&candidate);
            }
        }
        if explicit.is_none() && name.contains("wrist") {
            explicit = side_deform_joint_name(&name, "wrist").and_then(|value| lookup(&value));
        }
        if explicit.is_none() && name.contains("elbow") {
            explicit = side_deform_joint_name(&name, "elbow").and_then(|value| lookup(&value));
        }
        if explicit.is_none() && name.contains("shoulder") {
            explicit = side_deform_joint_name(&name, "shoulder").and_then(|value| lookup(&value));
        }
        if explicit.is_none() && name.contains("clavicle") {
            explicit = side_deform_joint_name(&name, "clavicle").and_then(|value| lookup(&value));
        }
        if explicit.is_none() && name.contains("twist_") {
            let number = name
                .split("twist_")
                .nth(1)
                .and_then(|tail| tail.get(0..2))
                .and_then(|raw| raw.parse::<u8>().ok());
            let canonical = match number {
                Some(0..=3) => "shoulder",
                Some(4..=6) => "elbow",
                Some(_) => "wrist",
                None => "elbow",
            };
            explicit = side_deform_joint_name(&name, canonical).and_then(|value| lookup(&value));
        }

        let target_index = if let Some(candidate) = explicit {
            if candidate == index {
                index
            } else {
                resolve(candidate, skeleton, by_name, memo, active)? as usize
            }
        } else if name.contains("_helper")
            || name.contains("finger_roll")
            || name.contains("_roll_")
            || name.contains("bicep")
            || name.contains("tricep")
        {
            if let Some(parent) = joint.parent_index {
                resolve(parent as usize, skeleton, by_name, memo, active)? as usize
            } else {
                index
            }
        } else {
            index
        };
        let target = u16::try_from(target_index)
            .map_err(|_| format!("corrective skin target exceeds u16 index={target_index}"))?;
        active.remove(&index);
        memo[index] = Some(target);
        Ok(target)
    }

    for index in 0..skeleton.joints.len() {
        let _ = resolve(index, skeleton, &by_name, &mut memo, &mut active)?;
    }
    Ok(memo
        .into_iter()
        .map(|value| value.expect("resolved"))
        .collect())
}

pub(super) fn collapse_mesh_corrective_skin(
    mesh: &mut crate::geometry::ImportMesh,
    target_map: &[u16],
) -> Result<usize, String> {
    let Some(skin) = mesh.skin.as_mut() else {
        return Ok(0);
    };
    let mut changed_vertices = 0usize;
    for vertex in skin.iter_mut() {
        let mut combined = BTreeMap::<u16, f32>::new();
        let mut changed = false;
        for (&joint, &weight) in vertex
            .joints
            .iter()
            .chain(vertex.joints_extra.iter())
            .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
        {
            if weight <= 0.0 {
                continue;
            }
            let target = *target_map.get(joint as usize).ok_or_else(|| {
                format!("corrective skin source joint outside mapping joint={joint}")
            })?;
            changed |= target != joint;
            *combined.entry(target).or_insert(0.0) += weight;
        }
        if !changed {
            continue;
        }
        let mut influences = combined.into_iter().collect::<Vec<_>>();
        influences.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        if influences.len() > 8 {
            influences.truncate(8);
        }
        let total = influences.iter().map(|(_, weight)| *weight).sum::<f32>();
        if !total.is_finite() || total <= 1.0e-8 {
            return Err(format!(
                "corrective skin collapse produced invalid total mesh='{}'",
                mesh.name
            ));
        }
        let mut joints = [0u16; 8];
        let mut weights = [0.0f32; 8];
        for (slot, (joint, weight)) in influences.into_iter().enumerate() {
            joints[slot] = joint;
            weights[slot] = weight / total;
        }
        *vertex = YddBinarySkinVertex {
            joints: [joints[0], joints[1], joints[2], joints[3]],
            weights: [weights[0], weights[1], weights[2], weights[3]],
            joints_extra: [joints[4], joints[5], joints[6], joints[7]],
            weights_extra: [weights[4], weights[5], weights[6], weights[7]],
        };
        changed_vertices += 1;
    }
    Ok(changed_vertices)
}

pub(super) fn remap_skin_vertex_to_master(
    mut vertex: YddBinarySkinVertex,
    source_domain_size: usize,
    master_domain_size: usize,
    local_to_master: Option<&[Option<u16>]>,
) -> Result<YddBinarySkinVertex, String> {
    if source_domain_size == master_domain_size {
        return Ok(vertex);
    }
    let mapping = local_to_master.ok_or_else(|| {
        format!(
            "unknown skin domain source_domain={} master_domain={}; explicit subset mapping required",
            source_domain_size, master_domain_size
        )
    })?;
    if mapping.len() != source_domain_size {
        return Err(format!(
            "subset mapping size mismatch source_domain={} mapping_entries={}",
            source_domain_size,
            mapping.len()
        ));
    }

    fn remap_quartet(
        joints: &mut [u16; 4],
        weights: &[f32; 4],
        mapping: &[Option<u16>],
        source_domain_size: usize,
        master_domain_size: usize,
    ) -> Result<(), String> {
        for slot in 0..4 {
            let weight = weights[slot];
            if weight <= 0.0 {
                continue;
            }
            let local = usize::from(joints[slot]);
            if local >= source_domain_size {
                return Err(format!(
                    "weighted local joint outside source skin domain local joint {} source_domain={}",
                    local, source_domain_size
                ));
            }
            let master = mapping[local].ok_or_else(|| {
                format!(
                    "subset mapping missing weighted local joint {} source_domain={} master_domain={}",
                    local, source_domain_size, master_domain_size
                )
            })?;
            if usize::from(master) >= master_domain_size {
                return Err(format!(
                    "subset mapping target outside master domain local joint {} target={} master_domain={}",
                    local, master, master_domain_size
                ));
            }
            joints[slot] = master;
        }
        Ok(())
    }

    remap_quartet(
        &mut vertex.joints,
        &vertex.weights,
        mapping,
        source_domain_size,
        master_domain_size,
    )?;
    remap_quartet(
        &mut vertex.joints_extra,
        &vertex.weights_extra,
        mapping,
        source_domain_size,
        master_domain_size,
    )?;
    Ok(vertex)
}
