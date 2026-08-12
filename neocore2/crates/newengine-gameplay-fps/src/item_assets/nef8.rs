use super::*;

pub fn encode_authored_item_package_nef8(
    package: &AuthoredItemPackage,
    _logical_path: &str,
) -> Result<Vec<u8>, String> {
    validate_package_header(package)?;
    let body = serde_json::to_vec(package)
        .map_err(|error| format!("item package JSON encode failed: {error}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&body)
        .map_err(|error| format!("NEITEMS deflate write failed: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("NEITEMS deflate finish failed: {error}"))?;

    let body_hash =
        (body.len() >= LIST_FILE_FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(&body).as_bytes());
    let entry_count = package
        .items
        .len()
        .saturating_add(package.loadouts.len())
        .min(u32::MAX as usize) as u32;
    encode_list_file(ListFileEncodeRequest {
        content_kind: LIST_FILE_CONTENT_KIND_NEITEMS,
        content_schema_version: AUTHORED_ITEM_PACKAGE_VERSION as u16,
        entry_count,
        additional_flags: 0,
        min_size_class: 4,
        header_metadata: &[],
        body_stored: &compressed,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

pub fn decode_authored_item_package_nef8(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    let header = parse_list_file_header(bytes)?;
    if !header.content_kind_matches(LIST_FILE_CONTENT_KIND_NEITEMS) {
        return Err(format!(
            "NEITEMS content kind mismatch: got={} expected={}",
            header.content_kind, LIST_FILE_CONTENT_KIND_NEITEMS
        ));
    }
    let start = usize::try_from(header.body_offset)
        .map_err(|_| "NEITEMS body offset does not fit usize".to_owned())?;
    let length = usize::try_from(header.body_len)
        .map_err(|_| "NEITEMS body length does not fit usize".to_owned())?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| "NEITEMS body range overflow".to_owned())?;
    let compressed = bytes.get(start..end).ok_or_else(|| {
        format!(
            "NEITEMS body range outside file: offset={start} len={length} file={}",
            bytes.len()
        )
    })?;
    let mut decoder = DeflateDecoder::new(compressed);
    let mut body = Vec::with_capacity(header.body_uncompressed_len as usize);
    decoder
        .read_to_end(&mut body)
        .map_err(|error| format!("NEITEMS deflate decode failed: {error}"))?;
    if header.body_uncompressed_len != 0 && body.len() != header.body_uncompressed_len as usize {
        return Err(format!(
            "NEITEMS body length mismatch: got={} expected={}",
            body.len(),
            header.body_uncompressed_len
        ));
    }
    if header.has_body_raw_hash() && header.body_raw_hash != *blake3::hash(&body).as_bytes() {
        return Err("NEITEMS body BLAKE3 hash mismatch".to_owned());
    }
    parse_authored_item_package_json(&body)
}

pub fn decode_authored_item_package(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    if bytes.starts_with(&LIST_FILE_MAGIC_NEF8) {
        decode_authored_item_package_nef8(bytes)
    } else {
        parse_authored_item_package_json(bytes)
    }
}
