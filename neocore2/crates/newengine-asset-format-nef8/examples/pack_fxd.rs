use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let source = PathBuf::from(args.next().ok_or("missing source JSON path")?);
    let output = PathBuf::from(args.next().ok_or("missing output .fxd path")?);
    let logical_path = args
        .next()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| output.file_name().unwrap_or_default().to_string_lossy().into_owned());
    if args.next().is_some() {
        return Err("usage: pack_fxd <source.json> <output.fxd> [logical_path]".into());
    }
    let dictionary: newengine_vfx_api::FxdDictionaryV1 =
        serde_json::from_slice(&fs::read(&source)?)?;
    dictionary.validate()?;
    let bytes = newengine_asset_format_nef8::encode_fxd_nef8(&dictionary, &logical_path)?;
    let decoded = newengine_asset_format_nef8::decode_fxd_nef8(&bytes)?;
    if decoded != dictionary {
        return Err("FXD encode/decode verification mismatch".into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &bytes)?;
    println!(
        "packed source='{}' output='{}' logical_path='{}' effects={} textures={} bytes={}",
        source.display(), output.display(), logical_path, dictionary.effects.len(), dictionary.textures.len(), bytes.len()
    );
    Ok(())
}
