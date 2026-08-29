use std::{env, fs};
use newengine_model_import_northstar::{decode_geometry_scene_lod0, PakFile};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        match decode_geometry_scene_lod0(&pak) {
            Ok(scene) => {
                let render_defs = scene.definitions.iter().filter(|d| !d.lod0_mesh_indices.is_empty()).count();
                let render_instances = scene.instances.iter().filter(|i| !scene.definitions[i.definition_index].lod0_mesh_indices.is_empty()).count();
                let used = scene.definitions.iter().flat_map(|d| d.lod0_mesh_indices.iter().copied()).collect::<std::collections::BTreeSet<_>>();
                println!("SCENE_OK\t{source}\tmeshes={}\tused={}\tdefs={}\trender_defs={}\tinstances={}\trender_instances={}", scene.geometry.meshes.len(), used.len(), scene.definitions.len(), render_defs, scene.instances.len(), render_instances);
            }
            Err(error) => println!("SCENE_ERROR\t{source}\t{error}"),
        }
    }
    Ok(())
}
