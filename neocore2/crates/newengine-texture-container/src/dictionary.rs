use crate::error::{Result, TextureContainerError};
use crate::header::HeaderV1;
use crate::manifest::{TextureDictionaryManifest, TextureEntryMeta};
use crate::mips::rgba8_len;
use crate::{slice_checked, slice_checked_len, PIXEL_FORMAT_RGBA8_SRGB, PIXEL_FORMAT_RGBA8_UNORM, SCHEMA_V1};
use std::borrow::Cow;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy)]
pub struct TextureEntryView<'a> {
    pub meta: &'a TextureEntryMeta,
    data_region: &'a [u8],
}

impl<'a> TextureEntryView<'a> {
    #[inline]
    pub fn mip_bytes(&self, level: u32) -> Option<&'a [u8]> {
        let mip = self.meta.mips.iter().find(|m| m.level == level)?;
        slice_checked(self.data_region, mip.byte_offset, mip.byte_len).ok()
    }

    #[inline]
    pub fn base_mip_rgba8(&self) -> Option<&'a [u8]> {
        self.mip_bytes(0)
    }
}

#[derive(Debug, Clone)]
pub struct TextureDictionary<'a> {
    header: HeaderV1,
    manifest: TextureDictionaryManifest,
    data_region: Cow<'a, [u8]>,
}

impl<'a> TextureDictionary<'a> {
    #[inline]
    pub fn header(&self) -> HeaderV1 { self.header }
    #[inline]
    pub fn manifest(&self) -> &TextureDictionaryManifest { &self.manifest }
    #[inline]
    pub fn entries(&self) -> &[TextureEntryMeta] { &self.manifest.entries }

    pub fn first_entry(&self) -> Result<TextureEntryView<'_>> {
        let meta = self.manifest.entries.first().ok_or(TextureContainerError::EmptyDictionary)?;
        Ok(TextureEntryView { meta, data_region: self.data_region.as_ref() })
    }

    pub fn entry(&self, name: &str) -> Result<TextureEntryView<'_>> {
        let meta = self
            .manifest
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| TextureContainerError::MissingEntry(name.to_owned()))?;
        Ok(TextureEntryView { meta, data_region: self.data_region.as_ref() })
    }

    pub fn entry_by_hash(&self, hash: u64) -> Result<TextureEntryView<'_>> {
        let meta = self
            .manifest
            .entries
            .iter()
            .find(|e| e.name_hash == hash)
            .ok_or_else(|| TextureContainerError::MissingEntry(format!("hash:{hash}")))?;
        Ok(TextureEntryView { meta, data_region: self.data_region.as_ref() })
    }
}

pub fn parse(bytes: &[u8]) -> Result<TextureDictionary<'_>> {
    let header = HeaderV1::parse(bytes)?;
    let dir = slice_checked(bytes, header.directory_offset, header.directory_len).map_err(|_| TextureContainerError::InvalidRange {
        what: "directory",
        offset: header.directory_offset,
        len: header.directory_len,
        total: bytes.len(),
    })?;
    let manifest: TextureDictionaryManifest = serde_json::from_slice(dir)?;
    validate_manifest(header, &manifest, bytes.len())?;
    let data = slice_checked(bytes, header.data_offset, header.data_len).map_err(|_| TextureContainerError::InvalidRange {
        what: "data",
        offset: header.data_offset,
        len: header.data_len,
        total: bytes.len(),
    })?;

    Ok(TextureDictionary { header, manifest, data_region: Cow::Borrowed(data) })
}

fn validate_manifest(header: HeaderV1, manifest: &TextureDictionaryManifest, total_len: usize) -> Result<()> {
    if manifest.schema != SCHEMA_V1 {
        return Err(TextureContainerError::BadSchema(manifest.schema.clone()));
    }
    if header.entry_count as usize != manifest.entries.len() {
        return Err(TextureContainerError::EntryCountMismatch { header: header.entry_count, directory: manifest.entries.len() });
    }
    let mut seen = BTreeSet::new();
    for entry in &manifest.entries {
        if !seen.insert(entry.name.to_ascii_lowercase()) {
            return Err(TextureContainerError::DuplicateEntry(entry.name.clone()));
        }
        if entry.width == 0 || entry.height == 0 {
            return Err(TextureContainerError::InvalidExtent { name: entry.name.clone(), width: entry.width, height: entry.height });
        }
        if entry.format != PIXEL_FORMAT_RGBA8_UNORM && entry.format != PIXEL_FORMAT_RGBA8_SRGB {
            return Err(TextureContainerError::InvalidFormat { name: entry.name.clone(), format: entry.format.clone() });
        }
        if entry.mip_count as usize != entry.mips.len() || entry.mips.is_empty() {
            return Err(TextureContainerError::InvalidMipChain(entry.name.clone()));
        }
        let _ = slice_checked_len(header.data_len as usize, entry.byte_offset, entry.byte_len).map_err(|_| TextureContainerError::InvalidRange {
            what: "entry",
            offset: header.data_offset.saturating_add(entry.byte_offset),
            len: entry.byte_len,
            total: total_len,
        })?;
        for mip in &entry.mips {
            let mip_bytes = slice_checked_len(header.data_len as usize, mip.byte_offset, mip.byte_len).map_err(|_| TextureContainerError::InvalidRange {
                what: "mip",
                offset: header.data_offset.saturating_add(mip.byte_offset),
                len: mip.byte_len,
                total: total_len,
            })?;
            let expected = rgba8_len(mip.width, mip.height);
            if mip_bytes != expected {
                return Err(TextureContainerError::PayloadSizeMismatch { name: entry.name.clone(), mip: mip.level, bytes: mip_bytes, expected });
            }
        }
    }
    Ok(())
}
