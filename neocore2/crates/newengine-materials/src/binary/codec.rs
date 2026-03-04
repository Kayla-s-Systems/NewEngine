use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::format::{
    decode_descriptor_from,
    encode_descriptor_into,
    MATERIAL_BINARY_HEADER_SIZE,
    MATERIAL_BINARY_MAGIC,
    MATERIAL_BINARY_VERSION,
    MATERIAL_DESCRIPTOR_SIZE,
};
use super::io::{push_u16, push_u32, read_u16, read_u32};
use super::types::MaterialBinaryAsset;
use crate::api::MaterialDescriptor;

/// Encodes a descriptor as a standalone payload (no header, no name).
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

/// Encodes a named material into the container (`.nemat`).
#[inline]
pub fn encode_asset(asset: &MaterialBinaryAsset) -> MaterialBinaryResult<Vec<u8>> {
    let name_bytes = asset.name.as_bytes();
    let name_len = name_bytes.len().min(u16::MAX as usize) as u16;

    let mut payload = Vec::with_capacity(8 + name_len as usize + 8 + MATERIAL_DESCRIPTOR_SIZE);

    push_u16(&mut payload, name_len);
    push_u16(&mut payload, 0);
    payload.extend_from_slice(&name_bytes[..name_len as usize]);

    while payload.len() % 4 != 0 {
        payload.push(0);
    }

    encode_descriptor_into(&mut payload, &asset.desc);

    let payload_size = payload.len() as u32;

    let mut out = Vec::with_capacity(MATERIAL_BINARY_HEADER_SIZE + payload.len());

    out.extend_from_slice(&MATERIAL_BINARY_MAGIC);
    push_u16(&mut out, MATERIAL_BINARY_VERSION);
    push_u16(&mut out, MATERIAL_BINARY_HEADER_SIZE as u16);
    push_u32(&mut out, payload_size);
    push_u32(&mut out, 0);
    push_u32(&mut out, 0);

    debug_assert_eq!(out.len(), MATERIAL_BINARY_HEADER_SIZE);

    out.extend_from_slice(&payload);
    Ok(out)
}

/// Decodes a binary material container (`.nemat`).
#[inline]
pub fn decode_asset(bytes: &[u8]) -> MaterialBinaryResult<MaterialBinaryAsset> {
    if bytes.len() < MATERIAL_BINARY_HEADER_SIZE {
        return Err(MaterialBinaryError::InvalidHeader);
    }

    if bytes[..8] != MATERIAL_BINARY_MAGIC {
        return Err(MaterialBinaryError::InvalidMagic);
    }

    let mut off = 8usize;
    let version = read_u16(bytes, &mut off)?;
    if version != MATERIAL_BINARY_VERSION {
        return Err(MaterialBinaryError::UnsupportedVersion { found: version });
    }

    let header_size = read_u16(bytes, &mut off)? as usize;
    if header_size != MATERIAL_BINARY_HEADER_SIZE {
        return Err(MaterialBinaryError::InvalidHeader);
    }

    let payload_size = read_u32(bytes, &mut off)? as usize;
    let _flags = read_u32(bytes, &mut off)?;
    let _reserved = read_u32(bytes, &mut off)?;

    if bytes.len() < header_size + payload_size {
        return Err(MaterialBinaryError::UnexpectedEof);
    }

    let payload = &bytes[header_size..header_size + payload_size];

    let mut poff = 0usize;
    let name_len = read_u16(payload, &mut poff)? as usize;
    let _ = read_u16(payload, &mut poff)?;

    if payload.len() < poff + name_len {
        return Err(MaterialBinaryError::UnexpectedEof);
    }

    let name_bytes = &payload[poff..poff + name_len];
    poff += name_len;

    let name = core::str::from_utf8(name_bytes)
        .map_err(|_| MaterialBinaryError::InvalidUtf8)?
        .to_string();

    while poff % 4 != 0 {
        poff += 1;
        if poff > payload.len() {
            return Err(MaterialBinaryError::UnexpectedEof);
        }
    }

    let desc = decode_descriptor_from(payload, &mut poff)?;

    Ok(MaterialBinaryAsset { name, desc })
}