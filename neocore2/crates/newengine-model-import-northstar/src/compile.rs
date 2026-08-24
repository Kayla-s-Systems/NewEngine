use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::ydd_binary::{
    encode_ydd_binary_body, YddBinaryDocument, YddBinaryEntry, YddBinaryMesh,
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
    /// Optional per-package mesh prefixes. If a package has one or more entries here,
    /// only meshes whose decoded name starts with one of those prefixes are imported.
    pub package_mesh_prefixes: Vec<(PathBuf, String)>,
    /// Optional mesh-prefix to canonical material-ref overrides. Longest prefix wins.
    pub material_overrides: Vec<(String, String)>,
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

    let mut meshes = Vec::new();
    let mut skin_loss = SkinLossStats::default();
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
        for mesh in decoded.meshes {
            if !package_filters.is_empty()
                && !package_filters
                    .iter()
                    .any(|prefix| mesh.name.starts_with(prefix))
            {
                continue;
            }
            validate_skin_joint_range(&mesh, skeleton.joints.len(), path)?;
            meshes.push(mesh);
        }
    }
    if meshes.is_empty() {
        return Err("character import produced no LOD0 meshes".to_owned());
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
                    request
                        .material_library_ref
                        .as_ref()
                        .map(|library| format!("{}@m{:02}", library.trim_end_matches('@'), index))
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
            // Geometry and JOINT_HIERARCHY are decoded in one authored source space.
            skin_source_to_model: Some([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]),
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

fn encode_nef8(
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
