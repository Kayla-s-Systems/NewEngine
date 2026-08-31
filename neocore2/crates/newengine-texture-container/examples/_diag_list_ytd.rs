use std::{env, fs};

fn main() -> Result<(), String> {
    let path = env::args().nth(1).ok_or("file")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        &path,
    )?;
    let dict = newengine_texture_container::parse(&decoded.body).map_err(|e| e.to_string())?;
    for meta in dict.entries() {
        println!(
            "ENTRY {} {}x{} fmt={} cs={} mips={} total={}",
            meta.name,
            meta.width,
            meta.height,
            meta.format,
            meta.color_space,
            meta.mip_count,
            meta.byte_len
        );
        for mip in &meta.mips {
            println!(
                "  MIP level={} {}x{} off={} len={}",
                mip.level, mip.width, mip.height, mip.byte_offset, mip.byte_len
            );
        }
    }
    Ok(())
}
