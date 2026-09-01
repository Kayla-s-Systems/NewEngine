use flate2::{write::DeflateEncoder, Compression};
use newengine_asset_format_nef8::ydd_binary::{
    encode_ydd_binary_body, YddBinaryDocument, YddBinaryEntry, YddBinaryMesh, YddBinaryVertex,
};
use newengine_assets_api::{encode_list_file, ListFileEncodeRequest};
use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Default)]
struct Batch {
    source_material: String,
    vertices: Vec<YddBinaryVertex>,
    indices: Vec<u32>,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

fn fnv1a64(text: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
fn safe_stem(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
fn material_identity(source: Option<&str>) -> String {
    let raw = source
        .unwrap_or("northstar/fallback/world")
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    // Preserve material path + authored variant, discard GUID/shader trailer.
    raw.split(',').next().unwrap_or(&raw).trim().to_owned()
}
fn shadow_proxy(mesh_name: &str, material: &str) -> bool {
    let s = format!(
        "{} {}",
        mesh_name.to_ascii_lowercase(),
        material.to_ascii_lowercase()
    );
    ["shadowproxy", "shadow_proxy", "lib-shadowblocker"]
        .iter()
        .any(|token| s.contains(token))
}
fn cell_coord(center: [f32; 3], cell_size: f32) -> (i32, i32) {
    (
        (center[0] / cell_size).floor() as i32,
        (center[2] / cell_size).floor() as i32,
    )
}
fn package_is_world_candidate(stem: &str) -> bool {
    let s = stem.to_ascii_lowercase();
    if s.starts_with("part-") || s.starts_with("part_") {
        return false;
    }
    if s.ends_with("-phys") || s.ends_with("-ingame") {
        return false;
    }
    !["script", "audio", "crowd", "anim", "cinematic"]
        .iter()
        .any(|t| s.contains(t))
}
fn is_far_lod(stem: &str) -> bool {
    let s = stem.to_ascii_lowercase();
    s.contains("-wide") || s.ends_with("wide") || s.contains("far-background")
}
fn update_bounds(min: &mut [f32; 3], max: &mut [f32; 3], p: [f32; 3]) {
    for a in 0..3 {
        min[a] = min[a].min(p[a]);
        max[a] = max[a].max(p[a]);
    }
}
fn encode_ydd(entry: YddBinaryEntry, output: &Path) -> Result<(), String> {
    let body = encode_ydd_binary_body(&YddBinaryDocument {
        entries: vec![entry],
    })?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    let bytes = encode_list_file(ListFileEncodeRequest {
        content_kind: newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        content_schema_version: newengine_asset_format_nef8::YDD_BINARY_SCHEMA_VERSION as u16,
        entry_count: 1,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(output, bytes).map_err(|e| format!("write {}: {e}", output.display()))
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut source_root = None::<PathBuf>;
    let mut output_root = None::<PathBuf>;
    let mut manifest = None::<PathBuf>;
    let mut materials = None::<PathBuf>;
    let mut errors = None::<PathBuf>;
    let mut cell_size = 256.0f32;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source-root" => source_root = args.next().map(PathBuf::from),
            "--output-root" => output_root = args.next().map(PathBuf::from),
            "--manifest" => manifest = args.next().map(PathBuf::from),
            "--materials" => materials = args.next().map(PathBuf::from),
            "--errors" => errors = args.next().map(PathBuf::from),
            "--cell-size" => {
                cell_size = args
                    .next()
                    .ok_or("missing cell size")?
                    .parse()
                    .map_err(|_| "invalid cell size")?
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if !cell_size.is_finite() || cell_size < 32.0 {
        return Err("cell size must be finite and >= 32".into());
    }
    let source_root = source_root.ok_or("--source-root required")?;
    let output_root = output_root.ok_or("--output-root required")?;
    let manifest = manifest.ok_or("--manifest required")?;
    let materials_path = materials.ok_or("--materials required")?;
    let errors_path = errors.ok_or("--errors required")?;

    let mut pak_paths = fs::read_dir(&source_root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("pak"))
        })
        .collect::<Vec<_>>();
    pak_paths.sort();

    let mut manifest_rows = Vec::new();
    let mut material_rows = BTreeMap::<String, (String, BTreeSet<String>)>::new();
    let mut error_rows = Vec::new();
    let mut total_triangles = 0u64;
    let mut total_meshes = 0u64;
    let mut total_outputs = 0u64;

    for path in pak_paths {
        let stem = path.file_stem().and_then(|v| v.to_str()).unwrap_or("");
        if !package_is_world_candidate(stem) {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(v) => v,
            Err(e) => {
                error_rows.push(format!("{}\tread\t{}", path.display(), e));
                continue;
            }
        };
        let pak = match PakFile::parse(bytes) {
            Ok(v) => v,
            Err(e) => {
                error_rows.push(format!(
                    "{}\tparse\t{}",
                    path.display(),
                    e.replace('\n', " ")
                ));
                continue;
            }
        };
        if pak.resource("GEOMETRY_1").is_none() {
            continue;
        }
        let geometry = match decode_geometry_lod0(&pak) {
            Ok(v) => v,
            Err(e) => {
                error_rows.push(format!(
                    "{}\tgeometry\t{}",
                    path.display(),
                    e.replace('\n', " ")
                ));
                continue;
            }
        };
        let mut cells = BTreeMap::<(i32, i32), BTreeMap<String, Batch>>::new();
        for mesh in geometry.meshes {
            let material = material_identity(mesh.source_material.as_deref());
            if shadow_proxy(&mesh.name, &material) {
                continue;
            }
            let center = [
                (mesh.bounds_min[0] + mesh.bounds_max[0]) * 0.5,
                (mesh.bounds_min[1] + mesh.bounds_max[1]) * 0.5,
                (mesh.bounds_min[2] + mesh.bounds_max[2]) * 0.5,
            ];
            let coord = cell_coord(center, cell_size);
            let origin = [coord.0 as f32 * cell_size, 0.0, coord.1 as f32 * cell_size];
            let material_hash = format!("{:016x}", fnv1a64(&material));
            let entry = material_rows
                .entry(material_hash.clone())
                .or_insert_with(|| (material.clone(), BTreeSet::new()));
            entry.1.insert(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            );
            let batch = cells
                .entry(coord)
                .or_default()
                .entry(material_hash)
                .or_insert_with(|| Batch {
                    source_material: material.clone(),
                    bounds_min: [f32::INFINITY; 3],
                    bounds_max: [f32::NEG_INFINITY; 3],
                    ..Batch::default()
                });
            let base = u32::try_from(batch.vertices.len())
                .map_err(|_| "cell batch vertex address overflow")?;
            for mut v in mesh.vertices {
                v.position[0] -= origin[0];
                v.position[1] -= origin[1];
                v.position[2] -= origin[2];
                update_bounds(&mut batch.bounds_min, &mut batch.bounds_max, v.position);
                batch.vertices.push(v);
            }
            batch
                .indices
                .extend(mesh.indices.into_iter().map(|i| base + i));
            total_meshes += 1;
        }

        for ((cx, cz), batches) in cells {
            if batches.is_empty() {
                continue;
            }
            let entry_name = safe_stem(&format!("{}-c{}-{}", stem, cx, cz));
            let mut entry_min = [f32::INFINITY; 3];
            let mut entry_max = [f32::NEG_INFINITY; 3];
            let mut meshes = Vec::with_capacity(batches.len());
            let mut triangles = 0u64;
            for (material_hash, batch) in batches {
                triangles += (batch.indices.len() / 3) as u64;
                for a in 0..3 {
                    entry_min[a] = entry_min[a].min(batch.bounds_min[a]);
                    entry_max[a] = entry_max[a].max(batch.bounds_max[a]);
                }
                meshes.push(YddBinaryMesh {
                    name: format!("mat_{material_hash}"),
                    material_ref: Some(format!(
                        "materials/forward_base/native/{material_hash}.nemat@mat_{material_hash}"
                    )),
                    bounds_min: batch.bounds_min,
                    bounds_max: batch.bounds_max,
                    vertices: batch.vertices,
                    skin: None,
                    indices: batch.indices,
                });
            }
            let file_name = format!("{entry_name}.ydd");
            let output = output_root.join(&file_name);
            encode_ydd(
                YddBinaryEntry {
                    name: entry_name.clone(),
                    source_path: path.display().to_string(),
                    properties_ref: None,
                    bounds_min: entry_min,
                    bounds_max: entry_max,
                    skin_source_to_model: None,
                    meshes,
                },
                &output,
            )?;
            let origin_x = cx as f32 * cell_size;
            let origin_z = cz as f32 * cell_size;
            manifest_rows.push(format!(
                "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}",
                cx,
                cz,
                path.file_name().unwrap_or_default().to_string_lossy(),
                file_name,
                origin_x,
                origin_z,
                entry_name,
                triangles,
                if is_far_lod(stem) {
                    "far_lod"
                } else {
                    "streamed"
                },
                output.display()
            ));
            total_triangles += triangles;
            total_outputs += 1;
        }
        println!(
            "WORLD_PACKAGE_OK source='{}' outputs_so_far={} triangles_so_far={}",
            path.display(),
            total_outputs,
            total_triangles
        );
    }
    manifest_rows.sort();
    let mut manifest_text = String::from("cell_x\tcell_z\tsource_package\tydd_file\torigin_x\torigin_z\tentry\ttriangles\tstreaming_class\toutput\n");
    for row in manifest_rows {
        manifest_text.push_str(&row);
        manifest_text.push('\n');
    }
    if let Some(parent) = manifest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&manifest, manifest_text).map_err(|e| e.to_string())?;
    let mut mat_text = String::from("material_hash\tsource_material\tpackages\n");
    for (hash, (source, packages)) in material_rows {
        mat_text.push_str(&format!(
            "{}\t{}\t{}\n",
            hash,
            source,
            packages.into_iter().collect::<Vec<_>>().join(";")
        ));
    }
    fs::write(&materials_path, mat_text).map_err(|e| e.to_string())?;
    fs::write(&errors_path, error_rows.join("\n")).map_err(|e| e.to_string())?;
    println!("FORWARD_BASE_WORLD_OK outputs={} source_meshes={} triangles={} manifest='{}' materials='{}' errors={}", total_outputs,total_meshes,total_triangles,manifest.display(),materials_path.display(),error_rows.len());
    Ok(())
}
