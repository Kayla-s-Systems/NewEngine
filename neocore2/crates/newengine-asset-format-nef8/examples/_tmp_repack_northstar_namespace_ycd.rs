use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, ListFileEncodeRequest, LIST_FILE_CONTENT_KIND_YCD,
};
use std::{env, fs, io::Write, path::PathBuf};

fn replace_all(input: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() { return input; }
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        if input[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(input[i]);
            i += 1;
        }
    }
    out
}

fn migrate(mut bytes: Vec<u8>) -> Vec<u8> {
    const RULES: &[(&[u8], &[u8])] = &[
        (b"northstar.tlou2.pc://", b"northstar.pc://"),
        (b"TLOU2_", b"NORTHSTAR_"),
        (b"tlou2_", b"northstar_"),
        (b"Tlou2_", b"NorthStar_"),
        (b"TLOU2", b"NorthStar"),
        (b"tlou2", b"northstar"),
        (b"Tlou2", b"NorthStar"),
    ];
    for (from, to) in RULES { bytes = replace_all(bytes, from, to); }
    bytes
}

fn contains_legacy(bytes: &[u8]) -> bool {
    bytes.windows(5).any(|w| w.eq_ignore_ascii_case(b"tlou2"))
}

fn repack(path: PathBuf) -> Result<(), String> {
    let old = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded = decode_list_file_envelope(&old, LIST_FILE_CONTENT_KIND_YCD, &path.to_string_lossy())?;
    let old_body_len = decoded.body.len();
    let body = migrate(decoded.body.clone());
    let old_metadata = serde_json::to_vec(&decoded.metadata).map_err(|e| format!("serialize metadata: {e}"))?;
    let metadata = migrate(old_metadata.clone());
    if body == decoded.body && metadata == old_metadata {
        println!("SKIP '{}' no legacy namespace", path.display());
        return Ok(());
    }
    if contains_legacy(&body) || contains_legacy(&metadata) {
        return Err(format!("legacy namespace remains after migration: {}", path.display()));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&body).map_err(|e| format!("deflate {}: {e}", path.display()))?;
    let stored = encoder.finish().map_err(|e| format!("deflate finish {}: {e}", path.display()))?;
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
        import_settings_hash: header.has_import_settings_hash().then_some(header.import_settings_hash),
    })?;
    let verify = decode_list_file_envelope(&rebuilt, LIST_FILE_CONTENT_KIND_YCD, &path.to_string_lossy())?;
    let verify_metadata = serde_json::to_vec(&verify.metadata).map_err(|e| format!("verify metadata: {e}"))?;
    if contains_legacy(&verify.body) || contains_legacy(&verify_metadata) {
        return Err(format!("verification found legacy namespace: {}", path.display()));
    }
    if verify.header.entry_count != header.entry_count || verify.header.stable_file_id != header.stable_file_id {
        return Err(format!("header identity changed: {}", path.display()));
    }
    fs::write(&path, &rebuilt).map_err(|e| format!("write {}: {e}", path.display()))?;
    println!("REPACK PASS '{}' body:{}->{} bytes:{}->{} entries={} stable_id={}", path.display(), old_body_len, body.len(), old.len(), rebuilt.len(), header.entry_count, header.stable_file_id);
    Ok(())
}

fn main() -> Result<(), String> {
    let paths: Vec<PathBuf> = env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() { return Err("one or more YCD paths required".to_owned()); }
    for path in paths { repack(path)?; }
    Ok(())
}
