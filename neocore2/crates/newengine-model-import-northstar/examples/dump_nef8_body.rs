use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let path = PathBuf::from(env::args().nth(1).ok_or("usage: dump_nef8_body FILE KIND")?);
    let kind: u32 = env::args().nth(2).ok_or("missing KIND")?.parse().map_err(|e| format!("invalid kind: {e}"))?;
    let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(&bytes, kind, path.to_string_lossy().as_ref())?;
    use std::io::Write;
    std::io::stdout().write_all(&decoded.body).map_err(|e| e.to_string())?;
    Ok(())
}
