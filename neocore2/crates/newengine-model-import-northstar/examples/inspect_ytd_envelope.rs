use std::{env, fs};

fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: inspect_ytd_envelope <file>")?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD,
        &path,
    )?;
    println!("header={:?}", decoded.header);
    println!("metadata={:#?}", decoded.metadata);
    Ok(())
}
