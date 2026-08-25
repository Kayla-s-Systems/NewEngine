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

#[derive(Clone, Debug)]
pub struct CharacterCompileRequest {
    pub name: String,
    pub package_paths: Vec<PathBuf>,
    pub skeleton_path: PathBuf,
    pub skeleton_profile: SkeletonProfile,
    pub output_dir: PathBuf,
    /// Optional canonical NEMAT reference. When set, imported LOD0 meshes are
    /// bound deterministically as @m00, @m01, ... in package/mesh order.
    pub material_library_ref: Option<String>,
    /// Resolve default material slots from sorted native source-material identity.
    pub material_by_source_identity: bool,
    /// Optional per-package mesh prefixes. If a package has one or more entries here,
    /// only meshes whose decoded name starts with one of those prefixes are imported.
    pub package_mesh_prefixes: Vec<(PathBuf, String)>,
    /// Optional mesh-prefix to canonical material-ref overrides. Longest prefix wins.
    pub material_overrides: Vec<(String, String)>,
    /// Build-time completeness contract: every prefix must match at least one imported LOD0 mesh.
    pub required_mesh_prefixes: Vec<String>,
    /// Explicit fallback for packages whose skin domain is not the master skeleton. The listed
    /// master joints are used to produce a stable proximity-weighted skeletal approximation until
    /// the source cloth simulation-node domain has a dedicated runtime.
    pub package_skin_fallback_joints: Vec<(PathBuf, Vec<String>)>,
    /// Optional rigid affine transform from decoded PAK source space into canonical model space.
    /// The same matrix is persisted as YDD `skin_source_to_model`, preserving native skinning.
    pub source_to_model: Option<[f32; 16]>,
}

#[derive(Clone, Debug)]
pub struct CharacterCompileReport {
    pub ydd_path: PathBuf,
    pub ymt_path: PathBuf,
    pub mesh_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub joint_count: usize,
    pub skin_loss: SkinLossStats,
    pub material_slots: Vec<(String, String)>,
    pub skin_fallbacks: Vec<SkinFallbackReport>,
}

#[derive(Clone, Debug)]
pub struct SkinFallbackReport {
    pub package: PathBuf,
    pub mesh: String,
    pub source_joint_domain_size: usize,
    pub target_joints: Vec<String>,
}

/// Offline extraction of rigid pieces authored as joints inside one skinned TLOU2 PC geometry.
/// This is used for weapon debris such as the five `rifle-shell-group` casing variants: source
/// skinning is consumed by the importer and runtime receives ordinary rigid YDD entries.
#[derive(Clone, Debug)]
pub struct RigidJointVariantsCompileRequest {
    pub name: String,
    pub package_path: PathBuf,
    pub joints: Vec<String>,
    pub output_path: PathBuf,
    pub material_ref: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RigidJointVariantsCompileReport {
    pub ydd_path: PathBuf,
    pub entry_count: usize,
    pub mesh_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
}

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

