use std::{env, fs, io::Write, path::PathBuf};
use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, ListFileEncodeRequest,
    LIST_FILE_CONTENT_KIND_YTYP,
};

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let existing = PathBuf::from(args.next().ok_or("existing compiled ytyp required")?);
    let authored = PathBuf::from(args.next().ok_or("authored XML required")?);
    let output = PathBuf::from(args.next().unwrap_or_else(|| existing.clone().into_os_string()));
    let old_bytes = fs::read(&existing).map_err(|e| format!("read {}: {e}", existing.display()))?;
    let decoded = decode_list_file_envelope(&old_bytes, LIST_FILE_CONTENT_KIND_YTYP, &existing.to_string_lossy())?;
    let body = fs::read(&authored).map_err(|e| format!("read {}: {e}", authored.display()))?;
    std::str::from_utf8(&body).map_err(|e| format!("authored YTYP is not UTF-8: {e}"))?;
    let metadata = serde_json::to_vec(&decoded.metadata).map_err(|e| format!("serialize metadata: {e}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&body).map_err(|e| format!("deflate write: {e}"))?;
    let stored = encoder.finish().map_err(|e| format!("deflate finish: {e}"))?;
    let header = decoded.header;
    let rebuilt = encode_list_file(ListFileEncodeRequest {
        content_kind: header.content_kind,
        content_schema_version: header.content_schema_version,
        entry_count: header.entry_count,
        additional_flags: header.flags,
        min_size_class: header.size_class,
        header_metadata: &metadata,
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: Some(*blake3::hash(&body).as_bytes()),
        stable_file_id: header.has_stable_file_id().then_some(header.stable_file_id),
        import_settings_hash: header
            .has_import_settings_hash()
            .then_some(header.import_settings_hash),
    })?;
    fs::write(&output, &rebuilt).map_err(|e| format!("write {}: {e}", output.display()))?;
    let verify = decode_list_file_envelope(&rebuilt, LIST_FILE_CONTENT_KIND_YTYP, &output.to_string_lossy())?;
    println!(
        "REPACK PASS output='{}' body={} entries={} metadata_entries={} stable_file_id={} bytes={}",
        output.display(), verify.body.len(), verify.header.entry_count, verify.metadata.entries.len(),
        verify.header.stable_file_id, rebuilt.len()
    );
    Ok(())
}
