use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| format!("read {source}: {e}"))?)?;
        let geometry = decode_geometry_lod0(&pak)?;
        println!("PACKAGE {source}");
        for mesh in geometry.meshes {
            println!(
                "  {}\n    material={}",
                mesh.name,
                mesh.source_material.as_deref().unwrap_or("<none>")
            );
        }
    }
    Ok(())
}
