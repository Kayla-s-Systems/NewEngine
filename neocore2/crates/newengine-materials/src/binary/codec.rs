use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::format::{
    decode_descriptor_from,
    encode_descriptor_into,
    read_header,
    write_header_into,
    MaterialBinaryHeader,
    MATERIAL_BINARY_HEADER_SIZE,
    MATERIAL_BINARY_VERSION,
    MATERIAL_DESCRIPTOR_SIZE,
};
use super::io::{pad_to_4, push_u16, read_u16, round_up_4, skip_padding_4};
use super::types::MaterialBinaryAsset;
use crate::api::MaterialDescriptor;

/// Encodes a descriptor as a standalone deterministic payload.
///
/// The result does not include a file header or a material name. The payload size is always
/// [`super::MATERIAL_DESCRIPTOR_SIZE`] bytes.
#[inline]
pub fn encode_descriptor(desc: &MaterialDescriptor) -> Vec<u8> {
    let mut out = Vec::with_capacity(MATERIAL_DESCRIPTOR_SIZE);
    encode_descriptor_into(&mut out, desc);
    out
}

/// Decodes a standalone descriptor payload previously produced by [`encode_descriptor`].
#[inline]
pub fn decode_descriptor(bytes: &[u8]) -> MaterialBinaryResult<MaterialDescriptor> {
    if bytes.len() != MATERIAL_DESCRIPTOR_SIZE {
        return Err(MaterialBinaryError::InvalidDescriptorSize {
            found: bytes.len(),
            expected: MATERIAL_DESCRIPTOR_SIZE,
        });
    }

    let mut off = 0usize;
    decode_descriptor_from(bytes, &mut off)
}

/// Encodes a named material into a `.nemat` container.
///
/// The name is stored as UTF-8 with a `u16` byte length prefix and padded to a 4-byte boundary.
#[inline]
pub fn encode_asset(asset: &MaterialBinaryAsset) -> MaterialBinaryResult<Vec<u8>> {
    let name_bytes = asset.name.as_bytes();
    let name_len = checked_name_len(name_bytes.len())?;

    let name_field_size = round_up_4(4 + name_len as usize);
    let mut payload = Vec::with_capacity(name_field_size + MATERIAL_DESCRIPTOR_SIZE);

    push_u16(&mut payload, name_len);
    push_u16(&mut payload, 0);
    payload.extend_from_slice(name_bytes);
    pad_to_4(&mut payload);

    encode_descriptor_into(&mut payload, &asset.desc);

    let payload_size = checked_payload_size(payload.len())?;
    let mut out = Vec::with_capacity(MATERIAL_BINARY_HEADER_SIZE + payload.len());

    write_header_into(
        &mut out,
        MaterialBinaryHeader {
            version: MATERIAL_BINARY_VERSION,
            payload_size,
            flags: 0,
            reserved: 0,
        },
    );

    debug_assert_eq!(out.len(), MATERIAL_BINARY_HEADER_SIZE);

    out.extend_from_slice(&payload);
    Ok(out)
}

/// Decodes a `.nemat` container into a named material asset.
#[inline]
pub fn decode_asset(bytes: &[u8]) -> MaterialBinaryResult<MaterialBinaryAsset> {
    let header = read_header(bytes)?;
    let payload_size = header.payload_size as usize;

    if bytes.len() < MATERIAL_BINARY_HEADER_SIZE + payload_size {
        return Err(MaterialBinaryError::UnexpectedEof);
    }

    let payload = &bytes[MATERIAL_BINARY_HEADER_SIZE..MATERIAL_BINARY_HEADER_SIZE + payload_size];

    let mut poff = 0usize;
    let name_len = read_u16(payload, &mut poff)? as usize;
    let _reserved = read_u16(payload, &mut poff)?;

    if payload.len() < poff + name_len {
        return Err(MaterialBinaryError::UnexpectedEof);
    }

    let name_bytes = &payload[poff..poff + name_len];
    poff += name_len;

    let name = core::str::from_utf8(name_bytes)
        .map_err(|_| MaterialBinaryError::InvalidUtf8)?
        .to_string();

    skip_padding_4(payload, &mut poff)?;
    let desc = decode_descriptor_from(payload, &mut poff)?;

    Ok(MaterialBinaryAsset { name, desc })
}

#[inline]
fn checked_name_len(len: usize) -> MaterialBinaryResult<u16> {
    if len > u16::MAX as usize {
        return Err(MaterialBinaryError::NameTooLong {
            len,
            max: u16::MAX as usize,
        });
    }
    Ok(len as u16)
}

#[inline]
fn checked_payload_size(size: usize) -> MaterialBinaryResult<u32> {
    if size > u32::MAX as usize {
        return Err(MaterialBinaryError::PayloadTooLarge {
            size,
            max: u32::MAX as usize,
        });
    }
    Ok(size as u32)
}
