use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs};
fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
        let g = decode_geometry_lod0(&pak)?;
        println!("PAK {source} meshes={}", g.meshes.len());
        for (i, m) in g.meshes.iter().enumerate() {
            let mut uvmin = [f32::INFINITY; 2];
            let mut uvmax = [f32::NEG_INFINITY; 2];
            for v in &m.vertices {
                for a in 0..2 {
                    uvmin[a] = uvmin[a].min(v.uv0[a]);
                    uvmax[a] = uvmax[a].max(v.uv0[a]);
                }
            }
            println!("{i:03} name='{}' material={:?} verts={} tris={} bounds={:?}..{:?} uv={:?}..{:?} skinned={}",m.name,m.source_material,m.vertices.len(),m.indices.len()/3,m.bounds_min,m.bounds_max,uvmin,uvmax,m.skin.is_some());
        }
    }
    Ok(())
}
