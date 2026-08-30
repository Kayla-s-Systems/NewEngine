use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::{
    encode_ydd_binary_body,
    ydd_binary::{YddBinaryDocument, YddBinaryEntry, YddBinaryMesh},
    YDD_BINARY_SCHEMA_VERSION,
};
use newengine_assets_api::{encode_list_file, ListFileEncodeRequest};
use newengine_math::{EulerRot, Mat4};
use newengine_model_import_northstar::{
    decode_geometry_scene_lod0, GeometryModelDefinition, PakFile,
};

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[derive(Clone, Debug)]
struct CanonicalDefinition {
    fingerprint: u64,
    source_path: String,
    source_package: String,
    ydd_path: String,
    entry: String,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    mesh_count: usize,
    triangles: u64,
}

#[derive(Clone, Debug)]
struct Placement {
    fingerprint: u64,
    tier: &'static str,
    package: String,
    instance_index: usize,
    position: [f32; 3],
    rotation_ypr: [f32; 3],
    scale: [f32; 3],
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

fn hash_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        h ^= u64::from(byte);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn hash_str(h: u64, value: &str) -> u64 {
    hash_bytes(h, value.as_bytes())
}
fn hash_u32(h: u64, value: u32) -> u64 {
    hash_bytes(h, &value.to_le_bytes())
}
fn hash_u64(h: u64, value: u64) -> u64 {
    hash_bytes(h, &value.to_le_bytes())
}

fn material_hash(material: &str) -> u64 {
    hash_str(FNV_OFFSET, material.trim())
}

fn definition_fingerprint(
    scene: &newengine_model_import_northstar::DecodedGeometryScene,
    definition: &GeometryModelDefinition,
) -> u64 {
    let mut h = hash_str(
        FNV_OFFSET,
        definition.source_path.trim().to_ascii_lowercase().as_str(),
    );
    h = hash_u64(h, definition.lod0_mesh_indices.len() as u64);
    for &mesh_index in &definition.lod0_mesh_indices {
        let mesh = &scene.geometry.meshes[mesh_index];
        h = hash_str(h, &mesh.name);
        h = hash_str(h, mesh.source_material.as_deref().unwrap_or(""));
        h = hash_u64(h, mesh.vertices.len() as u64);
        h = hash_u64(h, mesh.indices.len() as u64);
        for vertex in &mesh.vertices {
            for value in vertex.position {
                h = hash_u32(h, value.to_bits());
            }
            for value in vertex.normal {
                h = hash_u32(h, value.to_bits());
            }
            for value in vertex.uv0 {
                h = hash_u32(h, value.to_bits());
            }
        }
        for &index in &mesh.indices {
            h = hash_u32(h, index);
        }
    }
    h
}

fn canonical_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("world")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn tier_for(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("wide") || lower.contains("far-background") {
        "far"
    } else if lower.contains("-mid")
        || lower.contains("_mid")
        || lower.contains("-low")
        || lower.contains("_low")
    {
        "mid"
    } else {
        "near"
    }
}

fn tier_priority(tier: &str) -> u8 {
    match tier {
        "near" => 3,
        "mid" => 2,
        _ => 1,
    }
}

fn eligible_package(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    (name.starts_with("fob-") || name.starts_with("aqu-") || name.starts_with("ski-"))
        && name.ends_with(".pak")
}

fn encode_ydd(document: &YddBinaryDocument) -> Result<Vec<u8>, String> {
    let body = encode_ydd_binary_body(document)?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    encode_list_file(ListFileEncodeRequest {
        content_kind: newengine_asset_format_nef8::ydd::CONTENT_KIND,
        content_schema_version: YDD_BINARY_SCHEMA_VERSION as u16,
        entry_count: document.entries.len() as u32,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

fn trs(transform: [f32; 16]) -> Result<([f32; 3], [f32; 3], [f32; 3], f32), String> {
    let matrix = Mat4::from_cols_array(&transform);
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    if !scale.is_finite() || !rotation.is_finite() || !translation.is_finite() {
        return Err("TRS decomposition produced non-finite values".into());
    }
    let rebuilt =
        Mat4::from_scale_rotation_translation(scale, rotation, translation).to_cols_array();
    let max_error = rebuilt
        .iter()
        .zip(transform.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    let (yaw, pitch, roll) = rotation.to_euler(EulerRot::YXZ);
    Ok((
        [translation.x, translation.y, translation.z],
        [yaw, pitch, roll],
        [scale.x, scale.y, scale.z],
        max_error,
    ))
}

fn placement_key(fingerprint: u64, transform: [f32; 16]) -> (u64, [i64; 16]) {
    let mut quantized = [0i64; 16];
    for (out, value) in quantized.iter_mut().zip(transform) {
        *out = (f64::from(value) * 10000.0).round() as i64;
    }
    (fingerprint, quantized)
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source_root=PathBuf::from(args.next().ok_or("usage: compile_forward_base_scene <pak-dir> <project-root> [model-dir-name] [research-dir-name]")?);
    let project = PathBuf::from(args.next().ok_or("project root required")?);
    let model_dir_name = args
        .next()
        .unwrap_or_else(|| "seattle_forward_base_scene_v1".into());
    let research_dir_name = args
        .next()
        .unwrap_or_else(|| "SeattleForwardBaseSceneV1".into());
    let content_dir = project.join("Content/models/maps").join(&model_dir_name);
    let research_dir = project.join("Source/Research").join(&research_dir_name);
    fs::create_dir_all(&content_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&research_dir).map_err(|e| e.to_string())?;

    let mut paths = fs::read_dir(&source_root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| eligible_package(p))
        .collect::<Vec<_>>();
    paths.sort();

    let mut canonical = BTreeMap::<u64, CanonicalDefinition>::new();
    let mut placements = BTreeMap::<(u64, [i64; 16]), Placement>::new();
    let mut errors = Vec::<String>::new();
    let mut package_rows = Vec::<String>::new();
    let mut all_materials = BTreeSet::<String>::new();
    let mut source_packages = 0usize;
    let mut source_meshes = 0usize;
    let mut source_instances = 0usize;
    let mut render_instances = 0usize;
    let mut duplicate_instances = 0usize;
    let mut trs_errors = 0usize;

    for path in paths {
        let package_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("source.pak")
            .to_owned();
        let bytes = match fs::read(&path) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}\tread\t{e}", path.display()));
                continue;
            }
        };
        let pak = match PakFile::parse(bytes) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}\tparse\t{e}", path.display()));
                continue;
            }
        };
        if pak.resource("GEOMETRY_1").is_none() {
            continue;
        }
        let scene = match decode_geometry_scene_lod0(&pak) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("{}\tgeometry_scene\t{e}", path.display()));
                continue;
            }
        };
        let render_defs = scene
            .definitions
            .iter()
            .filter(|d| !d.lod0_mesh_indices.is_empty())
            .count();
        if render_defs == 0 {
            continue;
        }
        source_packages += 1;
        source_meshes += scene.geometry.meshes.len();
        source_instances += scene.instances.len();
        for mesh in &scene.geometry.meshes {
            if let Some(m) = mesh.source_material.as_deref() {
                if !m.trim().is_empty() {
                    all_materials.insert(m.trim().to_owned());
                }
            }
        }
        let stem = canonical_stem(&path);
        let ydd_logical = format!("models/maps/{model_dir_name}/{stem}.ydd");
        let tier = tier_for(&package_name);
        let mut local_fingerprints = BTreeMap::<usize, u64>::new();
        let mut new_entries = Vec::<YddBinaryEntry>::new();
        let canonical_before = canonical.len();

        for definition in scene
            .definitions
            .iter()
            .filter(|d| !d.lod0_mesh_indices.is_empty())
        {
            let fingerprint = definition_fingerprint(&scene, definition);
            local_fingerprints.insert(definition.index, fingerprint);
            if canonical.contains_key(&fingerprint) {
                continue;
            }
            let entry = format!("d_{fingerprint:016x}");
            let meshes = definition
                .lod0_mesh_indices
                .iter()
                .map(|&mesh_index| {
                    let mesh = &scene.geometry.meshes[mesh_index];
                    let material_ref = mesh
                        .source_material
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(|m| {
                            format!(
                                "materials/maps/seattle_forward_base.nemat@mat_{:016x}",
                                material_hash(m)
                            )
                        })
                        .or_else(|| {
                            Some(
                                "materials/maps/seattle_forward_base.nemat@mat_fallback".to_owned(),
                            )
                        });
                    YddBinaryMesh {
                        name: mesh.name.clone(),
                        material_ref,
                        bounds_min: mesh.bounds_min,
                        bounds_max: mesh.bounds_max,
                        vertices: mesh.vertices.clone(),
                        skin: mesh.skin.clone(),
                        indices: mesh.indices.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let triangles = meshes.iter().map(|m| (m.indices.len() / 3) as u64).sum();
            let has_skin = meshes.iter().any(YddBinaryMesh::is_skinned);
            new_entries.push(YddBinaryEntry {
                name: entry.clone(),
                source_path: definition.source_path.clone(),
                properties_ref: None,
                bounds_min: definition.bounds_min,
                bounds_max: definition.bounds_max,
                // World GEOMETRY_1 skin streams (foliage/deformation-capable assets) are
                // already authored in the same model space as their definition. YDD V4
                // requires this contract whenever a skin stream is present.
                skin_source_to_model: has_skin.then_some([
                    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                ]),
                meshes,
            });
            canonical.insert(
                fingerprint,
                CanonicalDefinition {
                    fingerprint,
                    source_path: definition.source_path.clone(),
                    source_package: package_name.clone(),
                    ydd_path: ydd_logical.clone(),
                    entry,
                    bounds_min: definition.bounds_min,
                    bounds_max: definition.bounds_max,
                    mesh_count: definition.lod0_mesh_indices.len(),
                    triangles,
                },
            );
        }
        if !new_entries.is_empty() {
            let document = YddBinaryDocument {
                entries: new_entries,
            };
            let bytes = encode_ydd(&document)?;
            fs::write(content_dir.join(format!("{stem}.ydd")), bytes).map_err(|e| e.to_string())?;
        }

        let mut package_render_instances = 0usize;
        for instance in &scene.instances {
            let Some(&fingerprint) = local_fingerprints.get(&instance.definition_index) else {
                continue;
            };
            package_render_instances += 1;
            render_instances += 1;
            let (position, rotation_ypr, scale, error) = match trs(instance.transform) {
                Ok(v) => v,
                Err(e) => {
                    trs_errors += 1;
                    errors.push(format!(
                        "{}\tinstance_trs\t{} instance={} def={}",
                        path.display(),
                        e,
                        instance.index,
                        instance.definition_index
                    ));
                    continue;
                }
            };
            if error > 0.0025 {
                trs_errors += 1;
                errors.push(format!(
                    "{}\tinstance_shear\treconstruction_error={error:.6} instance={} def={}",
                    path.display(),
                    instance.index,
                    instance.definition_index
                ));
                continue;
            }
            let key = placement_key(fingerprint, instance.transform);
            let value = Placement {
                fingerprint,
                tier,
                package: package_name.clone(),
                instance_index: instance.index,
                position,
                rotation_ypr,
                scale,
                bounds_min: instance.world_bounds_min,
                bounds_max: instance.world_bounds_max,
            };
            match placements.get_mut(&key) {
                Some(existing) => {
                    duplicate_instances += 1;
                    if tier_priority(value.tier) > tier_priority(existing.tier) {
                        *existing = value;
                    }
                }
                None => {
                    placements.insert(key, value);
                }
            }
        }
        package_rows.push(format!(
            "{package_name}\t{tier}\t{}\t{}\t{}\t{}\t{}",
            scene.geometry.meshes.len(),
            render_defs,
            scene.instances.len(),
            package_render_instances,
            canonical.len() - canonical_before
        ));
        println!("SCENE_PACKAGE_OK package='{package_name}' tier={tier} meshes={} render_defs={render_defs} instances={} render_instances={package_render_instances} new_definitions={}",scene.geometry.meshes.len(),scene.instances.len(),canonical.len()-canonical_before);
    }

    let mut definitions_tsv=String::from("fingerprint\tsource_package\tsource_path\tydd_path\tentry\tmesh_count\ttriangles\tbounds_min\tbounds_max\n");
    for d in canonical.values() {
        definitions_tsv.push_str(&format!(
            "{:016x}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.6},{:.6},{:.6}\t{:.6},{:.6},{:.6}\n",
            d.fingerprint,
            d.source_package,
            d.source_path.replace(['\t', '\n', '\r'], " "),
            d.ydd_path,
            d.entry,
            d.mesh_count,
            d.triangles,
            d.bounds_min[0],
            d.bounds_min[1],
            d.bounds_min[2],
            d.bounds_max[0],
            d.bounds_max[1],
            d.bounds_max[2]
        ));
    }
    fs::write(research_dir.join("definitions.tsv"), definitions_tsv).map_err(|e| e.to_string())?;

    let mut placements_tsv=String::from("fingerprint\ttier\tsource_package\tinstance_index\tposition\trotation_ypr\tscale\tbounds_min\tbounds_max\n");
    for p in placements.values() {
        placements_tsv.push_str(&format!("{:016x}\t{}\t{}\t{}\t{:.7},{:.7},{:.7}\t{:.9},{:.9},{:.9}\t{:.7},{:.7},{:.7}\t{:.6},{:.6},{:.6}\t{:.6},{:.6},{:.6}\n",p.fingerprint,p.tier,p.package,p.instance_index,p.position[0],p.position[1],p.position[2],p.rotation_ypr[0],p.rotation_ypr[1],p.rotation_ypr[2],p.scale[0],p.scale[1],p.scale[2],p.bounds_min[0],p.bounds_min[1],p.bounds_min[2],p.bounds_max[0],p.bounds_max[1],p.bounds_max[2]));
    }
    fs::write(research_dir.join("placements.tsv"), placements_tsv).map_err(|e| e.to_string())?;
    fs::write(research_dir.join("packages.tsv"),format!("package\ttier\tmeshes\trender_definitions\tinstances\trender_instances\tnew_definitions\n{}\n",package_rows.join("\n"))).map_err(|e|e.to_string())?;
    fs::write(
        research_dir.join("materials.txt"),
        all_materials.iter().cloned().collect::<Vec<_>>().join("\n") + "\n",
    )
    .map_err(|e| e.to_string())?;
    fs::write(research_dir.join("errors.tsv"), errors.join("\n") + "\n")
        .map_err(|e| e.to_string())?;

    println!("FORWARD_BASE_SCENE_OK packages={source_packages} source_meshes={source_meshes} source_instances={source_instances} render_instances={render_instances} canonical_definitions={} placements={} duplicate_instances={duplicate_instances} materials={} trs_errors={trs_errors} errors={} content='{}' research='{}'",canonical.len(),placements.len(),all_materials.len(),errors.len(),content_dir.display(),research_dir.display());
    Ok(())
}
