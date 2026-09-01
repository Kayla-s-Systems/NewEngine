use std::{env, fs};
fn main() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: dump_ydd_mesh_names FILE.ydd [logical]")?;
    let logical = env::args().nth(2).unwrap_or_else(|| "model.ydd".to_owned());
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD,
        &logical,
    )?;
    let doc = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(&decoded.body)?;
    for entry in doc.entries {
        println!("ENTRY '{}' meshes={}", entry.name, entry.meshes.len());
        for (i, m) in entry.meshes.iter().enumerate() {
            println!("{i}\t{}\t{:?}", m.name, m.material_ref);
        }
    }
    Ok(())
}
