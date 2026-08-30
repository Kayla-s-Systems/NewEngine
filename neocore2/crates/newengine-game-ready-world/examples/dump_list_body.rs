use std::{env, fs};
fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: dump_list_body FILE KIND")?;
    let kind = env::args()
        .nth(2)
        .ok_or("missing kind")?
        .parse::<u32>()
        .map_err(|e| e.to_string())?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(&bytes, kind, &path)?;
    print!("{}", String::from_utf8_lossy(&decoded.body));
    Ok(())
}
