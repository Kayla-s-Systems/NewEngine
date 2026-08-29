use std::{env, fs, path::PathBuf};

use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

#[inline]
fn hash_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

#[inline]
fn quantize(value: f32, scale: f32) -> i32 {
    (value * scale).round().clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn mesh_signature(mesh: &newengine_model_import_northstar::ImportMesh) -> u64 {
    let center = [
        (mesh.bounds_min[0] + mesh.bounds_max[0]) * 0.5,
        (mesh.bounds_min[1] + mesh.bounds_max[1]) * 0.5,
        (mesh.bounds_min[2] + mesh.bounds_max[2]) * 0.5,
    ];
    let mut h = FNV_OFFSET;
    h = hash_bytes(h, &(mesh.vertices.len() as u64).to_le_bytes());
    h = hash_bytes(h, &(mesh.indices.len() as u64).to_le_bytes());
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            h = hash_bytes(h, &quantize(vertex.position[axis] - center[axis], 4096.0).to_le_bytes());
            h = hash_bytes(h, &quantize(vertex.normal[axis], 32767.0).to_le_bytes());
        }
        for axis in 0..2 {
            h = hash_bytes(h, &quantize(vertex.uv0[axis], 4096.0).to_le_bytes());
        }
    }
    for &index in &mesh.indices {
        h = hash_bytes(h, &index.to_le_bytes());
    }
    h
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("usage: inventory_level_mesh_signatures <pak-dir> <output.tsv>")?);
    let output = PathBuf::from(args.next().ok_or("output path required")?);
    let mut packages = fs::read_dir(&root)
        .map_err(|e| format!("read_dir '{}': {e}", root.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("pak")))
        .collect::<Vec<_>>();
    packages.sort();

    let mut out = String::from("package\tmesh\tmaterial\tvertices\ttriangles\tcenter_x\tcenter_y\tcenter_z\textent_x\textent_y\textent_z\tsignature\n");
    let mut package_count = 0usize;
    let mut mesh_count = 0usize;
    for path in packages {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let pak = match PakFile::parse(bytes) {
            Ok(pak) => pak,
            Err(_) => continue,
        };
        if pak.resource("GEOMETRY_1").is_none() {
            continue;
        }
        let decoded = match decode_geometry_lod0(&pak) {
            Ok(decoded) => decoded,
            Err(_) => continue,
        };
        package_count += 1;
        let package = path.file_name().and_then(|s| s.to_str()).unwrap_or("source.pak");
        for mesh in decoded.meshes {
            let center = [
                (mesh.bounds_min[0] + mesh.bounds_max[0]) * 0.5,
                (mesh.bounds_min[1] + mesh.bounds_max[1]) * 0.5,
                (mesh.bounds_min[2] + mesh.bounds_max[2]) * 0.5,
            ];
            let extent = [
                mesh.bounds_max[0] - mesh.bounds_min[0],
                mesh.bounds_max[1] - mesh.bounds_min[1],
                mesh.bounds_max[2] - mesh.bounds_min[2],
            ];
            let material = mesh.source_material.as_deref().unwrap_or("").replace(['\t', '\n', '\r'], " ");
            let name = mesh.name.replace(['\t', '\n', '\r'], " ");
            let sig = mesh_signature(&mesh);
            out.push_str(&format!(
                "{package}\t{name}\t{material}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{sig:016x}\n",
                mesh.vertices.len(), mesh.indices.len() / 3,
                center[0], center[1], center[2], extent[0], extent[1], extent[2]
            ));
            mesh_count += 1;
        }
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, out).map_err(|e| format!("write '{}': {e}", output.display()))?;
    println!("MESH_SIGNATURE_INVENTORY_OK packages={package_count} meshes={mesh_count} output='{}'", output.display());
    Ok(())
}
