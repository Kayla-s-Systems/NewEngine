use std::{env, fs, path::PathBuf};

use newengine_gameplay_fps::{
    compile_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8, parse_authored_item_package_json,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let source = PathBuf::from(args.next().ok_or("missing source JSON path")?);
    let output = PathBuf::from(args.next().ok_or("missing output .neitems path")?);
    let logical_path = args
        .next()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "items/fps_items.neitems".to_owned());
    if args.next().is_some() {
        return Err("usage: pack_neitems <source.json> <output.neitems> [logical_path]".into());
    }

    let source_bytes = fs::read(&source)?;
    let authored = parse_authored_item_package_json(&source_bytes)?;
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
        "packed source='{}' output='{}' logical_path='{}' items={} loadouts={} bytes={}",
        source.display(),
        output.display(),
        logical_path,
        compiled.catalog.definitions().count(),
        compiled.loadouts.loadouts().count(),
        encoded.len(),
    );
    Ok(())
}
