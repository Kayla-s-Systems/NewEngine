use std::{env, fs, path::PathBuf, process};
fn main() {
    let mut a = env::args().skip(1);
    let id = a.next().unwrap_or_else(|| usage());
    let input = a.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let output = a.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let logical = a.next().unwrap_or_else(|| {
        input
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("asset")
            .to_owned()
    });
    let spec = newengine_migration_registry::migration(&id).unwrap_or_else(|| {
        eprintln!("unknown migration '{id}'");
        process::exit(2)
    });
    let bytes = fs::read(&input).unwrap_or_else(|e| fail(format!("read {}: {e}", input.display())));
    let migrated = newengine_migration_registry::migrate_bytes(spec, &bytes, &logical)
        .unwrap_or_else(|error| fail(error));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| fail(format!("create {}: {e}", parent.display())));
    }
    fs::write(&output, &migrated)
        .unwrap_or_else(|e| fail(format!("write {}: {e}", output.display())));
    newengine_migration_registry::verify_target(spec, &migrated, &logical)
        .unwrap_or_else(|error| fail(error));
    println!(
        "PASS migration={} source={} target={} source_repr={} target_repr={} output={} bytes={}",
        id,
        spec.source.version,
        spec.target.version,
        spec.source.representation_id.unwrap_or("<versioned>"),
        spec.target.representation_id.unwrap_or("<versioned>"),
        output.display(),
        migrated.len()
    );
}
fn usage() -> ! {
    eprintln!("usage: migrate_asset <migration-id> <input> <output> [logical-path]");
    process::exit(2)
}
fn fail<T: std::fmt::Display>(e: T) -> ! {
    eprintln!("FAIL {e}");
    process::exit(1)
}
