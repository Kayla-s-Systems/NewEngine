use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs};
fn main() {
    for source in env::args().skip(1) {
        let result = (|| -> Result<(), String> {
            let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
            if pak.resource("GEOMETRY_1").is_none() {
                println!("NO_GEOMETRY\t{}", source);
                return Ok(());
            }
            let geo = decode_geometry_lod0(&pak)?;
            let vertices: usize = geo.meshes.iter().map(|m| m.vertices.len()).sum();
            let triangles: usize = geo.meshes.iter().map(|m| m.indices.len() / 3).sum();
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for m in &geo.meshes {
                for a in 0..3 {
                    min[a] = min[a].min(m.bounds_min[a]);
                    max[a] = max[a].max(m.bounds_max[a]);
                }
            }
            println!(
                "GEOMETRY\t{}\tmeshes={}\tvertices={}\ttriangles={}\tbounds={:?}..{:?}",
                source,
                geo.meshes.len(),
                vertices,
                triangles,
                min,
                max
            );
            Ok(())
        })();
        if let Err(e) = result {
            println!("ERROR\t{}\t{}", source, e.replace('\n', " "));
        }
    }
}
