use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <ytyp|ydd|ytd|nemat|neui>");
        process::exit(2);
    });
    let domain = args.next().unwrap_or_else(|| {
        eprintln!("usage: check_list_file <path> <ytyp|ydd|ytd|nemat|neui>");
        process::exit(2);
    });
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        eprintln!("{}: read failed: {error}", path.display());
        process::exit(2);
    });
    let result = match domain.as_str() {
        "ytyp" => newengine_contract_conformance::validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::ytyp::CONTENT_KIND,
            newengine_asset_format_nef8::ytyp::CONTENT_SCHEMA_CONTRACT_SPEC,
        ),
        "ydd" => newengine_contract_conformance::validate_list_file_contract_with_read_compatibility(
            &bytes,
            newengine_asset_format_nef8::ydd::CONTENT_KIND,
            newengine_asset_format_nef8::YDD_BINARY_CONTRACT_SPEC,
            newengine_asset_format_nef8::ydd::READABLE_CONTENT_SCHEMA_VERSIONS,
        ),
        "ytd" => newengine_contract_conformance::validate_list_file_contract_with_read_compatibility(
            &bytes,
            newengine_asset_format_nef8::ytd::CONTENT_KIND,
            newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_CONTRACT_SPEC,
            newengine_asset_format_nef8::ytd::READABLE_CONTENT_SCHEMA_VERSIONS,
        ),
        "nemat" => newengine_contract_conformance::validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::nemat::CONTENT_KIND,
            newengine_asset_format_nef8::nemat::CONTENT_SCHEMA_CONTRACT_SPEC,
        ),
        "neui" => newengine_contract_conformance::validate_list_file_contract(
            &bytes,
            newengine_asset_format_nef8::neui::CONTENT_KIND,
            newengine_asset_format_nef8::neui::CONTENT_SCHEMA_CONTRACT_SPEC,
        ),
        other => {
            eprintln!("unsupported domain '{other}'");
            process::exit(2);
        }
    };
    match result {
        Ok(report) => println!(
            "PASS path={} wire={} content_kind={} schema={} contract={}",
            path.display(),
            report.wire_version,
            report.content_kind,
            report.content_schema_version,
            report.schema_contract_key
        ),
        Err(errors) => {
            for error in errors {
                eprintln!("FAIL path={} {error}", path.display());
            }
            process::exit(1);
        }
    }
}
