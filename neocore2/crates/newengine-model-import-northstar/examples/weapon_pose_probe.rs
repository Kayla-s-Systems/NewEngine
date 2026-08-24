use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{fs, path::PathBuf};

fn main() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("pak path required")?;
    let pak = PakFile::parse(fs::read(&path).map_err(|e| e.to_string())?)?;
    let geometry = decode_geometry_lod0(&pak)?;
    let mut vertices = Vec::new();
    for mesh in &geometry.meshes {
        vertices.extend(mesh.vertices.iter().map(|v| v.position));
    }
    let min_y = vertices.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = vertices
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    println!(
        "meshes={} vertices={} y=[{:.6},{:.6}]",
        geometry.meshes.len(),
        vertices.len(),
        min_y,
        max_y
    );
    const BINS: usize = 24;
    for bin in 0..BINS {
        let y0 = min_y + (max_y - min_y) * bin as f32 / BINS as f32;
        let y1 = min_y + (max_y - min_y) * (bin + 1) as f32 / BINS as f32;
        let mut count = 0usize;
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        for p in &vertices {
            if p[1] >= y0 && (p[1] < y1 || bin + 1 == BINS) {
                count += 1;
                min_x = min_x.min(p[0]);
                max_x = max_x.max(p[0]);
                min_z = min_z.min(p[2]);
                max_z = max_z.max(p[2]);
            }
        }
        if count > 0 {
            println!("bin={bin:02} y=[{y0:.4},{y1:.4}] n={count:5} x=[{min_x:.4},{max_x:.4}] z=[{min_z:.4},{max_z:.4}] wx={:.4} wz={:.4}",max_x-min_x,max_z-min_z);
        }
    }
    Ok(())
}
