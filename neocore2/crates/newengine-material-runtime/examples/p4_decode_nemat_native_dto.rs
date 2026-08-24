use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: p4_decode_nemat_native_dto <artifact> <logical-path>");
        process::exit(2);
    });
    let logical_path = args.next().unwrap_or_else(|| "fixture.nemat".to_owned());
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", path.display());
        process::exit(2);
    });
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_asset_format_nef8::nemat::CONTENT_KIND,
        &logical_path,
    )
    .unwrap_or_else(|error| {
        eprintln!("canonical NEF8 decode failed: {error}");
        process::exit(1);
    });
    let library =
        newengine_material_runtime::decode_nemat_material_library_from_body(&decoded.body)
            .unwrap_or_else(|error| {
                eprintln!("NEMAT runtime domain decode failed: {error}");
                process::exit(1);
            });
    println!(
        "{}",
        serde_json::to_string(&library).expect("serialize NEMAT runtime DTO")
    );
}
