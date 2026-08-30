use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs, path::PathBuf};

fn read_tga_rgba(path: &PathBuf) -> Result<(u32, u32, Vec<u8>), String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if bytes.len() < 18 || bytes[2] != 2 || bytes[16] != 32 {
        return Err(format!(
            "{} is not an uncompressed 32-bit TGA",
            path.display()
        ));
    }
    let width = u16::from_le_bytes([bytes[12], bytes[13]]) as u32;
    let height = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;
    let pixel_bytes = width as usize * height as usize * 4;
    if bytes.len() < 18 + pixel_bytes {
        return Err(format!("{} is truncated", path.display()));
    }
    let mut rgba = Vec::with_capacity(pixel_bytes);
    for bgra in bytes[18..18 + pixel_bytes].chunks_exact(4) {
        rgba.extend_from_slice(&[bgra[2], bgra[1], bgra[0], bgra[3]]);
    }
    Ok((width, height, rgba))
}

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let pak_path = PathBuf::from(
        args.next()
            .ok_or("usage: match_vram_source_tga PAK SOURCE.tga...")?,
    );
    let sources = args.map(PathBuf::from).collect::<Vec<_>>();
    if sources.is_empty() {
        return Err("at least one SOURCE.tga is required".to_owned());
    }
    let pak = PakFile::parse(fs::read(&pak_path).map_err(|e| e.to_string())?)?;
    let textures = decode_vram_textures(&pak)?;

    for source in sources {
        let (width, height, source_rgba) = read_tga_rgba(&source)?;
        let mut exact = Vec::new();
        let mut ranked = Vec::<(u64, String)>::new();
        for texture in &textures {
            if texture.width != width || texture.height != height {
                continue;
            }
            let Ok(rgba) = texture.base_rgba8(&pak) else {
                continue;
            };
            if rgba == source_rgba {
                exact.push(texture.source_path.clone());
                continue;
            }
            let error = rgba
                .iter()
                .zip(source_rgba.iter())
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum::<u64>();
            ranked.push((error, texture.source_path.clone()));
        }
        ranked.sort_unstable_by_key(|(error, _)| *error);
        println!("SOURCE {} {}x{}", source.display(), width, height);
        if exact.is_empty() {
            println!("  EXACT none");
            for (error, path) in ranked.into_iter().take(5) {
                println!("  NEAREST error={} source='{}'", error, path);
            }
        } else {
            for path in exact {
                println!("  EXACT source='{}'", path);
            }
        }
    }
    Ok(())
}
