use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let path = PathBuf::from(env::args().nth(1).ok_or("usage: dump_ymt FILE.ymt")?);
    let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YMT,
        path.to_string_lossy().as_ref(),
    )?;
    let text = String::from_utf8(decoded.body).map_err(|e| e.to_string())?;
    print!("{text}");
    Ok(())
}
