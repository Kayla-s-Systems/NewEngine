use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{encode_list_file, ListFileEncodeRequest};
use newengine_texture_container::{
    generate_rgba8_mips, pack_encoded, TextureEncodedBuildEntry, TextureEncodedMipData,
};
use std::{env, fs, io::Write, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let input = PathBuf::from(args.next().ok_or("input ytd required")?);
    let logical_path = args.next().ok_or("logical path required")?;
    let target = args.next().ok_or("target entry required")?;
    let width: u32 = args
        .next()
        .ok_or("width required")?
        .parse()
        .map_err(|_| "bad width")?;
    let height: u32 = args
        .next()
        .ok_or("height required")?
        .parse()
        .map_err(|_| "bad height")?;
    let rgba_path = PathBuf::from(args.next().ok_or("rgba raw path required")?);
    let output = PathBuf::from(args.next().ok_or("output ytd required")?);

    let source = fs::read(&input).map_err(|e| format!("read input: {e}"))?;
    let decoded = newengine_assets_api::decode_list_file_envelope(
        &source,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        &logical_path,
    )?;
    let dictionary = newengine_texture_container::parse(&decoded.body)
        .map_err(|e| format!("parse YTD body: {e}"))?;
    let target_meta = dictionary
        .entries()
        .iter()
        .find(|meta| meta.name.eq_ignore_ascii_case(&target))
        .ok_or_else(|| format!("target entry '{target}' missing"))?;
    if target_meta.width != width || target_meta.height != height {
        return Err(format!(
            "target extent mismatch entry='{}' existing={}x{} replacement={}x{}",
            target, target_meta.width, target_meta.height, width, height
        ));
    }
    if !newengine_texture_container::is_rgba8_format(&target_meta.format) {
        return Err(format!(
            "target entry '{}' is not RGBA8: {}",
            target, target_meta.format
        ));
    }

    let replacement_rgba = fs::read(&rgba_path).map_err(|e| format!("read rgba: {e}"))?;
    let expected = width as usize * height as usize * 4;
    if replacement_rgba.len() != expected {
        return Err(format!(
            "replacement RGBA bytes={} expected={expected}",
            replacement_rgba.len()
        ));
    }
    let replacement_mips = generate_rgba8_mips(width, height, replacement_rgba)
        .map_err(|e| format!("generate replacement mips: {e}"))?;

    let mut entries = Vec::with_capacity(dictionary.entries().len());
    for meta in dictionary.entries() {
        let view = dictionary.entry(&meta.name).map_err(|e| e.to_string())?;
        let mips = if meta.name.eq_ignore_ascii_case(&target) {
            replacement_mips
                .iter()
                .map(|mip| TextureEncodedMipData {
                    level: mip.level,
                    width: mip.width,
                    height: mip.height,
                    bytes: mip.rgba.clone(),
                })
                .collect()
        } else {
            meta.mips
                .iter()
                .map(|mip| {
                    Ok(TextureEncodedMipData {
                        level: mip.level,
                        width: mip.width,
                        height: mip.height,
                        bytes: view
                            .mip_bytes(mip.level)
                            .ok_or_else(|| {
                                format!("missing mip {} for '{}'", mip.level, meta.name)
                            })?
                            .to_vec(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
        };
        entries.push(TextureEncodedBuildEntry {
            name: meta.name.clone(),
            width: meta.width,
            height: meta.height,
            format: meta.format.clone(),
            color_space: meta.color_space.clone(),
            mips,
        });
    }

    let body = pack_encoded(entries).map_err(|e| format!("pack YTD body: {e}"))?;
    let body_hash = *blake3::hash(&body).as_bytes();
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body).map_err(|e| e.to_string())?;
    let stored = encoder.finish().map_err(|e| e.to_string())?;
    let out = encode_list_file(ListFileEncodeRequest {
        content_kind: decoded.header.content_kind,
        content_schema_version: decoded.header.content_schema_version,
        entry_count: dictionary.entries().len() as u32,
        additional_flags: 0,
        min_size_class: decoded.header.size_class.max(6),
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: Some(body_hash),
        stable_file_id: decoded
            .header
            .has_stable_file_id()
            .then_some(decoded.header.stable_file_id),
        import_settings_hash: decoded
            .header
            .has_import_settings_hash()
            .then_some(decoded.header.import_settings_hash),
    })?;

    let verify = newengine_assets_api::decode_list_file_envelope(
        &out,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        &logical_path,
    )?;
    let verify_dict = newengine_texture_container::parse(&verify.body)
        .map_err(|e| format!("verify YTD body: {e}"))?;
    if verify_dict.entries().len() != dictionary.entries().len() {
        return Err("entry count changed during repack".to_owned());
    }
    let target_view = verify_dict.entry(&target).map_err(|e| e.to_string())?;
    if target_view.base_mip_rgba8() != Some(replacement_mips[0].rgba.as_slice()) {
        return Err("replacement base mip did not round-trip exactly".to_owned());
    }

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, out).map_err(|e| format!("write output: {e}"))?;
    println!(
        "YTD_ENTRY_REPACK_OK target='{}' entries={} body_bytes={} output='{}'",
        target,
        verify_dict.entries().len(),
        body.len(),
        output.display()
    );
    Ok(())
}
