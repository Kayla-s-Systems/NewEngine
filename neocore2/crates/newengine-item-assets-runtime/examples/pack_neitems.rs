use std::{env, fs, path::PathBuf};

use newengine_item_assets_runtime::{
    compile_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8, hydrate_item_package_from_ytyp_source_roots,
    parse_authored_item_package_json,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let source = PathBuf::from(args.next().ok_or("missing source JSON path")?);
    let output = PathBuf::from(args.next().ok_or("missing output .neitems path")?);

    let mut logical_path = "items/fps_items.neitems".to_owned();
    let mut logical_path_set = false;
    let mut definition_roots = Vec::new();

    while let Some(arg) = args.next() {
        if arg == "--definition-root" {
            definition_roots.push(PathBuf::from(
                args.next().ok_or("--definition-root requires a path")?,
            ));
            continue;
        }
        let text = arg.to_string_lossy();
        if text.starts_with("--") {
            return Err(format!("unknown option '{text}'").into());
        }
        if logical_path_set {
            return Err(
                "usage: pack_neitems <source.json> <output.neitems> [logical_path] [--definition-root <path>]..."
                    .into(),
            );
        }
        logical_path = text.into_owned();
        logical_path_set = true;
    }

    let source_bytes = fs::read(&source)?;
    let mut authored = parse_authored_item_package_json(&source_bytes)?;
    let hydrated = if definition_roots.is_empty() {
        0
    } else {
        hydrate_item_package_from_ytyp_source_roots(&mut authored, &definition_roots)?
    };
    let compiled = compile_authored_item_package(&authored)?;
    let encoded = encode_authored_item_package_nef8(&authored, &logical_path)?;
    let decoded = decode_authored_item_package_nef8(&encoded)?;
    compile_authored_item_package(&decoded)?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = output.with_extension("neitems.tmp");
    fs::write(&temporary, &encoded)?;
    fs::rename(&temporary, &output).or_else(|_| {
        fs::copy(&temporary, &output)
            .map(|_| ())
            .and_then(|_| fs::remove_file(&temporary))
    })?;

    println!(
        "packed source='{}' output='{}' logical_path='{}' items={} loadouts={} hydrated_ytyp={} bytes={}",
        source.display(),
        output.display(),
        logical_path,
        compiled.catalog.definitions().count(),
        compiled.loadouts.loadouts().count(),
        hydrated,
        encoded.len(),
    );
    Ok(())
}
