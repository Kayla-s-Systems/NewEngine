use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::ydd_binary::{
    encode_ydd_binary_body, YddBinaryDocument, YddBinaryEntry, YddBinaryMesh, YddBinarySkinVertex,
    YDD_BINARY_SCHEMA_VERSION,
};
use newengine_assets_api::{
    encode_list_file, ListFileEncodeRequest, LIST_FILE_CONTENT_KIND_YDD, LIST_FILE_CONTENT_KIND_YMT,
};
use newengine_math::{Mat4, Quat, Vec3};

use crate::geometry::{decode_geometry_lod0, SkinLossStats};
use crate::pak::PakFile;
use crate::skeleton::{decode_skeleton_with_profile, DecodedSkeleton, SkeletonProfile};

mod artifact_io;
mod rigid;
mod static_pak;
mod types;
mod validation;

use artifact_io::{encode_skeleton_xml, read_file, write_atomic};
pub use rigid::compile_rigid_joint_variants;
use rigid::imported_joint_globals;
pub use static_pak::compile_static_pak;
pub use types::*;
pub(crate) use validation::encode_nef8;
use validation::{
    validate_geometry_sanity, validate_native_eye_contract, validate_skin_contract,
    validate_skin_joint_range,
};

fn validate_rigid_source_to_model(matrix: [f32; 16]) -> Result<Mat4, String> {
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

fn transform_mesh_to_model_space(
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

fn resolve_master_fallback_joints(
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

fn rebind_mesh_skin_to_master_joints(
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

fn corrective_skin_target_map(skeleton: &DecodedSkeleton) -> Result<Vec<u16>, String> {
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

fn collapse_mesh_corrective_skin(
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

pub fn compile_character(
    request: &CharacterCompileRequest,
) -> Result<CharacterCompileReport, String> {
    if request.name.trim().is_empty() {
        return Err("character import name must not be empty".to_owned());
    }
    if request.package_paths.is_empty() {
        return Err("character import requires at least one geometry package".to_owned());
    }
    let skeleton_pak = PakFile::parse(read_file(&request.skeleton_path)?)?;
    let skeleton = decode_skeleton_with_profile(&skeleton_pak, request.skeleton_profile)?;
    let skeleton_globals = imported_joint_globals(&skeleton)?;
    let corrective_skin_targets = (!request.corrective_skin_collapse_prefixes.is_empty())
        .then(|| corrective_skin_target_map(&skeleton))
        .transpose()?;

    if request.master_rig && !request.package_skin_fallback_joints.is_empty() {
        return Err(
            "master-rig import forbids package_skin_fallback_joints; use exact package skin subsets"
                .to_owned(),
        );
    }
    for subset in &request.package_skin_subsets {
        if !request.package_paths.contains(&subset.package_path) {
            return Err(format!(
                "master-rig subset references a package outside this build package='{}'",
                subset.package_path.display()
            ));
        }
        if subset.source_domain_size == 0
            || subset.local_to_master.len() != subset.source_domain_size
        {
            return Err(format!(
                "master-rig subset mapping size mismatch package='{}' source_domain={} mapping_entries={}",
                subset.package_path.display(),
                subset.source_domain_size,
                subset.local_to_master.len()
            ));
        }
        for (local, target) in subset.local_to_master.iter().enumerate() {
            if let Some(target) = target {
                if usize::from(*target) >= skeleton.joints.len() {
                    return Err(format!(
                        "master-rig subset target outside master domain package='{}' local={} target={} master_joints={}",
                        subset.package_path.display(),
                        local,
                        target,
                        skeleton.joints.len()
                    ));
                }
            }
        }
    }
    for (index, left) in request.package_skin_subsets.iter().enumerate() {
        for right in request.package_skin_subsets.iter().skip(index + 1) {
            if left.package_path == right.package_path
                && left.source_domain_size == right.source_domain_size
            {
                return Err(format!(
                    "duplicate master-rig subset rule package='{}' source_domain={}",
                    left.package_path.display(),
                    left.source_domain_size
                ));
            }
        }
    }

    for (package, joints) in &request.package_skin_fallback_joints {
        if !request.package_paths.contains(package) {
            return Err(format!(
                "non-skeletal skin fallback references a package outside this character build package='{}'",
                package.display()
            ));
        }
        if joints.is_empty() {
            return Err(format!(
                "non-skeletal skin fallback has no joints package='{}'",
                package.display()
            ));
        }
    }

    let mut meshes = Vec::new();
    let mut skin_loss = SkinLossStats::default();
    let mut skin_fallbacks = Vec::new();
    for path in &request.package_paths {
        let pak = PakFile::parse(read_file(path)?)?;
        let decoded =
            decode_geometry_lod0(&pak).map_err(|error| format!("{}: {error}", path.display()))?;
        skin_loss.merge(decoded.skin_loss);
        let package_filters = request
            .package_mesh_prefixes
            .iter()
            .filter(|(filter_path, _)| filter_path == path)
            .map(|(_, prefix)| prefix.as_str())
            .collect::<Vec<_>>();
        let fallback_rules = request
            .package_skin_fallback_joints
            .iter()
            .filter(|(package, _)| package == path)
            .collect::<Vec<_>>();
        if fallback_rules.len() > 1 {
            return Err(format!(
                "multiple non-skeletal skin fallback rules target package='{}'",
                path.display()
            ));
        }
        let fallback = fallback_rules.first().map(|(_, joints)| joints.as_slice());
        let resolved_fallback = fallback
            .map(|joints| resolve_master_fallback_joints(&skeleton, joints, path))
            .transpose()?;
        let subset_rules = request
            .package_skin_subsets
            .iter()
            .filter(|rule| &rule.package_path == path)
            .collect::<Vec<_>>();

        for mut mesh in decoded.meshes {
            if !package_filters.is_empty()
                && !package_filters
                    .iter()
                    .any(|prefix| mesh.name.starts_with(prefix))
            {
                continue;
            }
            if mesh.skin.is_some() && request.master_rig {
                let source_domain = mesh.source_skin_joint_domain_size.ok_or_else(|| {
                    format!(
                        "master-rig skinned mesh has no source skin domain package='{}' mesh='{}'",
                        path.display(),
                        mesh.name
                    )
                })?;
                if source_domain != skeleton.joints.len() {
                    let matching = subset_rules
                        .iter()
                        .filter(|rule| rule.source_domain_size == source_domain)
                        .copied()
                        .collect::<Vec<_>>();
                    if matching.len() != 1 {
                        return Err(format!(
                            "master-rig skin domain requires exactly one explicit subset package='{}' mesh='{}' source_domain={} master_joints={} matching_rules={}",
                            path.display(),
                            mesh.name,
                            source_domain,
                            skeleton.joints.len(),
                            matching.len()
                        ));
                    }
                    let rule = matching[0];
                    let skin = mesh.skin.as_mut().ok_or_else(|| {
                        format!("master-rig skin disappeared mesh='{}'", mesh.name)
                    })?;
                    for vertex in skin.iter_mut() {
                        *vertex = remap_skin_vertex_to_master(
                            *vertex,
                            source_domain,
                            skeleton.joints.len(),
                            Some(&rule.local_to_master),
                        )?;
                    }
                }
            } else if let Some(source_domain) = mesh.source_skin_joint_domain_size {
                if mesh.skin.is_some() && source_domain != skeleton.joints.len() {
                    let target_joints = fallback.ok_or_else(|| {
                        format!(
                            "non-skeletal skin domain cannot be emitted as master-skeleton skin package='{}' mesh='{}' source_domain={} master_joints={}; configure an explicit package skin fallback or a dedicated cloth runtime",
                            path.display(),
                            mesh.name,
                            source_domain,
                            skeleton.joints.len()
                        )
                    })?;
                    let resolved = resolved_fallback.as_deref().ok_or_else(|| {
                        format!(
                            "non-skeletal skin fallback failed to resolve package='{}' mesh='{}'",
                            path.display(),
                            mesh.name
                        )
                    })?;
                    rebind_mesh_skin_to_master_joints(&mut mesh, &skeleton_globals, resolved)?;
                    skin_fallbacks.push(SkinFallbackReport {
                        package: path.clone(),
                        mesh: mesh.name.clone(),
                        source_joint_domain_size: source_domain,
                        target_joints: target_joints.to_vec(),
                    });
                }
            }
            if request
                .corrective_skin_collapse_prefixes
                .iter()
                .any(|prefix| mesh.name.starts_with(prefix))
            {
                let targets = corrective_skin_targets
                    .as_deref()
                    .ok_or("corrective skin target map unavailable")?;
                let changed = collapse_mesh_corrective_skin(&mut mesh, targets)?;
                println!(
                    "corrective-skin-collapse package='{}' mesh='{}' changed_vertices={}",
                    path.display(),
                    mesh.name,
                    changed
                );
            }
            validate_skin_joint_range(&mesh, skeleton.joints.len(), path)?;
            meshes.push(mesh);
        }
    }
    if meshes.is_empty() {
        return Err("character import produced no LOD0 meshes".to_owned());
    }
    for prefix in &request.required_mesh_prefixes {
        let prefix = prefix.trim();
        if prefix.is_empty() {
            return Err("required mesh prefix must not be empty".to_owned());
        }
        if !meshes.iter().any(|mesh| mesh.name.starts_with(prefix)) {
            return Err(format!(
                "character completeness contract missing required LOD0 mesh prefix='{prefix}'"
            ));
        }
    }
    validate_geometry_sanity(&meshes)?;
    // Native diagnostics compare decoded source geometry to the authored native skeleton.
    // Run them before optional model-space canonicalization; otherwise a valid rigid
    // source_to_model transform makes geometry and bind centers live in different spaces.
    validate_native_eye_contract(&meshes, &skeleton)?;
    let source_to_model = request
        .source_to_model
        .map(validate_rigid_source_to_model)
        .transpose()?;
    if let Some(transform) = source_to_model {
        for mesh in &mut meshes {
            transform_mesh_to_model_space(mesh, transform)?;
        }
    }

    let bounds_min = [
        meshes
            .iter()
            .map(|mesh| mesh.bounds_min[0])
            .fold(f32::INFINITY, f32::min),
        meshes
            .iter()
            .map(|mesh| mesh.bounds_min[1])
            .fold(f32::INFINITY, f32::min),
        meshes
            .iter()
            .map(|mesh| mesh.bounds_min[2])
            .fold(f32::INFINITY, f32::min),
    ];
    let bounds_max = [
        meshes
            .iter()
            .map(|mesh| mesh.bounds_max[0])
            .fold(f32::NEG_INFINITY, f32::max),
        meshes
            .iter()
            .map(|mesh| mesh.bounds_max[1])
            .fold(f32::NEG_INFINITY, f32::max),
        meshes
            .iter()
            .map(|mesh| mesh.bounds_max[2])
            .fold(f32::NEG_INFINITY, f32::max),
    ];
    let source_material_slots = if request.material_by_source_identity {
        let identities = meshes
            .iter()
            .map(|mesh| {
                mesh.source_material
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        format!(
                            "source-material identity mode requires material metadata mesh='{}'",
                            mesh.name
                        )
                    })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if request.material_identity_slots.is_empty() {
            identities
                .into_iter()
                .enumerate()
                .map(|(index, identity)| (identity.to_owned(), index))
                .collect::<BTreeMap<_, _>>()
        } else {
            let mut canonical = BTreeMap::<String, usize>::new();
            for (identity, slot) in &request.material_identity_slots {
                let identity = identity.trim();
                if identity.is_empty() {
                    return Err(
                        "material identity slot contract contains an empty identity".to_owned()
                    );
                }
                if canonical.insert(identity.to_owned(), *slot).is_some() {
                    return Err(format!(
                        "material identity slot contract contains duplicate identity='{identity}'"
                    ));
                }
            }
            let mut resolved = BTreeMap::new();
            for identity in identities {
                let slot = canonical.get(identity).copied().ok_or_else(|| {
                    format!(
                        "material identity slot contract has no binding for source material='{identity}'"
                    )
                })?;
                resolved.insert(identity.to_owned(), slot);
            }
            resolved
        }
    } else {
        if !request.material_identity_slots.is_empty() {
            return Err(
                "material identity slot contract requires --material-by-source-identity".to_owned(),
            );
        }
        BTreeMap::new()
    };
    let native_meshes = meshes
        .iter()
        .enumerate()
        .map(|(index, mesh)| YddBinaryMesh {
            name: mesh.name.clone(),
            material_ref: request
                .material_overrides
                .iter()
                .filter(|(prefix, _)| mesh.name.starts_with(prefix))
                .max_by_key(|(prefix, _)| prefix.len())
                .map(|(_, reference)| reference.clone())
                .or_else(|| {
                    request.material_library_ref.as_ref().map(|library| {
                        let material_index = if request.material_by_source_identity {
                            let identity = mesh
                                .source_material
                                .as_deref()
                                .expect("validated source-material identity");
                            *source_material_slots
                                .get(identity)
                                .expect("source material slot must exist")
                        } else {
                            index
                        };
                        format!("{}@m{:02}", library.trim_end_matches('@'), material_index)
                    })
                }),
            bounds_min: mesh.bounds_min,
            bounds_max: mesh.bounds_max,
            vertices: mesh.vertices.clone(),
            skin: mesh.skin.clone(),
            indices: mesh.indices.clone(),
        })
        .collect::<Vec<_>>();
    let source_path = request
        .package_paths
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("source.pak")
        })
        .collect::<Vec<_>>()
        .join("+");
    validate_skin_contract(&native_meshes, skeleton.joints.len())?;

    let document = YddBinaryDocument {
        entries: vec![YddBinaryEntry {
            name: request.name.clone(),
            source_path: format!("northstar.pc://{source_path}"),
            properties_ref: None,
            bounds_min,
            bounds_max,
            // Geometry is optionally canonicalized while JOINT_HIERARCHY remains in native
            // source space. Skinning conjugates the native palette by this exact matrix.
            skin_source_to_model: Some(request.source_to_model.unwrap_or([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ])),
            meshes: native_meshes,
        }],
    };
    let ydd_body = encode_ydd_binary_body(&document)?;
    let ydd_file = encode_nef8(
        &ydd_body,
        LIST_FILE_CONTENT_KIND_YDD,
        YDD_BINARY_SCHEMA_VERSION as u16,
        1,
    )?;
    let ymt_body = encode_skeleton_xml(&skeleton);
    let ymt_file = encode_nef8(&ymt_body, LIST_FILE_CONTENT_KIND_YMT, 1, 1)?;

    fs::create_dir_all(&request.output_dir).map_err(|error| {
        format!(
            "failed to create output directory '{}': {error}",
            request.output_dir.display()
        )
    })?;
    let ydd_path = request.output_dir.join(format!("{}.ydd", request.name));
    let ymt_path = request.output_dir.join(format!("{}.ymt", request.name));
    write_atomic(&ydd_path, &ydd_file)?;
    write_atomic(&ymt_path, &ymt_file)?;

    let vertex_count = meshes.iter().map(|mesh| mesh.vertices.len()).sum();
    let index_count = meshes.iter().map(|mesh| mesh.indices.len()).sum();
    Ok(CharacterCompileReport {
        ydd_path,
        ymt_path,
        mesh_count: meshes.len(),
        vertex_count,
        index_count,
        joint_count: skeleton.joints.len(),
        skin_loss,
        material_slots: source_material_slots
            .iter()
            .map(|(identity, index)| (format!("m{index:02}"), identity.clone()))
            .collect(),
        skin_fallbacks,
    })
}

fn remap_skin_vertex_to_master(
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

#[cfg(test)]
mod tests;
