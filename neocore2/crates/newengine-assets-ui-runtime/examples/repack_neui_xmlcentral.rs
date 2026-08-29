use std::{env, fs, io::Write, path::PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{encode_list_file, parse_list_file_header, ListFileEncodeRequest};

fn main() {
    if let Err(error) = run() {
        eprintln!("NEUI repack failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let source =
        PathBuf::from(args.next().ok_or(
            "usage: repack_neui_xmlcentral <source.neui.xml> <template.neui> [output.neui]",
        )?);
    let template =
        PathBuf::from(args.next().ok_or(
            "usage: repack_neui_xmlcentral <source.neui.xml> <template.neui> [output.neui]",
        )?);
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| template.clone());
    if args.next().is_some() {
        return Err("too many arguments".to_owned());
    }

    let xml =
        fs::read_to_string(&source).map_err(|e| format!("read '{}': {e}", source.display()))?;
    let template_bytes =
        fs::read(&template).map_err(|e| format!("read '{}': {e}", template.display()))?;
    let header = parse_list_file_header(&template_bytes)?;
    if header.content_kind != newengine_asset_format_nef8::neui::CONTENT_KIND {
        return Err(format!(
            "template '{}' is not NEUI content_kind={}",
            template.display(),
            header.content_kind
        ));
    }

    let metadata_start = header.header_metadata_offset as usize;
    let metadata_end = metadata_start
        .checked_add(header.header_metadata_len as usize)
        .ok_or("metadata range overflow")?;
    let metadata = template_bytes
        .get(metadata_start..metadata_end)
        .ok_or("template metadata range out of bounds")?;
    let logical_path = if metadata.is_empty() {
        output
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("ui.neui")
            .to_owned()
    } else {
        serde_json::from_slice::<newengine_assets_api::ListFileHeaderMetadata>(metadata)
            .map_err(|e| format!("template metadata decode failed: {e}"))?
            .logical_path
    };

    let document_ref = format!("{}@surface", logical_path.trim().replace('\\', "/"));
    let root =
        newengine_assets_ui_runtime::compile_xmlcentral_surface_root(&xml, &document_ref, None)?;
    if root.id.trim().is_empty() {
        return Err("semantic compile produced empty root id".to_owned());
    }

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .map_err(|e| format!("deflate write failed: {e}"))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("deflate finish failed: {e}"))?;
    let hash = *blake3::hash(xml.as_bytes()).as_bytes();
    let packed = encode_list_file(ListFileEncodeRequest {
        content_kind: header.content_kind,
        content_schema_version: header.content_schema_version,
        entry_count: header.entry_count,
        additional_flags: 0,
        min_size_class: header.size_class,
        header_metadata: metadata,
        body_stored: &compressed,
        body_uncompressed_len: xml.len() as u64,
        body_raw_hash: header.has_body_raw_hash().then_some(hash),
        stable_file_id: header.has_stable_file_id().then_some(header.stable_file_id),
        import_settings_hash: header
            .has_import_settings_hash()
            .then_some(header.import_settings_hash),
    })?;
    fs::write(&output, &packed).map_err(|e| format!("write '{}': {e}", output.display()))?;

    newengine_assets_ui_runtime::compile_neui_bytes_surface_root(&packed, &document_ref, None)
        .map_err(|e| format!("post-pack semantic verification failed: {e}"))?;
    println!(
        "NEUI repacked source='{}' output='{}' bytes={} root='{}'",
        source.display(),
        output.display(),
        packed.len(),
        root.id
    );
    Ok(())
}
