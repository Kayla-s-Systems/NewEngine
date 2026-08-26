use std::{collections::BTreeMap, env, fs, io::Write, path::PathBuf};

use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{encode_list_file, ListFileEncodeRequest};
use newengine_model_import_northstar::{decode_vram_textures, ImportedTextureFormat, PakFile};
use newengine_texture_container::{
    encode_rgba8_mips_to_bcn, generate_rgba8_mips, pack_encoded, TextureEncodedBuildEntry,
    COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB, PIXEL_FORMAT_BC1_RGBA_SRGB, PIXEL_FORMAT_BC1_RGBA_UNORM,
    PIXEL_FORMAT_BC5_RG_UNORM,
};

#[derive(Clone, Debug)]
struct Selection {
    entry: String,
    package: String,
    source_contains: String,
    role: String,
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mut output = None::<PathBuf>;
    let mut manifest = None::<PathBuf>;
    let mut packages = Vec::<PathBuf>::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().map(PathBuf::from),
            "--manifest" => manifest = args.next().map(PathBuf::from),
            "--package" => {
                packages.push(PathBuf::from(args.next().ok_or("--package requires path")?))
            }
            "-h" | "--help" => {
                println!("usage: compile_level_texture_dictionary --manifest selections.tsv --package level.pak [--package ...] --output level.ytd");
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let output = output.ok_or("--output is required")?;
    let manifest = manifest.ok_or("--manifest is required")?;
    if packages.is_empty() {
        return Err("at least one --package is required".into());
    }

    let text = fs::read_to_string(&manifest)
        .map_err(|e| format!("read manifest '{}': {e}", manifest.display()))?;
    let mut selections = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line_index == 0 && line.starts_with("entry")
        {
            continue;
        }
        let cols = line.split('\t').collect::<Vec<_>>();
        if cols.len() < 4 {
            return Err(format!(
                "manifest line {} requires entry/package/source_contains/role",
                line_index + 1
            ));
        }
        selections.push(Selection {
            entry: cols[0].trim().to_owned(),
            package: cols[1].trim().to_ascii_lowercase(),
            source_contains: cols[2].trim().to_ascii_lowercase(),
            role: cols[3].trim().to_ascii_lowercase(),
        });
    }
    if selections.is_empty() {
        return Err("selection manifest is empty".into());
    }

    let mut corpus = BTreeMap::<String, (PakFile, PathBuf)>::new();
    for path in packages {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let pak = PakFile::parse(
            fs::read(&path).map_err(|e| format!("read package '{}': {e}", path.display()))?,
        )?;
        corpus.insert(name, (pak, path));
    }

    let mut entries = Vec::new();
    for selection in &selections {
        let (pak, package_path) = corpus.get(&selection.package).ok_or_else(|| {
            format!(
                "selection '{}' requests missing package '{}'",
                selection.entry, selection.package
            )
        })?;
        let textures = decode_vram_textures(pak)?;
        let candidates = textures
            .iter()
            .filter(|t| {
                t.logical_name()
                    .to_ascii_lowercase()
                    .contains(&selection.source_contains)
                    || t.source_path
                        .to_ascii_lowercase()
                        .contains(&selection.source_contains)
            })
            .collect::<Vec<_>>();
        let texture = match candidates.as_slice() {
            [only] => *only,
            [] => {
                return Err(format!(
                    "texture '{}' not found package='{}' selector='{}'",
                    selection.entry,
                    package_path.display(),
                    selection.source_contains
                ))
            }
            many => {
                let exact = many
                    .iter()
                    .copied()
                    .filter(|t| {
                        t.logical_name()
                            .eq_ignore_ascii_case(&selection.source_contains)
                    })
                    .collect::<Vec<_>>();
                match exact.as_slice() {
                    [only] => *only,
                    _ => {
                        return Err(format!(
                            "texture '{}' selector ambiguous package='{}' selector='{}' matches={}",
                            selection.entry,
                            package_path.display(),
                            selection.source_contains,
                            many.len()
                        ))
                    }
                }
            }
        };
        let linear_base = texture.base_linear_bytes(pak)?;
        let rgba = texture.base_rgba8(pak)?;
        let rgba_mips = generate_rgba8_mips(texture.width, texture.height, rgba)
            .map_err(|e| format!("mip generation '{}' failed: {e}", texture.source_path))?;
        let (format, color_space) = match selection.role.as_str() {
            "color" | "base_color" => match texture.format {
                ImportedTextureFormat::Bc1Srgb => (PIXEL_FORMAT_BC1_RGBA_SRGB, COLOR_SPACE_SRGB),
                ImportedTextureFormat::Bc1Unorm => (PIXEL_FORMAT_BC1_RGBA_UNORM, COLOR_SPACE_SRGB),
                other => {
                    return Err(format!(
                        "color '{}' requires BC1 source, got DXGI={} path='{}'",
                        selection.entry,
                        other.dxgi(),
                        texture.source_path
                    ))
                }
            },
            "normal" => match texture.format {
                ImportedTextureFormat::Bc5Unorm => (PIXEL_FORMAT_BC5_RG_UNORM, COLOR_SPACE_LINEAR),
                other => {
                    return Err(format!(
                        "normal '{}' requires BC5 source, got DXGI={} path='{}'",
                        selection.entry,
                        other.dxgi(),
                        texture.source_path
                    ))
                }
            },
            other => {
                return Err(format!(
                    "unsupported texture role '{other}' for '{}'",
                    selection.entry
                ))
            }
        };
        let mut encoded = encode_rgba8_mips_to_bcn(format, &rgba_mips)
            .map_err(|e| format!("BCN encode '{}' failed: {e}", texture.source_path))?;
        encoded[0].bytes = linear_base;
        println!(
            "TEXTURE entry='{}' role='{}' source='{}' dxgi={} {}x{} mips={} -> {}",
            selection.entry,
            selection.role,
            texture.logical_name(),
            texture.format.dxgi(),
            texture.width,
            texture.height,
            encoded.len(),
            format
        );
        entries.push(TextureEncodedBuildEntry {
            name: selection.entry.clone(),
            width: texture.width,
            height: texture.height,
            format: format.to_owned(),
            color_space: color_space.to_owned(),
            mips: encoded,
        });
    }

    let body = pack_encoded(entries).map_err(|e| format!("YTD body pack failed: {e}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&body)
        .map_err(|e| format!("deflate write failed: {e}"))?;
    let stored = encoder
        .finish()
        .map_err(|e| format!("deflate finish failed: {e}"))?;
    let ytd = encode_list_file(ListFileEncodeRequest {
        content_kind: newengine_asset_format_nef8::ytd::CONTENT_KIND,
        content_schema_version: newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_VERSION,
        entry_count: selections.len() as u32,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&output, ytd).map_err(|e| format!("write '{}': {e}", output.display()))?;
    println!(
        "LEVEL_TEXTURE_DICTIONARY_OK output='{}' entries={} body_bytes={}",
        output.display(),
        selections.len(),
        body.len()
    );
    Ok(())
}
