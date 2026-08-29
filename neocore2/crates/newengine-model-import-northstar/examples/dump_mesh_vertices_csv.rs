use newengine_model_import_northstar::{decode_geometry_lod0, PakFile};
use std::{env, fs, io::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let pak_path = PathBuf::from(args.next().ok_or("pak")?);
    let needle = args.next().ok_or("mesh substring")?;
    let output = PathBuf::from(args.next().ok_or("output csv")?);
    let pak = PakFile::parse(fs::read(&pak_path).map_err(|e| e.to_string())?)?;
    let geom = decode_geometry_lod0(&pak)?;
    let matches = geom
        .meshes
        .iter()
        .filter(|mesh| mesh.name.contains(&needle))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(format!("no mesh containing '{needle}'"));
    }
    let mut file = fs::File::create(&output).map_err(|e| e.to_string())?;
    writeln!(file, "mesh,vertex,px,py,pz,u,v,nx,ny,nz").map_err(|e| e.to_string())?;
    for mesh in matches {
        for (index, vertex) in mesh.vertices.iter().enumerate() {
            writeln!(
                file,
                "\"{}\",{},{},{},{},{},{},{},{},{}",
                mesh.name.replace('"', "\""),
                index,
                vertex.position[0],
                vertex.position[1],
                vertex.position[2],
                vertex.uv0[0],
                vertex.uv0[1],
                vertex.normal[0],
                vertex.normal[1],
                vertex.normal[2],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    println!("wrote {}", output.display());
    Ok(())
}
