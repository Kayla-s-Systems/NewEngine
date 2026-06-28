use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::format::{
    decode_descriptor_from, encode_descriptor_into, MATERIAL_BINARY_VERSION,
    MATERIAL_DESCRIPTOR_SIZE,
};
use super::io::{push_u16, read_u16};
use super::types::MaterialBinaryAsset;
use crate::api::MaterialDescriptor;

/// Encodes a descriptor as a standalone compact payload.
///
/// This is not a `.nemat` file and intentionally has no top-level magic/header.
/// Public `.nemat` assets are NEF8/ListFile `content_kind=4` material libraries.
#[inline]
pub fn encode_descriptor(desc: &MaterialDescriptor) -> Vec<u8> {
    let mut out = Vec::with_capacity(MATERIAL_DESCRIPTOR_SIZE);
    encode_descriptor_into(&mut out, desc);
    out
}

/// Decodes a descriptor payload previously produced by `encode_descriptor`.
#[inline]
pub fn decode_descriptor(bytes: &[u8]) -> MaterialBinaryResult<MaterialDescriptor> {
    let mut off = 0usize;
    let desc = decode_descriptor_from(bytes, &mut off)?;
    Ok(desc)
}

/// Encodes a named material as a compact inner payload.
///
/// This preserves the legacy Rust API name for callers that need a named
/// descriptor blob, but it no longer creates or accepts a standalone `.nemat`
/// file format. The public `.nemat` resident format is NEF8/ListFile only.
#[inline]
pub fn encode_asset(asset: &MaterialBinaryAsset) -> MaterialBinaryResult<Vec<u8>> {
    let name_bytes = asset.name.as_bytes();
    let name_len = name_bytes.len().min(u16::MAX as usize) as u16;

    let mut out = Vec::with_capacity(8 + name_len as usize + MATERIAL_DESCRIPTOR_SIZE);
    push_u16(&mut out, MATERIAL_BINARY_VERSION);
    push_u16(&mut out, name_len);
    out.extend_from_slice(&name_bytes[..name_len as usize]);

    while out.len() % 4 != 0 {
        out.push(0);
    }

    encode_descriptor_into(&mut out, &asset.desc);
    Ok(out)
}

/// Decodes a compact named material inner payload.
///
/// This function does not parse a top-level `.nemat` file. `.nemat` files must
/// first be decoded through the NEF8/ListFile codec and selected as
/// `file.nemat@entry` by the material gateway.
#[inline]
pub fn decode_asset(bytes: &[u8]) -> MaterialBinaryResult<MaterialBinaryAsset> {
    let mut off = 0usize;
    let version = read_u16(bytes, &mut off)?;
    if version != MATERIAL_BINARY_VERSION {
        return Err(MaterialBinaryError::UnsupportedVersion { found: version });
    }
    let name_len = read_u16(bytes, &mut off)? as usize;
    if bytes.len() < off + name_len {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    let name_bytes = &bytes[off..off + name_len];
    off += name_len;
    let name = core::str::from_utf8(name_bytes)
        .map_err(|_| MaterialBinaryError::InvalidUtf8)?
        .to_string();

    while off % 4 != 0 {
        off += 1;
        if off > bytes.len() {
            return Err(MaterialBinaryError::UnexpectedEof);
        }
    }

    let desc = decode_descriptor_from(bytes, &mut off)?;
    Ok(MaterialBinaryAsset { name, desc })
}
