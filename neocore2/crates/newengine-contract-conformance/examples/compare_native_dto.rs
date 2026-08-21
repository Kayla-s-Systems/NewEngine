use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let spec_id = args.next().unwrap_or_else(|| {
        eprintln!("usage: compare_native_dto <spec-id> <logical-path> <asset-manager-json> <runtime-json>");
        process::exit(2);
    });
    let logical_path = args.next().unwrap_or_else(|| {
        eprintln!("missing logical path");
        process::exit(2);
    });
    let asset_manager_path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("missing AssetManager JSON path");
        process::exit(2);
    });
    let runtime_path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("missing runtime JSON path");
        process::exit(2);
    });
    let spec = newengine_contract_conformance::tool_runtime_conformance_spec(&spec_id)
        .unwrap_or_else(|| {
            eprintln!("unknown ToolRuntimeConformanceSpec '{spec_id}'");
            process::exit(2);
        });
    let asset_manager = fs::read(&asset_manager_path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", asset_manager_path.display());
        process::exit(2);
    });
    let runtime = fs::read(&runtime_path).unwrap_or_else(|error| {
        eprintln!("read {} failed: {error}", runtime_path.display());
        process::exit(2);
    });
    match newengine_contract_conformance::validate_native_dto_parity(
        spec,
        &logical_path,
        &asset_manager,
        &runtime,
    ) {
        Ok(report) => println!(
            "PASS spec={} projection={} entries={} canonical={}",
            report.spec_id,
            report.projection,
            report.canonical.entries.len(),
            serde_json::to_string(&report.canonical).expect("serialize canonical DTO")
        ),
        Err(errors) => {
            for error in errors {
                eprintln!("FAIL {error}");
            }
            process::exit(1);
        }
    }
}
