use newengine_model_import_northstar::{decode_vram_textures, PakFile};
use std::{env, fs};

fn main() -> Result<(), String> {
    for source in env::args().skip(1) {
        let pak = PakFile::parse(fs::read(&source).map_err(|e| format!("read {source}: {e}"))?)?;
        let textures = decode_vram_textures(&pak)?;
        println!("PACKAGE {source} textures={}", textures.len());
        for t in textures {
            println!("  {}x{} mips={} dxgi={} format={:?} type={} source='{}'", t.width, t.height, t.mip_count, t.format.dxgi(), t.format, t.texture_type, t.source_path);
        }
    }
    Ok(())
}
