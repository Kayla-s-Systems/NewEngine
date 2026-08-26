use newengine_model_import_northstar::{decode_vram_textures, ImportedTextureFormat, PakFile};
use std::{env, fs, path::PathBuf};

fn align_up(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}

fn write_tga(path: &PathBuf, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let mut out = Vec::with_capacity(18 + rgba.len());
    out.extend_from_slice(&[0, 0, 2]);
    out.extend_from_slice(&[0; 5]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(height as u16).to_le_bytes());
    out.push(32);
    out.push(0x28);
    for px in rgba.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    fs::write(path, out).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: extract_vram_linear_pitch_tga PAK SELECTOR OUT")?;
    let selector = args.next().ok_or("missing selector")?.to_ascii_lowercase();
    let output = PathBuf::from(args.next().ok_or("missing output")?);
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let textures = decode_vram_textures(&pak)?;
    let matches = textures
        .iter()
        .filter(|t| t.source_path.to_ascii_lowercase().contains(&selector))
        .collect::<Vec<_>>();
    let texture = match matches.as_slice() {
        [texture] => *texture,
        [] => return Err("no match".to_owned()),
        many => return Err(format!("ambiguous {}", many.len())),
    };
    if !matches!(
        texture.format,
        ImportedTextureFormat::Bc1Unorm | ImportedTextureFormat::Bc1Srgb
    ) {
        return Err(format!(
            "candidate only supports BC1, got {:?}",
            texture.format
        ));
    }
    let bw = texture.width.div_ceil(4) as usize;
    let bh = texture.height.div_ceil(4) as usize;
    let row_bytes = bw * 8;
    let row_pitch = align_up(row_bytes, 256);
    let physical_base = row_pitch * bh;
    if physical_base > texture.vram_size as usize {
        return Err(format!(
            "base {} > vram {}",
            physical_base, texture.vram_size
        ));
    }
    let raw = pak.slice(texture.absolute_data_offset, physical_base)?;
    let mut linear = Vec::with_capacity(row_bytes * bh);
    for y in 0..bh {
        let start = y * row_pitch;
        linear.extend_from_slice(&raw[start..start + row_bytes]);
    }
    let zero = linear
        .chunks_exact(8)
        .filter(|b| b.iter().all(|&v| v == 0))
        .count();
    let format = match texture.format {
        ImportedTextureFormat::Bc1Srgb => newengine_texture_container::PIXEL_FORMAT_BC1_RGBA_SRGB,
        _ => newengine_texture_container::PIXEL_FORMAT_BC1_RGBA_UNORM,
    };
    let rgba = newengine_texture_container::decode_bcn_to_rgba8(
        format,
        texture.width,
        texture.height,
        &linear,
    )
    .map_err(|e| e.to_string())?;
    let black = rgba
        .chunks_exact(4)
        .filter(|p| p[0] < 15 && p[1] < 15 && p[2] < 15)
        .count();
    println!("candidate path='{}' blocks={}x{} row_bytes={} row_pitch={} physical_base={} zero_blocks={} black_pixels={} black_frac={:.6}", texture.source_path, bw, bh, row_bytes, row_pitch, physical_base, zero, black, black as f64 / (texture.width as u64 * texture.height as u64) as f64);
    write_tga(&output, texture.width, texture.height, &rgba)?;
    Ok(())
}
