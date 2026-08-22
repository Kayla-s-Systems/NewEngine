use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: p4_decode_ytyp_native_dto <artifact> <logical-path>");
        process::exit(2);
    });
    let logical_path = args.next().unwrap_or_else(|| "fixture.ytyp".to_owned());
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", path.display());
        process::exit(2);
    });
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &bytes,
        newengine_assets_api::LIST_FILE_CONTENT_KIND_YTYP,
        &logical_path,
    )
    .unwrap_or_else(|error| {
        eprintln!("canonical NEF8 decode failed: {error}");
        process::exit(1);
    });
    let dto = newengine_definitions_runtime::decode_ytyp_definition_entries_from_body(
        &logical_path,
        &decoded.body,
    )
    .unwrap_or_else(|error| {
        eprintln!("YTYP runtime domain decode failed: {error}");
        process::exit(1);
    });
    println!("{}", serde_json::to_string(&dto).expect("serialize YTYP runtime DTO"));
}
