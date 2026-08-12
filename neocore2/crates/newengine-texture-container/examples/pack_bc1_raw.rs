use std::{env, fs, path::PathBuf};

use newengine_texture_container::{
    encode_rgba8_mips_to_bcn, generate_rgba8_mips, pack_encoded, TextureEncodedBuildEntry,
    COLOR_SPACE_SRGB, PIXEL_FORMAT_BC1_RGBA_SRGB,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 6 {
        return Err(
            "usage: pack_bc1_raw <input.rgba> <output.netd> <width> <height> <entry>".into(),
        );
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let width: u32 = args[3].parse()?;
    let height: u32 = args[4].parse()?;
    let name = args[5].clone();
    let rgba = fs::read(input)?;
    let mips = generate_rgba8_mips(width, height, rgba)?;
    let encoded = encode_rgba8_mips_to_bcn(PIXEL_FORMAT_BC1_RGBA_SRGB, &mips)?;
    let bytes = pack_encoded(vec![TextureEncodedBuildEntry {
        name,
        width,
        height,
        format: PIXEL_FORMAT_BC1_RGBA_SRGB.to_owned(),
        color_space: COLOR_SPACE_SRGB.to_owned(),
        mips: encoded,
    }])?;
    fs::write(&output, bytes)?;
    println!("NETD_BC1_OK {}", output.display());
    Ok(())
}
