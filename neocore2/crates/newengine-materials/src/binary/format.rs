use crate::api::{MaterialDescriptor, MaterialDomain, MaterialFlags, ShadingModel};

use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::io::{push_f32, push_u16, push_u32, read_f32, read_u16, read_u32, read_u8};

/// Current `.nemat` binary format version.
pub const MATERIAL_BINARY_VERSION: u16 = 1;

/// Fixed binary container header size in bytes.
pub const MATERIAL_BINARY_HEADER_SIZE: usize = 24;

/// Magic bytes written at the start of every `.nemat` file.
pub const MATERIAL_BINARY_MAGIC: [u8; 8] = *b"NEMAT\0\0\0";

/// Encoded size of [`MaterialDescriptor`] in bytes.
pub const MATERIAL_DESCRIPTOR_SIZE: usize = 68;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MaterialBinaryHeader {
    pub version: u16,
    pub payload_size: u32,
    pub flags: u32,
    pub reserved: u32,
}

#[inline]
pub(crate) fn write_header_into(out: &mut Vec<u8>, header: MaterialBinaryHeader) {
    out.extend_from_slice(&MATERIAL_BINARY_MAGIC);
    push_u16(out, header.version);
    push_u16(out, MATERIAL_BINARY_HEADER_SIZE as u16);
    push_u32(out, header.payload_size);
    push_u32(out, header.flags);
    push_u32(out, header.reserved);
}

#[inline]
pub(crate) fn read_header(bytes: &[u8]) -> MaterialBinaryResult<MaterialBinaryHeader> {
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

    Ok(MaterialBinaryHeader {
        version,
        payload_size: read_u32(bytes, &mut off)?,
        flags: read_u32(bytes, &mut off)?,
        reserved: read_u32(bytes, &mut off)?,
    })
}

#[inline]
pub(crate) fn encode_descriptor_into(out: &mut Vec<u8>, desc: &MaterialDescriptor) {
    out.push(desc.domain as u8);
    out.push(desc.shading_model as u8);
    push_u16(out, 0);
    push_u32(out, desc.flags.0);

    for &v in &desc.base_color {
        push_f32(out, v);
    }

    for &v in &desc.emissive {
        push_f32(out, v);
    }

    push_f32(out, desc.emissive_strength);
    push_f32(out, desc.metallic);
    push_f32(out, desc.roughness);
    push_f32(out, desc.normal_scale);
    push_f32(out, desc.occlusion_strength);
    push_f32(out, desc.alpha_cutoff);

    push_u32(out, desc.reserved[0]);
    push_u32(out, desc.reserved[1]);
}

#[inline]
pub(crate) fn decode_descriptor_from(
    bytes: &[u8],
    off: &mut usize,
) -> MaterialBinaryResult<MaterialDescriptor> {
    let domain = decode_material_domain(read_u8(bytes, off)?)?;
    let shading_model = decode_shading_model(read_u8(bytes, off)?)?;
    let _ = read_u16(bytes, off)?;
    let flags = read_u32(bytes, off)?;

    let mut base_color = [0.0_f32; 4];
    for item in &mut base_color {
        *item = read_f32(bytes, off)?;
    }

    let mut emissive = [0.0_f32; 3];
    for item in &mut emissive {
        *item = read_f32(bytes, off)?;
    }

    let emissive_strength = read_f32(bytes, off)?;
    let metallic = read_f32(bytes, off)?;
    let roughness = read_f32(bytes, off)?;
    let normal_scale = read_f32(bytes, off)?;
    let occlusion_strength = read_f32(bytes, off)?;
    let alpha_cutoff = read_f32(bytes, off)?;

    let r0 = read_u32(bytes, off)?;
    let r1 = read_u32(bytes, off)?;

    Ok(MaterialDescriptor {
        domain,
        shading_model,
        base_color,
        emissive,
        emissive_strength,
        metallic,
        roughness,
        normal_scale,
        occlusion_strength,
        alpha_cutoff,
        flags: MaterialFlags(flags),
        reserved: [r0, r1],
    })
}

#[inline]
fn decode_material_domain(value: u8) -> MaterialBinaryResult<MaterialDomain> {
    match value {
        0 => Ok(MaterialDomain::Surface),
        1 => Ok(MaterialDomain::PostProcess),
        2 => Ok(MaterialDomain::Ui),
        v => Err(MaterialBinaryError::InvalidEnumValue {
            field: "domain",
            value: v,
        }),
    }
}

#[inline]
fn decode_shading_model(value: u8) -> MaterialBinaryResult<ShadingModel> {
    match value {
        0 => Ok(ShadingModel::Unlit),
        1 => Ok(ShadingModel::PbrMetallicRoughness),
        v => Err(MaterialBinaryError::InvalidEnumValue {
            field: "shading_model",
            value: v,
        }),
    }
}
