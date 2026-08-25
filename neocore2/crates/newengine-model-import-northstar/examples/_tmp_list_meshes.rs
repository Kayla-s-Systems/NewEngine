use newengine_model_import_northstar::{decode_geometry_lod0, decode_skeleton, PakFile};
use std::{env, fs, path::PathBuf};
fn main() {
    let path = PathBuf::from(env::args().nth(1).expect("pak"));
    let pak = PakFile::parse(fs::read(&path).unwrap()).unwrap();
    if let Ok(s) = decode_skeleton(&pak) {
        println!("SKELETON name={} joints={}", s.name, s.joints.len());
    }
    match decode_geometry_lod0(&pak) {
        Ok(g) => {
            println!("MESHES {}", g.meshes.len());
            for (i, m) in g.meshes.iter().enumerate() {
                println!(
                    "{i:02} {} v={} i={} bounds={:?}..{:?}",
                    m.name,
                    m.vertices.len(),
                    m.indices.len(),
                    m.bounds_min,
                    m.bounds_max
                );
            }
        }
        Err(e) => println!("NO_GEOMETRY {e}"),
    }
}
