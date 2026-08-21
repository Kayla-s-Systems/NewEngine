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

use crate::geometry::{decode_geometry_lod0, SkinLossStats};
use crate::pak::PakFile;
use crate::skeleton::{decode_skeleton, DecodedSkeleton};

#[derive(Clone, Debug)]
pub struct CharacterCompileRequest {
    pub name: String,
    pub package_paths: Vec<PathBuf>,
    pub skeleton_path: PathBuf,
    pub output_dir: PathBuf,
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
    let skeleton = decode_skeleton(&skeleton_pak)?;

    let mut meshes = Vec::new();
    let mut skin_loss = SkinLossStats::default();
    for path in &request.package_paths {
        let pak = PakFile::parse(read_file(path)?)?;
        let decoded =
            decode_geometry_lod0(&pak).map_err(|error| format!("{}: {error}", path.display()))?;
        skin_loss.merge(decoded.skin_loss);
        for mesh in decoded.meshes {
            validate_skin_joint_range(&mesh, skeleton.joints.len(), path)?;
            meshes.push(mesh);
        }
    }
    if meshes.is_empty() {
        return Err("character import produced no LOD0 meshes".to_owned());
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
    let native_meshes = meshes
        .iter()
        .map(|mesh| YddBinaryMesh {
            name: mesh.name.clone(),
            material_ref: None,
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
            source_path: format!("naughtydog.tlou2.pc://{source_path}"),
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
        "  <Skeleton source_format=\"naughtydog.tlou2.pc.joint_hierarchy.v1\" name=\"{}\">\n",
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
