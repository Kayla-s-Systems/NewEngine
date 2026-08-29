use std::{env, fs};

fn main() -> Result<(), String> {
    let a = env::args().nth(1).ok_or("first ytd")?;
    let b = env::args().nth(2).ok_or("second ytd")?;
    let skip = env::args().nth(3).unwrap_or_default();
    let read = |path: &str| -> Result<Vec<u8>, String> { fs::read(path).map_err(|e| e.to_string()) };
    let ab = read(&a)?;
    let bb = read(&b)?;
    let ad = newengine_assets_api::decode_list_file_envelope(&ab, newengine_asset_format_nef8::ytd::CONTENT_KIND, &a)?;
    let bd = newengine_assets_api::decode_list_file_envelope(&bb, newengine_asset_format_nef8::ytd::CONTENT_KIND, &b)?;
    let aa = newengine_texture_container::parse(&ad.body).map_err(|e| e.to_string())?;
    let ba = newengine_texture_container::parse(&bd.body).map_err(|e| e.to_string())?;
    if aa.entries().len() != ba.entries().len() { return Err("entry count differs".into()); }
    let mut compared = 0usize;
    for meta in aa.entries() {
        let other = ba.entries().iter().find(|m| m.name.eq_ignore_ascii_case(&meta.name)).ok_or_else(|| format!("missing {}", meta.name))?;
        if meta.name.eq_ignore_ascii_case(&skip) { continue; }
        if meta.width != other.width || meta.height != other.height || meta.format != other.format || meta.color_space != other.color_space || meta.mip_count != other.mip_count {
            return Err(format!("metadata differs for {}", meta.name));
        }
        let av = aa.entry(&meta.name).map_err(|e| e.to_string())?;
        let bv = ba.entry(&meta.name).map_err(|e| e.to_string())?;
        for mip in &meta.mips {
            if av.mip_bytes(mip.level) != bv.mip_bytes(mip.level) {
                return Err(format!("mip differs entry={} level={}", meta.name, mip.level));
            }
        }
        compared += 1;
    }
    println!("YTD_COMPARE_OK compared={} skipped='{}'", compared, skip);
    Ok(())
}
