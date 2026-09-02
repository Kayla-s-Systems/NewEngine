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
#[path = "compile/skin_processing.rs"]
mod skin_processing;
mod static_pak;
mod types;
mod validation;

use artifact_io::{encode_skeleton_xml, read_file, write_atomic};
pub use rigid::compile_rigid_joint_variants;
use rigid::imported_joint_globals;
use skin_processing::{
    collapse_mesh_corrective_skin, corrective_skin_target_map, rebind_mesh_skin_to_master_joints,
    remap_skin_vertex_to_master, resolve_master_fallback_joints, transform_mesh_to_model_space,
    validate_rigid_source_to_model,
};
pub use static_pak::compile_static_pak;
pub use types::*;
pub(crate) use validation::encode_nef8;
use validation::{
    validate_geometry_sanity, validate_native_eye_contract, validate_skin_contract,
    validate_skin_joint_range,
};

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

#[cfg(test)]
mod tests;
