use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: p4_decode_neui_native_dto <artifact> <logical-path>");
        process::exit(2);
    });
    let logical_path = args.next().unwrap_or_else(|| "fixture.neui".to_owned());
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", path.display());
        process::exit(2);
    });
    let document_ref = format!("{logical_path}@surface");
    let root = newengine_assets_ui_runtime::compile_neui_bytes_surface_root(
        &bytes,
        &document_ref,
        None,
    )
    .unwrap_or_else(|error| {
        eprintln!("NEUI runtime semantic compile failed: {error}");
        process::exit(1);
    });
    println!("{}", serde_json::to_string(&root).expect("serialize NEUI runtime DTO"));
}
