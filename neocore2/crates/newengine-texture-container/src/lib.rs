#![forbid(unsafe_op_in_unsafe_fn)]

//! NewEngine Texture Dictionary container (`.neytd`).
//!
//! This crate owns the stable, engine-native texture dictionary format used by
//! tools, AssetManager importers and runtime upload paths. Runtime systems must
//! not parse source image containers directly.

pub mod builder;
pub mod dds;
pub mod dictionary;
pub mod error;
pub mod header;
pub mod manifest;
pub mod mips;
pub mod names;
pub mod selector;

pub use builder::{pack, TextureBuildEntry};
pub use dds::{write_dds_rgba8, write_dds_rgba8_mip_chain, DdsExportError};
pub use dictionary::{parse, TextureDictionary, TextureEntryView};
pub use error::{Result, TextureContainerError};
pub use header::HeaderV1;
pub use manifest::{TextureDictionaryManifest, TextureEntryMeta, TextureMipMeta};
pub use mips::{generate_rgba8_mips, rgba8_len, TextureMipData};
pub use names::{infer_color_space_from_name, normalize_color_space, normalize_texture_name, stable_name_hash64};
pub use selector::{TextureDictionarySelector, TextureSelectorError};

pub const MAGIC: [u8; 4] = *b"NETD";
pub const VERSION_V1: u16 = 1;
pub const HEADER_LEN: usize = 64;
pub const EXTENSION: &str = "neytd";
pub const SCHEMA_V1: &str = "newengine.texture_dictionary.v1";
pub const PIXEL_FORMAT_RGBA8_UNORM: &str = "RGBA8_UNORM";
pub const PIXEL_FORMAT_RGBA8_SRGB: &str = "RGBA8_SRGB";
pub const COLOR_SPACE_LINEAR: &str = "linear";
pub const COLOR_SPACE_SRGB: &str = "srgb";

#[inline]
pub(crate) fn align_u64(v: u64, alignment: u64) -> u64 {
    ((v + alignment - 1) / alignment) * alignment
}

#[inline]
pub(crate) fn align_vec(v: &mut Vec<u8>, alignment: usize) {
    let aligned = ((v.len() + alignment - 1) / alignment) * alignment;
    v.resize(aligned, 0);
}

#[inline]
pub(crate) fn slice_checked(bytes: &[u8], offset: u64, len: u64) -> std::result::Result<&[u8], ()> {
    let start = offset as usize;
    let len_usize = len as usize;
    let end = start.checked_add(len_usize).ok_or(())?;
    if start > bytes.len() || end > bytes.len() {
        return Err(());
    }
    Ok(&bytes[start..end])
}

#[inline]
pub(crate) fn slice_checked_len(total_len: usize, offset: u64, len: u64) -> std::result::Result<usize, ()> {
    let start = offset as usize;
    let len_usize = len as usize;
    let end = start.checked_add(len_usize).ok_or(())?;
    if start > total_len || end > total_len {
        return Err(());
    }
    Ok(len_usize)
}
