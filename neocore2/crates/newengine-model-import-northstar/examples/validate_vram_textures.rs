use newengine_model_import_northstar::{decode_vram_textures, ImportedTextureFormat, PakFile};
use std::{env, fs};
fn main() -> Result<(), String> {
    let source = env::args()
        .nth(1)
        .ok_or("usage: validate_vram_textures PAK")?;
    let pak = PakFile::parse(fs::read(&source).map_err(|e| e.to_string())?)?;
    let textures = decode_vram_textures(&pak)?;
    println!("VRAM_VALIDATE descriptors={}", textures.len());
    let needles = [
        "muzzleflash-main-alpha.tga",
        "muzzleflash-main-emis3.tga",
        "sparks.tga",
        "concrete-bullet-hole-col.tga",
        "sphere-norm.tga",
    ];
    let mut found = 0usize;
    for needle in needles {
        let tex = textures
            .iter()
            .find(|t| t.source_path.contains(needle))
            .ok_or_else(|| format!("missing {needle}"))?;
        found += 1;
        match tex.base_linear_bytes(&pak) {
            Ok(bytes) => println!(
                "PASS path='{}' logical='{}' dxgi={} {}x{} linear_base_bytes={}",
                tex.source_path,
                tex.logical_name(),
                tex.format.dxgi(),
                tex.width,
                tex.height,
                bytes.len()
            ),
            Err(e) => println!(
                "REJECT path='{}' dxgi={} reason='{}'",
                tex.source_path,
                tex.format.dxgi(),
                e
            ),
        }
    }
    let supported = textures
        .iter()
        .filter(|t| t.format.validated_1d_thin_detile())
        .count();
    let bc5 = textures
        .iter()
        .filter(|t| matches!(t.format, ImportedTextureFormat::Bc5Unorm))
        .count();
    println!(
        "VRAM_VALIDATE PASS found={} supported64={} bc5_unvalidated={}",
        found, supported, bc5
    );
    Ok(())
}