        for mut mesh in decoded.meshes {
            if !package_filters.is_empty()
                && !package_filters
                    .iter()
                    .any(|prefix| mesh.name.starts_with(prefix))
            {
                continue;
            }
            if let Some(source_domain) = mesh.source_skin_joint_domain_size {
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
    let source_to_model = request
        .source_to_model
        .map(validate_rigid_source_to_model)
        .transpose()?;
    if let Some(transform) = source_to_model {
        for mesh in &mut meshes {
            transform_mesh_to_model_space(mesh, transform)?;
        }
    }
    validate_native_eye_contract(&meshes, &skeleton)?;

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
        identities
            .into_iter()
            .enumerate()
            .map(|(index, identity)| (identity.to_owned(), index))
            .collect::<BTreeMap<_, _>>()
    } else {
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
            source_path: format!("northstar.tlou2.pc://{source_path}"),
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

fn validate_geometry_sanity(meshes: &[crate::geometry::ImportMesh]) -> Result<(), String> {
    const MAX_CHARACTER_EXTENT: f32 = 100.0;
    for mesh in meshes {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            return Err(format!(
                "invalid runtime geometry mesh='{}' vertices={} indices={}",
                mesh.name,
                mesh.vertices.len(),
                mesh.indices.len()
            ));
        }
        if mesh
            .bounds_min
            .iter()
            .chain(mesh.bounds_max.iter())
            .any(|value| !value.is_finite())
        {
            return Err(format!("non-finite runtime bounds mesh='{}'", mesh.name));
        }
        let extent = [
            mesh.bounds_max[0] - mesh.bounds_min[0],
            mesh.bounds_max[1] - mesh.bounds_min[1],
            mesh.bounds_max[2] - mesh.bounds_min[2],
        ];
        if extent
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > MAX_CHARACTER_EXTENT)
        {
            return Err(format!(
                "implausible runtime bounds mesh='{}' min={:?} max={:?}",
                mesh.name, mesh.bounds_min, mesh.bounds_max
            ));
        }
        for (vertex_index, vertex) in mesh.vertices.iter().enumerate() {
            if vertex
                .position
                .iter()
                .chain(vertex.normal.iter())
                .chain(vertex.uv0.iter())
                .any(|value| !value.is_finite())
            {
                return Err(format!(
                    "non-finite runtime vertex mesh='{}' vertex={vertex_index}",
                    mesh.name
                ));
            }
        }
    }
    Ok(())
}

fn imported_joint_local_matrix(joint: &crate::skeleton::ImportedJoint) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::new(joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2]),
        Quat::from_xyzw(
            joint.rotation_ls[0],
            joint.rotation_ls[1],
            joint.rotation_ls[2],
            joint.rotation_ls[3],
        )
        .normalize_or_identity(),
        Vec3::new(
            joint.position_ls[0],
            joint.position_ls[1],
            joint.position_ls[2],
        ),
    )
}

fn imported_joint_globals(skeleton: &DecodedSkeleton) -> Result<Vec<Mat4>, String> {
    let mut globals = vec![Mat4::IDENTITY; skeleton.joints.len()];
    let mut done = vec![false; skeleton.joints.len()];
    let mut remaining = skeleton.joints.len();
    while remaining > 0 {
        let mut progress = false;
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if done[index] {
                continue;
            }
            if joint
                .parent_index
                .is_some_and(|parent| !done[parent as usize])
            {
                continue;
            }
            let local = imported_joint_local_matrix(joint);
            globals[index] = joint
                .parent_index
                .map(|parent| globals[parent as usize] * local)
                .unwrap_or(local);
            done[index] = true;
            remaining -= 1;
            progress = true;
        }
        if !progress {
            return Err(
                "rigid-joint extraction found an unresolvable skeleton hierarchy".to_owned(),
            );
        }
    }
    Ok(globals)
}

#[inline]
fn dominant_skin_joint(
    vertex: &newengine_asset_format_nef8::ydd_binary::YddBinarySkinVertex,
) -> u16 {
    vertex
        .joints
        .iter()
        .chain(vertex.joints_extra.iter())
        .copied()
        .zip(
            vertex
                .weights
                .iter()
                .chain(vertex.weights_extra.iter())
                .copied(),
        )
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(joint, _)| joint)
        .unwrap_or(0)
}

