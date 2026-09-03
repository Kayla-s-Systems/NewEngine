use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{
    env, fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

fn clean_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let pak_path = PathBuf::from(
        args.next()
            .ok_or("usage: export_level_obj INPUT.pak OUTPUT.obj OUTPUT.materials.tsv")?,
    );
    let obj_path = PathBuf::from(args.next().ok_or("missing OUTPUT.obj")?);
    let tsv_path = PathBuf::from(args.next().ok_or("missing OUTPUT.materials.tsv")?);
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let pak = PakFile::parse(
        fs::read(&pak_path).map_err(|e| format!("read {}: {e}", pak_path.display()))?,
    )?;
    let geometry = decode_geometry_lod0(&pak)?;
    if let Some(parent) = obj_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Some(parent) = tsv_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut obj = BufWriter::new(fs::File::create(&obj_path).map_err(|e| e.to_string())?);
    let mut tsv = BufWriter::new(fs::File::create(&tsv_path).map_err(|e| e.to_string())?);
    writeln!(obj, "# NorthStar stadium native geometry export").map_err(|e| e.to_string())?;
    writeln!(obj, "# source: {}", pak_path.display()).map_err(|e| e.to_string())?;
    writeln!(tsv, "mesh_index\tmesh_name\tmaterial_id\tsource_material")
        .map_err(|e| e.to_string())?;

    let mut base: u64 = 1;
    let mut vertices = 0usize;
    let mut triangles = 0usize;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for (mesh_index, mesh) in geometry.meshes.iter().enumerate() {
        let mat = format!("mat_{mesh_index:04}");
        let group = format!("m{mesh_index:04}_{}", clean_name(&mesh.name));
        writeln!(obj, "g {group}").map_err(|e| e.to_string())?;
        writeln!(obj, "usemtl {mat}").map_err(|e| e.to_string())?;
        for v in &mesh.vertices {
            writeln!(
                obj,
                "v {:.9} {:.9} {:.9}",
                v.position[0], v.position[1], v.position[2]
            )
            .map_err(|e| e.to_string())?;
            for a in 0..3 {
                min[a] = min[a].min(v.position[a]);
                max[a] = max[a].max(v.position[a]);
            }
        }
        for v in &mesh.vertices {
            writeln!(obj, "vt {:.9} {:.9}", v.uv0[0], v.uv0[1]).map_err(|e| e.to_string())?;
        }
        for v in &mesh.vertices {
            writeln!(
                obj,
                "vn {:.9} {:.9} {:.9}",
                v.normal[0], v.normal[1], v.normal[2]
            )
            .map_err(|e| e.to_string())?;
        }
        for tri in mesh.indices.as_chunks::<3>().0 {
            let a = base + u64::from(tri[0]);
            let b = base + u64::from(tri[1]);
            let c = base + u64::from(tri[2]);
            writeln!(obj, "f {a}/{a}/{a} {b}/{b}/{b} {c}/{c}/{c}").map_err(|e| e.to_string())?;
            triangles += 1;
        }
        let source = mesh
            .source_material
            .as_deref()
            .unwrap_or("")
            .replace(['\t', '\r', '\n'], " ");
        let mesh_name = mesh.name.replace(['\t', '\r', '\n'], " ");
        writeln!(tsv, "{mesh_index}\t{mesh_name}\t{mat}\t{source}").map_err(|e| e.to_string())?;
        base += mesh.vertices.len() as u64;
        vertices += mesh.vertices.len();
    }
    obj.flush().map_err(|e| e.to_string())?;
    tsv.flush().map_err(|e| e.to_string())?;
    println!("LEVEL_OBJ_EXPORT_OK meshes={} vertices={} triangles={} bounds={:?}..{:?} obj='{}' tsv='{}'",
        geometry.meshes.len(), vertices, triangles, min, max, obj_path.display(), tsv_path.display());
    Ok(())
}
