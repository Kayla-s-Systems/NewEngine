use std::{fs, io::Write, path::PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, ListFileEncodeRequest,
    SourceDictionaryManifestV1, LIST_FILE_CONTENT_KIND_YCD,
};

fn main() -> Result<(), String> {
    let root = PathBuf::from("C:\\Users\\Aiden\\Documents\\Repos\\NorthStar");
    let source_root = root.join("Shared\\Source\\animations\\characters\\abby");
    let content_root = root.join("Shared\\Content\\animations\\characters\\abby");

    let mut dictionaries = fs::read_dir(&source_root)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false))
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ycd"))
        })
        .collect::<Vec<_>>();
    dictionaries.sort_by_key(|entry| entry.file_name());

    let mut rebuilt_count = 0usize;
    for entry in dictionaries {
        let source_dir = entry.path();
        let manifest_path = source_dir.join("dictionary.source.json");
        let manifest_bytes = fs::read(&manifest_path).map_err(|error| {
            format!("read manifest '{}' failed: {error}", manifest_path.display())
        })?;
        let manifest: SourceDictionaryManifestV1 = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| {
                format!("parse manifest '{}' failed: {error}", manifest_path.display())
            })?;
        if manifest.importer.trim() != "nef8.primary_body.v1" {
            return Err(format!(
                "unexpected importer '{}' for '{}'",
                manifest.importer,
                manifest.logical_path
            ));
        }
        manifest.validate_for_runtime_path(&manifest.logical_path)?;

        let source_path = source_dir.join(&manifest.primary);
        let body = fs::read(&source_path).map_err(|error| {
            format!("read primary '{}' failed: {error}", source_path.display())
        })?;
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
        if manifest
            .options
            .get("content_kind")
            .and_then(|value| value.parse::<u32>().ok())
            .is_some_and(|kind| kind != LIST_FILE_CONTENT_KIND_YCD)
        {
            return Err(format!(
                "manifest '{}' is not YCD content kind",
                manifest.logical_path
            ));
        }

        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&body).map_err(|error| error.to_string())?;
        let stored = encoder.finish().map_err(|error| error.to_string())?;
        let rebuilt = encode_list_file(ListFileEncodeRequest {
            content_kind: LIST_FILE_CONTENT_KIND_YCD,
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
        let decoded = decode_list_file_envelope(
            &rebuilt,
            LIST_FILE_CONTENT_KIND_YCD,
            &manifest.logical_path,
        )?;
        if decoded.body != body {
            return Err(format!(
                "rebuilt body differs from authoritative source '{}'",
                manifest.logical_path
            ));
        }

        let target_path = content_root.join(entry.file_name());
        let tmp = target_path.with_extension("ycd.tmp-repack");
        fs::write(&tmp, &rebuilt).map_err(|error| error.to_string())?;
        if target_path.exists() {
            fs::remove_file(&target_path).map_err(|error| error.to_string())?;
        }
        fs::rename(&tmp, &target_path).map_err(|error| error.to_string())?;
        rebuilt_count += 1;
        println!(
            "repacked {} entries={} raw={} stored={} blake3={}",
            manifest.logical_path,
            entry_count,
            body.len(),
            stored.len(),
            blake3::hash(&body)
        );
    }

    println!("repacked dictionaries={rebuilt_count}");
    Ok(())
}