pub fn compile_rigid_joint_variants(
    request: &RigidJointVariantsCompileRequest,
) -> Result<RigidJointVariantsCompileReport, String> {
    if request.name.trim().is_empty() {
        return Err("rigid-joint asset name must not be empty".to_owned());
    }
    if request.joints.is_empty() {
        return Err("rigid-joint extraction requires at least one joint".to_owned());
    }
    let pak = PakFile::parse(read_file(&request.package_path)?)?;
    let geometry = decode_geometry_lod0(&pak)?;
    let skeleton = decode_skeleton_with_profile(&pak, SkeletonProfile::Generic)?;
    let globals = imported_joint_globals(&skeleton)?;
    let mut entries = Vec::with_capacity(request.joints.len());
    let mut total_meshes = 0usize;
    let mut total_vertices = 0usize;
    let mut total_indices = 0usize;

    for requested_name in &request.joints {
        let requested_name = requested_name.trim();
        if requested_name.is_empty() {
            return Err("rigid-joint extraction contains an empty joint name".to_owned());
        }
        let joint_index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == requested_name)
            .ok_or_else(|| format!("rigid-joint source has no joint '{requested_name}'"))?;
        let joint_to_local = globals[joint_index].inverse();
        if joint_to_local
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "rigid-joint '{requested_name}' bind transform is not invertible"
            ));
        }

        let mut entry_meshes = Vec::new();
        let mut entry_min = Vec3::splat(f32::INFINITY);
        let mut entry_max = Vec3::splat(f32::NEG_INFINITY);
        for source_mesh in &geometry.meshes {
            let skin = source_mesh.skin.as_ref().ok_or_else(|| {
                format!(
                    "rigid-joint source mesh '{}' has no skin stream",
                    source_mesh.name
                )
            })?;
            if skin.len() != source_mesh.vertices.len() {
                return Err(format!(
                    "rigid-joint skin/vertex mismatch mesh='{}' skin={} vertices={}",
                    source_mesh.name,
                    skin.len(),
                    source_mesh.vertices.len()
                ));
            }
            let dominant = skin.iter().map(dominant_skin_joint).collect::<Vec<_>>();
            let mut remap = std::collections::BTreeMap::<u32, u32>::new();
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            for triangle in source_mesh.indices.chunks_exact(3) {
                if !triangle.iter().all(|index| {
                    dominant
                        .get(*index as usize)
                        .is_some_and(|joint| *joint as usize == joint_index)
                }) {
                    continue;
                }
                for source_index in triangle {
                    let target_index = if let Some(existing) = remap.get(source_index) {
                        *existing
                    } else {
                        let source = source_mesh
                            .vertices
                            .get(*source_index as usize)
                            .ok_or("rigid-joint source index outside vertex stream")?;
                        let source_position =
                            Vec3::new(source.position[0], source.position[1], source.position[2]);
                        let source_normal =
                            Vec3::new(source.normal[0], source.normal[1], source.normal[2]);
                        let position = joint_to_local.transform_point3(source_position);
                        let normal = joint_to_local
                            .transform_vector3(source_normal)
                            .normalize_or_zero();
                        if !position.is_finite()
                            || !normal.is_finite()
                            || normal.length_squared() <= 1.0e-10
                        {
                            return Err(format!(
                                "rigid-joint '{requested_name}' produced invalid vertex"
                            ));
                        }
                        entry_min = entry_min.min(position);
                        entry_max = entry_max.max(position);
                        let target = vertices.len() as u32;
                        vertices.push(newengine_asset_format_nef8::ydd_binary::YddBinaryVertex {
                            position: [position.x, position.y, position.z],
                            normal: [normal.x, normal.y, normal.z],
                            uv0: source.uv0,
                        });
                        remap.insert(*source_index, target);
                        target
                    };
                    indices.push(target_index);
                }
            }
            if vertices.is_empty() {
                continue;
            }
            let mesh_min = vertices
                .iter()
                .fold(Vec3::splat(f32::INFINITY), |min, vertex| {
                    min.min(Vec3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                });
            let mesh_max = vertices
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |max, vertex| {
                    max.max(Vec3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                });
            total_vertices += vertices.len();
            total_indices += indices.len();
            total_meshes += 1;
            entry_meshes.push(YddBinaryMesh {
                name: requested_name.to_owned(),
                material_ref: request.material_ref.clone(),
                bounds_min: [mesh_min.x, mesh_min.y, mesh_min.z],
                bounds_max: [mesh_max.x, mesh_max.y, mesh_max.z],
                vertices,
                skin: None,
                indices,
            });
        }
        if entry_meshes.is_empty() {
            return Err(format!(
                "rigid-joint '{requested_name}' selected no complete source triangles"
            ));
        }
        entries.push(YddBinaryEntry {
            name: requested_name.to_owned(),
            source_path: format!(
                "northstar.tlou2.pc://{}#joint={requested_name}",
                request
                    .package_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("source.pak")
            ),
            properties_ref: None,
            bounds_min: [entry_min.x, entry_min.y, entry_min.z],
            bounds_max: [entry_max.x, entry_max.y, entry_max.z],
            skin_source_to_model: None,
            meshes: entry_meshes,
        });
    }

    let document = YddBinaryDocument { entries };
    let body = encode_ydd_binary_body(&document)?;
    let file = encode_nef8(
        &body,
        LIST_FILE_CONTENT_KIND_YDD,
        YDD_BINARY_SCHEMA_VERSION as u16,
        document.entries.len() as u32,
    )?;
    if let Some(parent) = request.output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create output directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    write_atomic(&request.output_path, &file)?;
    Ok(RigidJointVariantsCompileReport {
        ydd_path: request.output_path.clone(),
        entry_count: document.entries.len(),
        mesh_count: total_meshes,
        vertex_count: total_vertices,
        index_count: total_indices,
    })
}

