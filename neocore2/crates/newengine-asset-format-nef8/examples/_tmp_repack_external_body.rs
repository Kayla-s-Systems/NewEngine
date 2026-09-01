use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, parse_list_file_header, ListFileEncodeRequest,
};
use std::{env, fs, io::Write, path::Path};

fn replace_all(input: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return input;
    }
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

fn migrate_metadata(mut bytes: Vec<u8>) -> Vec<u8> {
    const RULES: &[(&[u8], &[u8])] = &[
        (b"naughtydog.tlou2.pc://", b"northstar.pc://"),
        (b"northstar.tlou2.pc://", b"northstar.pc://"),
        (b"northstar.northstar", b"northstar"),
        (b"tlou2_impact_decals", b"northstar_impact_decals"),
        (b"tlou2_weapon_vfx", b"northstar_weapon_vfx"),
        (b"Naughty Dog", b"NorthStar"),
        (b"NaughtyDog", b"NorthStar"),
        (b"naughtydog", b"northstar"),
        (b"TLOU2", b"NorthStar"),
        (b"tlou2", b"northstar"),
        (b"Tlou2", b"NorthStar"),
    ];
    for (from, to) in RULES {
        bytes = replace_all(bytes, from, to);
    }
    bytes
}

fn forbidden(bytes: &[u8]) -> bool {
    let lower: Vec<u8> = bytes.iter().map(|b| b.to_ascii_lowercase()).collect();
    [
        b"naughtydog".as_slice(),
        b"tlou2".as_slice(),
        b"northstar.northstar".as_slice(),
    ]
    .iter()
    .any(|needle| lower.windows(needle.len()).any(|w| w == *needle))
}

fn repack(asset: &Path, body_path: &Path) -> Result<(), String> {
    let old = fs::read(asset).map_err(|e| format!("read {}: {e}", asset.display()))?;
    let parsed = parse_list_file_header(&old)?;
    let decoded = decode_list_file_envelope(&old, parsed.content_kind, &asset.to_string_lossy())?;
    let body =
        fs::read(body_path).map_err(|e| format!("read body {}: {e}", body_path.display()))?;
    if forbidden(&body) {
        return Err(format!(
            "forbidden namespace remains in supplied body: {}",
            asset.display()
        ));
    }
    let old_metadata =
        serde_json::to_vec(&decoded.metadata).map_err(|e| format!("serialize metadata: {e}"))?;
    let metadata = migrate_metadata(old_metadata);
    if forbidden(&metadata) {
        return Err(format!(
            "forbidden namespace remains in metadata: {}",
            asset.display()
        ));
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder
        .write_all(&body)
        .map_err(|e| format!("deflate {}: {e}", asset.display()))?;
    let stored = encoder
        .finish()
        .map_err(|e| format!("deflate finish {}: {e}", asset.display()))?;
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
    let verify =
        decode_list_file_envelope(&rebuilt, header.content_kind, &asset.to_string_lossy())?;
    if verify.body != body {
        return Err(format!("body verification mismatch: {}", asset.display()));
    }
    let vm = serde_json::to_vec(&verify.metadata).map_err(|e| format!("verify metadata: {e}"))?;
    if forbidden(&vm) {
        return Err(format!(
            "forbidden namespace after verify: {}",
            asset.display()
        ));
    }
    if verify.header.entry_count != header.entry_count
        || verify.header.stable_file_id != header.stable_file_id
    {
        return Err(format!("header identity changed: {}", asset.display()));
    }
    fs::write(asset, &rebuilt).map_err(|e| format!("write {}: {e}", asset.display()))?;
    println!(
        "REPACK PASS '{}' kind={} entries={} bytes:{}->{} body:{}",
        asset.display(),
        header.content_kind,
        header.entry_count,
        old.len(),
        rebuilt.len(),
        body.len()
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let manifest = env::args_os().nth(1).ok_or("manifest path required")?;
    let text = fs::read_to_string(&manifest).map_err(|e| format!("read manifest: {e}"))?;
    let mut count = 0usize;
    for (line_no, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (asset, body) = line
            .split_once('\t')
            .ok_or_else(|| format!("manifest line {} missing tab", line_no + 1))?;
        repack(Path::new(asset), Path::new(body))?;
        count += 1;
    }
    println!("REPACK COMPLETE count={count}");
    Ok(())
}
