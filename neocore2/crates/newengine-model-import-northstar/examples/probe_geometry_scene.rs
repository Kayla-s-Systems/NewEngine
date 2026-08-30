use std::{env, fs};

use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        match decode_geometry_lod0(&pak) {
            Ok(geometry) => {
                let skinned = geometry
                    .meshes
                    .iter()
                    .filter(|mesh| mesh.skin.as_ref().is_some_and(|skin| !skin.is_empty()))
                    .count();
                let vertices = geometry
                    .meshes
                    .iter()
                    .map(|mesh| mesh.vertices.len())
                    .sum::<usize>();
                let triangles = geometry
                    .meshes
                    .iter()
                    .map(|mesh| mesh.indices.len() / 3)
                    .sum::<usize>();
                println!(
                    "GEOMETRY_OK\t{source}\tmeshes={}\tskinned={}\tvertices={}\ttriangles={}",
                    geometry.meshes.len(),
                    skinned,
                    vertices,
                    triangles,
                );
            }
            Err(error) => println!("GEOMETRY_ERROR\t{source}\t{error}"),
        }
    }
    Ok(())
}