fn validate_native_eye_contract(
    meshes: &[crate::geometry::ImportMesh],
    skeleton: &DecodedSkeleton,
) -> Result<(), String> {
    let Some(eye_mesh) = meshes.iter().find(|mesh| {
        let name = mesh.name.to_ascii_lowercase();
        let material = mesh
            .source_material
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        name.contains("abby_eyes_") || material.contains("/abby/abby-eyes:")
    }) else {
        return Ok(());
    };

    let joint_index = |name: &str| {
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .ok_or_else(|| format!("native Abby eye mesh requires skeleton joint '{name}'"))
    };
    let left = joint_index("l_eyeball")?;
    let right = joint_index("r_eyeball")?;
    if skeleton.joints[left].parent_index != skeleton.joints[right].parent_index {
        return Err("native Abby eyeballs do not share the same authored parent".to_owned());
    }
    let parent = skeleton.joints[left]
        .parent_index
        .ok_or_else(|| "native Abby eyeballs have no authored parent".to_owned())?
        as usize;
    let parent_name = skeleton
        .joints
        .get(parent)
        .map(|joint| joint.name.as_str())
        .ok_or_else(|| format!("native Abby eyeball parent outside skeleton index={parent}"))?;
    if parent_name != "headb" {
        return Err(format!(
            "native Abby eyeballs must remain direct children of headb parent={} name='{}'",
            parent, parent_name
        ));
    }

    let mut globals = vec![Mat4::IDENTITY; skeleton.joints.len()];
    for (index, joint) in skeleton.joints.iter().enumerate() {
        let local = Mat4::from_scale_rotation_translation(
            Vec3::new(joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2]),
            Quat::from_xyzw(
                joint.rotation_ls[0],
                joint.rotation_ls[1],
                joint.rotation_ls[2],
                joint.rotation_ls[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                joint.position_ls[0],
                joint.position_ls[1],
                joint.position_ls[2],
            ),
        );
        globals[index] = joint
            .parent_index
            .map(|parent| globals[parent as usize] * local)
            .unwrap_or(local);
    }

    let (left_scale, left_rotation, left_center) = globals[left].to_scale_rotation_translation();
    let (right_scale, right_rotation, right_center) =
        globals[right].to_scale_rotation_translation();
    if !left_scale.is_finite()
        || !right_scale.is_finite()
        || !left_rotation.is_finite()
        || !right_rotation.is_finite()
        || !left_center.is_finite()
        || !right_center.is_finite()
    {
        return Err("native Abby eye bind basis contains non-finite values".to_owned());
    }
    let scale_delta_vec = left_scale - right_scale;
    let scale_delta = scale_delta_vec
        .x
        .abs()
        .max(scale_delta_vec.y.abs())
        .max(scale_delta_vec.z.abs());
    let basis_dot = left_rotation
        .normalize_or_identity()
        .dot(right_rotation.normalize_or_identity())
        .abs();
    if scale_delta > 1.0e-4 || basis_dot < 0.9999 {
        return Err(format!(
            "native Abby eye bind bases diverge scale_delta={scale_delta:.8} rotation_dot={basis_dot:.8}"
        ));
    }
    let canonical_basis_dot = left_rotation
        .normalize_or_identity()
        .dot(Quat::IDENTITY)
        .abs()
        .min(
            right_rotation
                .normalize_or_identity()
                .dot(Quat::IDENTITY)
                .abs(),
        );
    if canonical_basis_dot < 0.9999 {
        return Err(format!(
            "native Abby eye global basis no longer matches authored canonical XYZ basis rotation_dot={canonical_basis_dot:.8}"
        ));
    }

    let Some(skin) = eye_mesh.skin.as_deref() else {
        return Err("native Abby eye mesh has no skin stream".to_owned());
    };
    if skin.len() != eye_mesh.vertices.len() {
        return Err(format!(
            "native Abby eye skin/vertex count mismatch skin={} vertices={}",
            skin.len(),
            eye_mesh.vertices.len()
        ));
    }

    let mut uv_min = [f32::INFINITY; 2];
    let mut uv_max = [f32::NEG_INFINITY; 2];
    let mut max_non_eye_weight = 0.0_f32;
    let mut left_vertices = 0usize;
    let mut right_vertices = 0usize;
    let mut max_center_distance = [0.0_f32; 2];
    for (vertex, skin) in eye_mesh.vertices.iter().zip(skin.iter()) {
        for component in 0..2 {
            if !vertex.uv0[component].is_finite() {
                return Err("native Abby eye UV0 contains non-finite values".to_owned());
            }
            uv_min[component] = uv_min[component].min(vertex.uv0[component]);
            uv_max[component] = uv_max[component].max(vertex.uv0[component]);
        }

        let mut left_weight = 0.0_f32;
        let mut right_weight = 0.0_f32;
        for (&joint, &weight) in skin
            .joints
            .iter()
            .chain(skin.joints_extra.iter())
            .zip(skin.weights.iter().chain(skin.weights_extra.iter()))
        {
            if usize::from(joint) == left {
                left_weight += weight;
            } else if usize::from(joint) == right {
                right_weight += weight;
            }
        }
        let non_eye_weight = (1.0 - left_weight - right_weight).max(0.0);
        max_non_eye_weight = max_non_eye_weight.max(non_eye_weight);
        let position = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
        if left_weight >= right_weight {
            left_vertices += 1;
            max_center_distance[0] = max_center_distance[0].max(position.distance(left_center));
        } else {
            right_vertices += 1;
            max_center_distance[1] = max_center_distance[1].max(position.distance(right_center));
        }
    }

    let uv_span = [uv_max[0] - uv_min[0], uv_max[1] - uv_min[1]];
    if uv_span[0] < 0.75 || uv_span[1] < 0.75 {
        return Err(format!(
            "native Abby eye UV0 collapsed/squashed u=[{:.6},{:.6}] v=[{:.6},{:.6}] span=[{:.6},{:.6}]",
            uv_min[0], uv_max[0], uv_min[1], uv_max[1], uv_span[0], uv_span[1]
        ));
    }
    let uv_aspect = uv_span[0] / uv_span[1].max(1.0e-8);
    if !(0.90..=1.10).contains(&uv_aspect) {
        return Err(format!(
            "native Abby eye UV0 anisotropy exceeds diagnostic contract aspect={uv_aspect:.6} span=[{:.6},{:.6}]",
            uv_span[0], uv_span[1]
        ));
    }
    if max_non_eye_weight > 1.0e-3 {
        return Err(format!(
            "native Abby eye mesh leaks skin weight outside l/r eyeball joints max_non_eye_weight={max_non_eye_weight:.8}"
        ));
    }
    if left_vertices == 0 || right_vertices == 0 {
        return Err(format!(
            "native Abby eye mesh did not resolve both eyeballs left_vertices={left_vertices} right_vertices={right_vertices}"
        ));
    }
    if max_center_distance[0] > 0.03 || max_center_distance[1] > 0.03 {
        return Err(format!(
            "native Abby eye geometry is displaced from authored bind centers left_max={:.6} right_max={:.6}",
            max_center_distance[0], max_center_distance[1]
        ));
    }

    Ok(())
}

