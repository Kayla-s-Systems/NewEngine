use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <tool-runtime-spec-id>");
        process::exit(2);
    });
    let spec_id = args.next().unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <tool-runtime-spec-id>");
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
    match newengine_contract_conformance::validate_tool_runtime_artifact(spec, &bytes) {
        Ok(report) => println!(
            "PASS path={} spec={} wire={} content_kind={} schema={} contract={}",
            path.display(),
            spec.id,
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
