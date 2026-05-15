use crate::error::{Result, TextureContainerError};
use crate::header::HeaderV1;
use crate::manifest::{TextureDictionaryManifest, TextureEntryMeta, TextureMipMeta};
use crate::mips::{rgba8_len, TextureMipData};
use crate::names::{normalize_color_space, normalize_texture_name, stable_name_hash64};
use crate::{align_u64, align_vec, COLOR_SPACE_SRGB, HEADER_LEN, PIXEL_FORMAT_RGBA8_SRGB, PIXEL_FORMAT_RGBA8_UNORM, SCHEMA_V1, VERSION_V1};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct TextureBuildEntry {
    pub name: String,
    pub source_path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub color_space: String,
    pub mips: Vec<TextureMipData>,
}

pub fn pack(entries: Vec<TextureBuildEntry>) -> Result<Vec<u8>> {
    if entries.is_empty() {
        return Err(TextureContainerError::EmptyDictionary);
    }

    let mut seen = BTreeSet::new();
    let mut metas = Vec::with_capacity(entries.len());
    let mut data = Vec::<u8>::new();

    for entry in entries {
        let name = normalize_texture_name(&entry.name);
        if !seen.insert(name.clone()) {
            return Err(TextureContainerError::DuplicateEntry(name));
        }
        if entry.width == 0 || entry.height == 0 {
            return Err(TextureContainerError::InvalidExtent { name, width: entry.width, height: entry.height });
        }
        if entry.mips.is_empty() || entry.mips[0].level != 0 || entry.mips[0].width != entry.width || entry.mips[0].height != entry.height {
            return Err(TextureContainerError::InvalidMipChain(name));
        }

        align_vec(&mut data, 16);
        let entry_offset = data.len() as u64;
        let mut mip_metas = Vec::with_capacity(entry.mips.len());
        let mut expected_w = entry.width;
        let mut expected_h = entry.height;

        for (i, mip) in entry.mips.into_iter().enumerate() {
            if mip.level != i as u32 || mip.width != expected_w || mip.height != expected_h {
                return Err(TextureContainerError::InvalidMipChain(name.clone()));
            }
            let expected = rgba8_len(mip.width, mip.height);
            if mip.rgba.len() != expected {
                return Err(TextureContainerError::PayloadSizeMismatch { name: name.clone(), mip: mip.level, bytes: mip.rgba.len(), expected });
            }
            align_vec(&mut data, 16);
            let offset = data.len() as u64;
            let len = mip.rgba.len() as u64;
            data.extend_from_slice(&mip.rgba);
            mip_metas.push(TextureMipMeta { level: mip.level, width: mip.width, height: mip.height, byte_offset: offset, byte_len: len });
            expected_w = (expected_w / 2).max(1);
            expected_h = (expected_h / 2).max(1);
        }

        let entry_end = data.len() as u64;
        let color_space = normalize_color_space(&entry.color_space);
        let format = if color_space == COLOR_SPACE_SRGB { PIXEL_FORMAT_RGBA8_SRGB } else { PIXEL_FORMAT_RGBA8_UNORM };
        metas.push(TextureEntryMeta {
            name: name.clone(),
            source_path: entry.source_path,
            name_hash: stable_name_hash64(&name),
            width: entry.width,
            height: entry.height,
            format: format.to_owned(),
            color_space,
            byte_offset: entry_offset,
            byte_len: entry_end.saturating_sub(entry_offset),
            mip_count: mip_metas.len() as u32,
            mips: mip_metas,
        });
    }

    let manifest = TextureDictionaryManifest {
        schema: SCHEMA_V1.to_owned(),
        version: VERSION_V1,
        default_format: PIXEL_FORMAT_RGBA8_UNORM.to_owned(),
        entries: metas,
    };
    let dir = serde_json::to_vec_pretty(&manifest).expect("manifest serialization is infallible for owned data");

    let directory_offset = HEADER_LEN as u64;
    let directory_len = dir.len() as u64;
    let data_offset = align_u64(directory_offset + directory_len, 16);
    let mut out = vec![0u8; data_offset as usize];
    out[HEADER_LEN..HEADER_LEN + dir.len()].copy_from_slice(&dir);
    out.extend_from_slice(&data);

    let header = HeaderV1 {
        version: VERSION_V1,
        flags: 0,
        entry_count: manifest.entries.len() as u32,
        directory_offset,
        directory_len,
        data_offset,
        data_len: data.len() as u64,
    };
    header.write(&mut out[..HEADER_LEN]);
    Ok(out)
}
