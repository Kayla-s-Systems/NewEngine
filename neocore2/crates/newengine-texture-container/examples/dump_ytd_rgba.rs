use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("usage: dump_ytd_rgba FILE.ytd ENTRY OUT.rgba")?,
    );
    let entry_name = args.next().ok_or("missing entry")?;
    let out = PathBuf::from(args.next().ok_or("missing output")?);
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let logical = path.to_string_lossy();
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD,
        &logical,
    )?;
    let dict = newengine_texture_container::parse(&decoded.body).map_err(|e| e.to_string())?;
    let entry = dict.entry(&entry_name).map_err(|e| e.to_string())?;
    let src = entry.base_mip_bytes().ok_or("missing base mip")?;
    let rgba = if entry.meta.format.starts_with("BC") {
        newengine_texture_container::decode_bcn_to_rgba8(
            &entry.meta.format,
            entry.meta.width,
            entry.meta.height,
            src,
        )
        .map_err(|e| e.to_string())?
    } else {
        src.to_vec()
    };
    fs::write(&out, &rgba).map_err(|e| e.to_string())?;
    println!(
        "{} {}x{} {} bytes={} out={}",
        entry.meta.name,
        entry.meta.width,
        entry.meta.height,
        entry.meta.format,
        rgba.len(),
        out.display()
    );
    Ok(())
}
