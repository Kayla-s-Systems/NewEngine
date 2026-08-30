use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        let geometry = decode_geometry_lod0(&pak)?;
        println!("PACKAGE\t{}\tmeshes={}", source, geometry.meshes.len());
        for mesh in geometry.meshes {
            println!(
                "MESH\t{}\tmaterial={}\tskin_domain={}\tvertices={}\tindices={}",
                mesh.name,
                mesh.source_material.as_deref().unwrap_or("<none>"),
                mesh.source_skin_joint_domain_size.map(|v| v.to_string()).unwrap_or_else(|| "<none>".to_owned()),
                mesh.vertices.len(),
                mesh.indices.len(),
            );
        }
    }
    Ok(())
}
