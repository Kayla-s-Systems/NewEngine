use std::{env, fs, path::PathBuf, process};

fn main() {
    let mut args = env::args().skip(1);
    let input = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let output = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let target_schema = args
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or_else(|| usage());
    let bytes =
        fs::read(&input).unwrap_or_else(|error| fail(format!("read {}: {error}", input.display())));
    let header = newengine_assets_api::parse_list_file_header(&bytes)
        .unwrap_or_else(|error| fail(format!("parse {}: {error}", input.display())));
    let metadata_start = usize::try_from(header.header_metadata_offset)
        .unwrap_or_else(|_| fail("metadata offset overflow".into()));
    let metadata_len = usize::try_from(header.header_metadata_len)
        .unwrap_or_else(|_| fail("metadata len overflow".into()));
    let body_start =
        usize::try_from(header.body_offset).unwrap_or_else(|_| fail("body offset overflow".into()));
    let body_len =
        usize::try_from(header.body_len).unwrap_or_else(|_| fail("body len overflow".into()));
    let metadata = bytes
        .get(metadata_start..metadata_start + metadata_len)
        .unwrap_or_else(|| fail("metadata range invalid".into()));
    let body = bytes
        .get(body_start..body_start + body_len)
        .unwrap_or_else(|| fail("body range invalid".into()));
    let rewritten =
        newengine_assets_api::encode_list_file(newengine_assets_api::ListFileEncodeRequest {
            content_kind: header.content_kind,
            content_schema_version: target_schema,
            entry_count: header.entry_count,
            additional_flags: header.flags,
            min_size_class: header.size_class,
            header_metadata: metadata,
            body_stored: body,
            body_uncompressed_len: header.body_uncompressed_len,
            body_raw_hash: header.has_body_raw_hash().then_some(header.body_raw_hash),
            stable_file_id: header.has_stable_file_id().then_some(header.stable_file_id),
            import_settings_hash: header
                .has_import_settings_hash()
                .then_some(header.import_settings_hash),
        })
        .unwrap_or_else(|error| fail(format!("encode {}: {error}", input.display())));
    let rewritten_header = newengine_assets_api::parse_list_file_header(&rewritten)
        .unwrap_or_else(|error| fail(format!("reparse {}: {error}", input.display())));
    let new_body_start = usize::try_from(rewritten_header.body_offset).unwrap();
    let new_body_len = usize::try_from(rewritten_header.body_len).unwrap();
    let new_meta_start = usize::try_from(rewritten_header.header_metadata_offset).unwrap();
    let new_meta_len = usize::try_from(rewritten_header.header_metadata_len).unwrap();
    if rewritten.get(new_body_start..new_body_start + new_body_len) != Some(body) {
        fail("stored body changed during schema rewrite".into());
    }
    if rewritten.get(new_meta_start..new_meta_start + new_meta_len) != Some(metadata) {
        fail("header metadata changed during schema rewrite".into());
    }
    if rewritten_header.content_kind != header.content_kind
        || rewritten_header.entry_count != header.entry_count
        || rewritten_header.flags != header.flags
        || rewritten_header.body_uncompressed_len != header.body_uncompressed_len
        || rewritten_header.body_raw_hash != header.body_raw_hash
        || rewritten_header.stable_file_id != header.stable_file_id
        || rewritten_header.import_settings_hash != header.import_settings_hash
    {
        fail("non-schema header contract changed during rewrite".into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| fail(format!("create {}: {error}", parent.display())));
    }
    fs::write(&output, &rewritten)
        .unwrap_or_else(|error| fail(format!("write {}: {error}", output.display())));
    println!(
        "PASS input={} output={} schema={}=>{} bytes={} body_preserved={} metadata_preserved={}",
        input.display(),
        output.display(),
        header.content_schema_version,
        target_schema,
        rewritten.len(),
        body.len(),
        metadata.len()
    );
}

fn usage() -> ! {
    eprintln!("usage: rewrite_list_file_schema <input> <output> <target_schema_u16>");
    process::exit(2)
}

fn fail(message: String) -> ! {
    eprintln!("FAIL {message}");
    process::exit(1)
}
