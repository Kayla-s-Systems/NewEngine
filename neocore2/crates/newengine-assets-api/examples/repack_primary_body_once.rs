use std::{env, fs, io::Write, path::PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, ListFileEncodeRequest,
    SourceDictionaryManifestV1,
};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let source_dir = PathBuf::from(args.next().ok_or("missing source directory")?);
    let target_path = PathBuf::from(args.next().ok_or("missing target path")?);
    if args.next().is_some() {
        return Err("expected exactly: <source-directory> <target-file>".to_owned());
    }

    let manifest_path = source_dir.join("dictionary.source.json");
    let manifest: SourceDictionaryManifestV1 = serde_json::from_slice(
        &fs::read(&manifest_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    if manifest.importer.trim() != "nef8.primary_body.v1" {
        return Err(format!("unsupported importer '{}'", manifest.importer));
    }
    manifest.validate_for_runtime_path(&manifest.logical_path)?;

    let body = fs::read(source_dir.join(&manifest.primary)).map_err(|error| error.to_string())?;
    let content_kind = manifest
        .options
        .get("content_kind")
        .ok_or("missing content_kind")?
        .parse::<u32>()
        .map_err(|error| error.to_string())?;
    let schema_version = manifest
        .options
        .get("content_schema_version")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(1);
    let entry_count = manifest
        .options
        .get("entry_count")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&body).map_err(|error| error.to_string())?;
    let stored = encoder.finish().map_err(|error| error.to_string())?;
    let rebuilt = encode_list_file(ListFileEncodeRequest {
        content_kind,
        content_schema_version: schema_version,
        entry_count,
        additional_flags: 0,
        min_size_class: 6,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: Some(*blake3::hash(&body).as_bytes()),
        stable_file_id: None,
        import_settings_hash: None,
    })?;

    let decoded = decode_list_file_envelope(&rebuilt, content_kind, &manifest.logical_path)?;
    if decoded.body != body {
        return Err("rebuilt body differs from authoritative source".to_owned());
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = target_path.with_extension("tmp-primary-body-repack");
    fs::write(&temp, &rebuilt).map_err(|error| error.to_string())?;
    if target_path.exists() {
        fs::remove_file(&target_path).map_err(|error| error.to_string())?;
    }
    fs::rename(&temp, &target_path).map_err(|error| error.to_string())?;

    println!(
        "repacked logical='{}' kind={} entries={} raw={} stored={} blake3={}",
        manifest.logical_path,
        content_kind,
        entry_count,
        body.len(),
        stored.len(),
        blake3::hash(&body)
    );
    Ok(())
}
