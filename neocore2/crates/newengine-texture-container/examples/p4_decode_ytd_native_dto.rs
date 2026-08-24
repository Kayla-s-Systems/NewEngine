use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: p4_decode_ytd_native_dto <artifact> <logical-path>");
        process::exit(2);
    });
    let logical_path = args.next().unwrap_or_else(|| "fixture.ytd".to_owned());
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", path.display());
        process::exit(2);
    });
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        &logical_path,
    )
    .unwrap_or_else(|error| {
        eprintln!("canonical NEF8 decode failed: {error}");
        process::exit(1);
    });
    let dictionary = newengine_texture_container::parse(&decoded.body).unwrap_or_else(|error| {
        eprintln!("YTD runtime domain decode failed: {error}");
        process::exit(1);
    });
    println!(
        "{}",
        serde_json::to_string(dictionary.manifest()).expect("serialize YTD runtime DTO")
    );
}