fn validate_skin_joint_range(
    mesh: &crate::geometry::ImportMesh,
    joint_count: usize,
    source: &Path,
) -> Result<(), String> {
    let Some(skin) = mesh.skin.as_deref() else {
        return Ok(());
    };
    for (vertex_index, vertex) in skin.iter().enumerate() {
        for (&joint, &weight) in vertex
            .joints
            .iter()
            .chain(vertex.joints_extra.iter())
            .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
        {
            if weight > 0.0 && joint as usize >= joint_count {
                return Err(format!(
                    "skin joint outside skeleton source='{}' mesh='{}' vertex={} joint={} joints={}",
                    source.display(),
                    mesh.name,
                    vertex_index,
                    joint,
                    joint_count
                ));
            }
        }
    }
    Ok(())
}

fn validate_skin_contract(
    meshes: &[YddBinaryMesh],
    skeleton_joint_count: usize,
) -> Result<(), String> {
    const WEIGHT_EPSILON: f32 = 1.0e-4;
    for mesh in meshes {
        let Some(skin) = mesh.skin.as_ref() else {
            continue;
        };
        if skin.len() != mesh.vertices.len() {
            return Err(format!(
                "native skin stream length mismatch mesh='{}' skin={} vertices={}",
                mesh.name,
                skin.len(),
                mesh.vertices.len()
            ));
        }
        for (vertex_index, vertex) in skin.iter().enumerate() {
            let mut sum = 0.0_f32;
            for (joint, weight) in vertex
                .joints
                .iter()
                .zip(vertex.weights.iter())
                .chain(vertex.joints_extra.iter().zip(vertex.weights_extra.iter()))
            {
                if !weight.is_finite() || *weight < 0.0 {
                    return Err(format!(
                        "native skin has invalid weight mesh='{}' vertex={} weight={}",
                        mesh.name, vertex_index, weight
                    ));
                }
                if *weight > 0.0 && usize::from(*joint) >= skeleton_joint_count {
                    return Err(format!(
                        "native skin joint outside Abby skeleton mesh='{}' vertex={} joint={} joints={}",
                        mesh.name, vertex_index, joint, skeleton_joint_count
                    ));
                }
                sum += *weight;
            }
            if !sum.is_finite() || (sum - 1.0).abs() > WEIGHT_EPSILON {
                return Err(format!(
                    "native skin weights are not normalized mesh='{}' vertex={} sum={sum}",
                    mesh.name, vertex_index
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn encode_nef8(
    raw_body: &[u8],
    content_kind: u32,
    schema_version: u16,
    entry_count: u32,
) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(raw_body)
        .map_err(|error| format!("NEF8 deflate write failed: {error}"))?;
    let stored = encoder
        .finish()
        .map_err(|error| format!("NEF8 deflate finish failed: {error}"))?;
    encode_list_file(ListFileEncodeRequest {
        content_kind,
        content_schema_version: schema_version,
        entry_count,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: raw_body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

fn encode_skeleton_xml(skeleton: &DecodedSkeleton) -> Vec<u8> {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Metadata>\n");
    out.push_str(&format!(
        "  <Skeleton source_format=\"northstar.tlou2.pc.joint_hierarchy.v1\" name=\"{}\">\n",
        xml_escape(&skeleton.name)
    ));
    for joint in &skeleton.joints {
        let parent = joint
            .parent_index
            .map(|index| skeleton.joints[index as usize].name.as_str());
        out.push_str("    <Joint");
        out.push_str(&format!(
            " index=\"{}\" tag=\"{}\" name=\"{}\"",
            joint.index,
            joint.tag,
            xml_escape(&joint.name)
        ));
        if let Some(parent) = parent {
            out.push_str(&format!(
                " parent=\"{}\" parent_index=\"{}\"",
                xml_escape(parent),
                joint.parent_index.unwrap_or_default()
            ));
        } else {
            out.push_str(" parent_index=\"-1\"");
        }
        out.push_str(&format!(
            " tx=\"{:.9}\" ty=\"{:.9}\" tz=\"{:.9}\" qx=\"{:.9}\" qy=\"{:.9}\" qz=\"{:.9}\" qw=\"{:.9}\" sx=\"{:.9}\" sy=\"{:.9}\" sz=\"{:.9}\" />\n",
            joint.position_ls[0], joint.position_ls[1], joint.position_ls[2],
            joint.rotation_ls[0], joint.rotation_ls[1], joint.rotation_ls[2], joint.rotation_ls[3],
            joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2],
        ));
    }
    out.push_str(&format!(
        "    <Anchors root=\"{}\" hips=\"{}\" head=\"{}\" left_hand=\"{}\" right_hand=\"{}\" left_foot=\"{}\" right_foot=\"{}\" eye=\"{}\" eye_height=\"{:.6}\" />\n",
        xml_escape(&skeleton.root),
        xml_escape(&skeleton.hips),
        xml_escape(&skeleton.head),
        xml_escape(&skeleton.left_hand),
        xml_escape(&skeleton.right_hand),
        xml_escape(&skeleton.left_foot),
        xml_escape(&skeleton.right_foot),
        xml_escape(&skeleton.eye),
        skeleton.eye_height,
    ));
    out.push_str("  </Skeleton>\n</Metadata>\n");
    out.into_bytes()
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("failed to read '{}': {error}", path.display()))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!(
        "{}.importing",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("asset")
    ));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("failed to write '{}': {error}", temporary.display()))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("failed to replace '{}': {error}", path.display()))?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("failed to publish '{}': {error}", path.display()))
}
