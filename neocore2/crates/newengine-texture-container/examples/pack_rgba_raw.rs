use newengine_texture_container::{generate_rgba8_mips, pack, TextureBuildEntry, COLOR_SPACE_SRGB};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or("missing input raw RGBA path")?);
    let output = PathBuf::from(args.next().ok_or("missing output NETD path")?);
    let width: u32 = args.next().ok_or("missing width")?.parse()?;
    let height: u32 = args.next().ok_or("missing height")?.parse()?;
    let name = args.next().ok_or("missing texture entry name")?;
    let rgba = fs::read(&input)?;
    let mips = generate_rgba8_mips(width, height, rgba)?;
    let bytes = pack(vec![TextureBuildEntry {
        name,
        width,
        height,
        color_space: COLOR_SPACE_SRGB.to_owned(),
        mips,
    }])?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, bytes)?;
    println!("NETD_OK {}", output.display());
    Ok(())
}
