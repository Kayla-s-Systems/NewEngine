use std::{fs, path::PathBuf};

use newengine_texture_container::{
    encode_rgba8_mips_to_bcn, generate_rgba8_mips, pack_encoded, TextureEncodedBuildEntry,
    COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB, PIXEL_FORMAT_BC1_RGBA_SRGB, PIXEL_FORMAT_BC1_RGBA_UNORM,
    PIXEL_FORMAT_BC3_RGBA_SRGB, PIXEL_FORMAT_BC3_RGBA_UNORM,
};

use crate::{compile::encode_nef8, decode_vram_textures, ImportedTextureFormat, PakFile};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfxTextureSelection {
    pub source_contains: String,
    pub alpha_source_contains: Option<String>,
    pub entry_name: String,
}

impl VfxTextureSelection {
    pub fn new(source_contains: impl Into<String>, entry_name: impl Into<String>) -> Self {
        Self {
            source_contains: source_contains.into(),
            alpha_source_contains: None,
            entry_name: entry_name.into(),
        }
    }

    pub fn with_alpha_source(mut self, source_contains: impl Into<String>) -> Self {
        self.alpha_source_contains = Some(source_contains.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct VfxTextureDictionaryCompileRequest {
    pub package_path: PathBuf,
    pub output_path: PathBuf,
    pub selections: Vec<VfxTextureSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledVfxTextureEntry {
    pub entry_name: String,
    pub source_path: String,
    pub source_dxgi: u32,
    pub output_format: String,
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub base_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct VfxTextureDictionaryCompileReport {
    pub output_path: PathBuf,
    pub entry_count: usize,
    pub netd_bytes: usize,
    pub ytd_bytes: usize,
    pub entries: Vec<CompiledVfxTextureEntry>,
}

/// Compile selected NorthStar PC particle textures into a canonical resident NorthStar YTD.
///
/// Source `VRAM_DESC` payloads never escape the offline importer. Validated BC1 base blocks are
/// detiled and preserved byte-for-byte at mip0; lower BC1 mips are regenerated from the decoded
/// base. BC4 scalar masks are decoded offline and emitted as BC3 alpha so the runtime stays on the
/// existing GPU-native texture ABI instead of gaining a TLOU-specific/BC4 compatibility route.
pub fn compile_vfx_texture_dictionary(
    request: &VfxTextureDictionaryCompileRequest,
) -> Result<VfxTextureDictionaryCompileReport, String> {
    if request.selections.is_empty() {
        return Err("VFX texture compile requires at least one selection".to_owned());
    }
    let bytes = fs::read(&request.package_path).map_err(|error| {
        format!(
            "failed to read source VFX package '{}': {error}",
            request.package_path.display()
        )
    })?;
    let pak = PakFile::parse(bytes)?;
    let textures = decode_vram_textures(&pak)?;
    let mut encoded_entries = Vec::with_capacity(request.selections.len());
    let mut report_entries = Vec::with_capacity(request.selections.len());

    for selection in &request.selections {
        let needle = selection.source_contains.trim().to_ascii_lowercase();
        if needle.is_empty() || selection.entry_name.trim().is_empty() {
            return Err(
                "VFX texture selection requires non-empty source and entry name".to_owned(),
            );
        }
        let candidates = textures
            .iter()
            .filter(|texture| texture.source_path.to_ascii_lowercase().contains(&needle))
            .collect::<Vec<_>>();
        let texture = match candidates.as_slice() {
            [texture] => *texture,
            [] => {
                return Err(format!(
                    "VFX source texture not found package='{}' contains='{}'",
                    request.package_path.display(),
                    selection.source_contains
                ))
            }
            _ => {
                return Err(format!(
                "VFX source texture selector is ambiguous package='{}' contains='{}' matches={}",
                request.package_path.display(),
                selection.source_contains,
                candidates.len()
            ))
            }
        };

        let linear_base = texture.base_linear_bytes(&pak)?;
        let mut rgba = texture.base_rgba8(&pak)?;
        let alpha_texture = if let Some(alpha_selector) = selection.alpha_source_contains.as_deref()
        {
            let alpha_needle = alpha_selector.trim().to_ascii_lowercase();
            let alpha_candidates = textures
                .iter()
                .filter(|candidate| {
                    candidate
                        .source_path
                        .to_ascii_lowercase()
                        .contains(&alpha_needle)
                })
                .collect::<Vec<_>>();
            let alpha = match alpha_candidates.as_slice() {
                [alpha] => *alpha,
                [] => {
                    return Err(format!(
                        "VFX alpha source texture not found contains='{alpha_selector}'"
                    ))
                }
                _ => {
                    return Err(format!(
                        "VFX alpha selector is ambiguous contains='{alpha_selector}' matches={}",
                        alpha_candidates.len()
                    ))
                }
            };
            if alpha.width != texture.width || alpha.height != texture.height {
                return Err(format!(
                    "VFX color/alpha extent mismatch color='{}' {}x{} alpha='{}' {}x{}",
                    texture.source_path,
                    texture.width,
                    texture.height,
                    alpha.source_path,
                    alpha.width,
                    alpha.height
                ));
            }
            let alpha_rgba = alpha.base_rgba8(&pak)?;
            for (color, mask) in rgba.chunks_exact_mut(4).zip(alpha_rgba.chunks_exact(4)) {
                color[3] = mask[3];
            }
            Some(alpha)
        } else {
            None
        };
        let rgba_mips =
            generate_rgba8_mips(texture.width, texture.height, rgba).map_err(|error| {
                format!(
                    "VFX mip generation failed '{}': {error}",
                    texture.source_path
                )
            })?;
        let (output_format, color_space, preserve_source_base) = if alpha_texture.is_some() {
            if texture.format.is_srgb() {
                (PIXEL_FORMAT_BC3_RGBA_SRGB, COLOR_SPACE_SRGB, false)
            } else {
                (PIXEL_FORMAT_BC3_RGBA_UNORM, COLOR_SPACE_LINEAR, false)
            }
        } else {
            match texture.format {
                ImportedTextureFormat::Bc1Unorm => {
                    (PIXEL_FORMAT_BC1_RGBA_UNORM, COLOR_SPACE_LINEAR, true)
                }
                ImportedTextureFormat::Bc1Srgb => {
                    (PIXEL_FORMAT_BC1_RGBA_SRGB, COLOR_SPACE_SRGB, true)
                }
                ImportedTextureFormat::Bc4Unorm => {
                    (PIXEL_FORMAT_BC3_RGBA_UNORM, COLOR_SPACE_LINEAR, false)
                }
                _ => {
                    return Err(format!(
                    "VFX canonical compile does not accept unvalidated source DXGI={} path='{}'",
                    texture.format.dxgi(),
                    texture.source_path
                ))
                }
            }
        };
        let mut encoded_mips = encode_rgba8_mips_to_bcn(output_format, &rgba_mips)
            .map_err(|error| format!("VFX BCN encode failed '{}': {error}", texture.source_path))?;
        if preserve_source_base {
            encoded_mips[0].bytes = linear_base.clone();
        }
        let mip_count = encoded_mips.len() as u32;
        encoded_entries.push(TextureEncodedBuildEntry {
            name: selection.entry_name.clone(),
            width: texture.width,
            height: texture.height,
            format: output_format.to_owned(),
            color_space: color_space.to_owned(),
            mips: encoded_mips,
        });
        report_entries.push(CompiledVfxTextureEntry {
            entry_name: selection.entry_name.clone(),
            source_path: alpha_texture
                .map(|alpha| format!("{} + alpha:{}", texture.source_path, alpha.source_path))
                .unwrap_or_else(|| texture.source_path.clone()),
            source_dxgi: texture.format.dxgi(),
            output_format: output_format.to_owned(),
            width: texture.width,
            height: texture.height,
            mip_count,
            base_bytes: linear_base.len(),
        });
    }

    let netd =
        pack_encoded(encoded_entries).map_err(|error| format!("VFX NETD pack failed: {error}"))?;
    let parsed = newengine_texture_container::parse(&netd)
        .map_err(|error| format!("VFX NETD self-verify failed: {error}"))?;
    if parsed.entries().len() != report_entries.len() {
        return Err(format!(
            "VFX NETD entry count mismatch encoded={} parsed={}",
            report_entries.len(),
            parsed.entries().len()
        ));
    }
    let ytd = encode_nef8(
        &netd,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        newengine_asset_format_nef8::ytd::CONTENT_SCHEMA_VERSION,
        report_entries.len() as u32,
    )?;
    let decoded_ytd = newengine_assets_api::decode_list_file_envelope(
        &ytd,
        newengine_asset_format_nef8::ytd::CONTENT_KIND,
        request.output_path.to_string_lossy().as_ref(),
    )?;
    if decoded_ytd.header.entry_count as usize != report_entries.len() {
        return Err(format!(
            "VFX YTD envelope entry count mismatch encoded={} decoded={}",
            report_entries.len(),
            decoded_ytd.header.entry_count
        ));
    }
    let verified_dictionary = newengine_texture_container::parse(&decoded_ytd.body)
        .map_err(|error| format!("VFX YTD envelope/runtime self-verify failed: {error}"))?;
    if verified_dictionary.entries().len() != report_entries.len() {
        return Err(format!(
            "VFX YTD runtime entry count mismatch encoded={} decoded={}",
            report_entries.len(),
            verified_dictionary.entries().len()
        ));
    }
    if let Some(parent) = request.output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create VFX output directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    fs::write(&request.output_path, &ytd).map_err(|error| {
        format!(
            "failed to write VFX YTD '{}': {error}",
            request.output_path.display()
        )
    })?;
    Ok(VfxTextureDictionaryCompileReport {
        output_path: request.output_path.clone(),
        entry_count: report_entries.len(),
        netd_bytes: netd.len(),
        ytd_bytes: ytd.len(),
        entries: report_entries,
    })
}
