use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs, path::PathBuf};

fn write_tga(path: &PathBuf, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let expected = width as usize * height as usize * 4;
    if rgba.len() != expected {
        return Err(format!("rgba bytes={} expected={expected}", rgba.len()));
    }
    let mut out = Vec::with_capacity(18 + expected);
    out.extend_from_slice(&[0, 0, 2]);
    out.extend_from_slice(&[0; 5]);
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(width as u16).to_le_bytes());
    out.extend_from_slice(&(height as u16).to_le_bytes());
    out.push(32);
    out.push(0x28); // 8-bit alpha + top-left origin.
    for px in rgba.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, out).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let pak_path = PathBuf::from(args.next().ok_or("usage: extract_vram_texture_tga PAK CONTAINS OUT.tga")?);
    let selector = args.next().ok_or("missing selector")?.to_ascii_lowercase();
    let output = PathBuf::from(args.next().ok_or("missing output")?);
    let pak = PakFile::parse(fs::read(&pak_path).map_err(|e| format!("read {}: {e}", pak_path.display()))?)?;
    let textures = decode_vram_textures(&pak)?;
    let matches = textures.iter().filter(|t| t.source_path.to_ascii_lowercase().contains(&selector)).collect::<Vec<_>>();
    let texture = match matches.as_slice() {
        [texture] => *texture,
        [] => return Err(format!("no texture contains '{selector}' in {}", pak_path.display())),
        many => return Err(format!("selector '{selector}' ambiguous matches={}: {}", many.len(), many.iter().map(|t| t.source_path.as_str()).collect::<Vec<_>>().join(" | "))),
    };
    let rgba = texture.base_rgba8(&pak)?;
    let opaque = rgba.chunks_exact(4).filter(|p| p[3] == 255).count();
    let transparent = rgba.chunks_exact(4).filter(|p| p[3] == 0).count();
    let partial = rgba.len() / 4 - opaque - transparent;
    write_tga(&output, texture.width, texture.height, &rgba)?;
    println!("EXTRACT source='{}' {}x{} format={:?} alpha[opaque={} transparent={} partial={}] output='{}'", texture.source_path, texture.width, texture.height, texture.format, opaque, transparent, partial, output.display());
    Ok(())
}
