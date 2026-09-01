use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <tool-runtime-spec-id> <descriptor.json>");
        process::exit(2);
    });
    let spec_id = args.next().unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <tool-runtime-spec-id> <descriptor.json>");
        process::exit(2);
    });
    let descriptor_path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <tool-runtime-spec-id> <descriptor.json>");
        process::exit(2);
    });
    let Some(spec) = newengine_contract_conformance::tool_runtime_conformance_spec(&spec_id) else {
        eprintln!("unsupported tool/runtime spec '{spec_id}'");
        process::exit(2);
    };
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("{}: read failed: {error}", path.display());
        process::exit(2);
    });
    let descriptor_bytes = fs::read(&descriptor_path).unwrap_or_else(|error| {
        eprintln!("{}: descriptor read failed: {error}", descriptor_path.display());
        process::exit(2);
    });
    let mut descriptor: newengine_assets_api::AssetFileTypeDescriptor =
        serde_json::from_slice(&descriptor_bytes).unwrap_or_else(|error| {
            eprintln!("{}: descriptor JSON invalid: {error}", descriptor_path.display());
            process::exit(2);
        });
    descriptor.normalize_layer_contract();
    if let Err(error) = descriptor.validate_generic_rules() {
        eprintln!("{}: descriptor invalid: {error}", descriptor_path.display());
        process::exit(2);
    }
    match newengine_contract_conformance::validate_tool_runtime_artifact(spec, &descriptor, &bytes) {
        Ok(report) => println!(
            "PASS path={} spec={} module={} wire={} content_kind={} schema={} contract={}",
            path.display(),
            spec.id,
            report.format_module_id,
            report.wire_version,
            report.content_kind,
            report.content_schema_version,
            report.schema_contract_key
        ),
        Err(errors) => {
            for error in errors {
                eprintln!("FAIL path={} spec={} {error}", path.display(), spec.id);
            }
            process::exit(1);
        }
    }
}
