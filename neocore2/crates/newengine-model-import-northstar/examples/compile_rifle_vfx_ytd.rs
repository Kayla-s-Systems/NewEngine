use newengine_model_import_northstar::{
    compile_vfx_texture_dictionary, VfxTextureDictionaryCompileRequest, VfxTextureSelection,
};
use std::{env, path::PathBuf};
fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let package_path = PathBuf::from(
        args.next()
            .ok_or("usage: compile_rifle_vfx_ytd PART_SQUIBS.pak OUTPUT.ytd")?,
    );
    let output_path = PathBuf::from(args.next().ok_or("missing OUTPUT.ytd")?);
    let selections = vec![
        VfxTextureSelection::new("muzzleflash-main-emis3.tga/", "muzzle_main_emissive"),
        VfxTextureSelection::new("muzzleflash-main-alpha.tga/", "muzzle_main_alpha"),
        VfxTextureSelection::new("muzzleflash-main-emis3.tga/", "muzzle_main_rgba")
            .with_alpha_source("muzzleflash-main-alpha.tga/"),
        VfxTextureSelection::new("/misc/sparks.tga/", "impact_sparks"),
        VfxTextureSelection::new("concrete-bullet-hole-col.tga/", "impact_concrete_color"),
        VfxTextureSelection::new("concrete-bullet-hole-alpha.tga/", "impact_concrete_alpha"),
        VfxTextureSelection::new("concrete-bullet-hole-col.tga/", "impact_concrete_rgba")
            .with_alpha_source("concrete-bullet-hole-alpha.tga/"),
        VfxTextureSelection::new("dust-impact-sideways-a.tga/", "impact_dust_sideways"),
        VfxTextureSelection::new("concrete-bits-front-a.tga/", "impact_concrete_bits"),
    ];
    let report = compile_vfx_texture_dictionary(&VfxTextureDictionaryCompileRequest {
        package_path,
        output_path,
        selections,
    })?;
    println!(
        "VFX_YTD_COMPILE PASS output='{}' entries={} netd_bytes={} ytd_bytes={}",
        report.output_path.display(),
        report.entry_count,
        report.netd_bytes,
        report.ytd_bytes
    );
    for e in report.entries {
        println!(
            "  entry='{}' dxgi={} -> {} {}x{} mips={} source='{}'",
            e.entry_name,
            e.source_dxgi,
            e.output_format,
            e.width,
            e.height,
            e.mip_count,
            e.source_path
        );
    }
    Ok(())
}
