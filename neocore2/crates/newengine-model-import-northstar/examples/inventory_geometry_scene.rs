use newengine_model_import_northstar::{decode_geometry_scene_lod0, PakFile};
use std::{collections::BTreeSet, env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let root = PathBuf::from(
        env::args()
            .nth(1)
            .ok_or("usage: inventory_geometry_scene <pak-dir>")?,
    );
    let mut paths = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("pak"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    let mut packages = 0usize;
    let mut errors = Vec::new();
    let mut meshes = 0usize;
    let mut referenced_meshes = 0usize;
    let mut definitions = 0usize;
    let mut instances = 0usize;
    let mut triangles = 0u64;
    let mut referenced_triangles = 0u64;
    for path in paths {
        let bytes = match fs::read(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let pak = match PakFile::parse(bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if pak.resource("GEOMETRY_1").is_none() {
            continue;
        }
        match decode_geometry_scene_lod0(&pak) {
            Ok(scene) => {
                packages += 1;
                definitions += scene.definitions.len();
                instances += scene.instances.len();
                meshes += scene.geometry.meshes.len();
                triangles += scene
                    .geometry
                    .meshes
                    .iter()
                    .map(|m| (m.indices.len() / 3) as u64)
                    .sum::<u64>();
                let used = scene
                    .definitions
                    .iter()
                    .flat_map(|d| d.lod0_mesh_indices.iter().copied())
                    .collect::<BTreeSet<_>>();
                referenced_meshes += used.len();
                referenced_triangles += used
                    .iter()
                    .map(|&i| (scene.geometry.meshes[i].indices.len() / 3) as u64)
                    .sum::<u64>();
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                println!("SCENE_OK\t{name}\tmeshes={}\tused={}\tdefs={}\tinstances={}\ttris={}\tused_tris={}",scene.geometry.meshes.len(),used.len(),scene.definitions.len(),scene.instances.len(),scene.geometry.meshes.iter().map(|m|m.indices.len()/3).sum::<usize>(),used.iter().map(|&i|scene.geometry.meshes[i].indices.len()/3).sum::<usize>());
            }
            Err(e) => {
                errors.push(format!("{}\t{}", path.display(), e));
            }
        }
    }
    println!("SCENE_CORPUS_OK packages={packages} meshes={meshes} referenced_meshes={referenced_meshes} definitions={definitions} instances={instances} triangles={triangles} referenced_triangles={referenced_triangles} errors={}",errors.len());
    for e in errors {
        println!("ERROR\t{e}");
    }
    Ok(())
}
