use crate::api::{MaterialDescriptor, MaterialDomain, MaterialFlags, ShadingModel};

use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::io::{push_f32, push_u16, push_u32, read_f32, read_u16, read_u32, read_u8};

/// Current binary format version.
pub const MATERIAL_BINARY_VERSION: u16 = 1;

/// Fixed header size in bytes.
pub const MATERIAL_BINARY_HEADER_SIZE: usize = 24;

/// Magic bytes (8).
pub const MATERIAL_BINARY_MAGIC: [u8; 8] = *b"NEMAT\0\0\0";

pub(crate) const MATERIAL_DESCRIPTOR_SIZE: usize = 68;

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
    let domain_u8 = read_u8(bytes, off)?;
    let shading_u8 = read_u8(bytes, off)?;
    let _ = read_u16(bytes, off)?;
    let flags = read_u32(bytes, off)?;

    let domain = match domain_u8 {
        0 => MaterialDomain::Surface,
        1 => MaterialDomain::PostProcess,
        2 => MaterialDomain::Ui,
        v => {
            return Err(MaterialBinaryError::InvalidEnumValue {
                field: "domain",
                value: v,
            })
        }
    };

    let shading_model = match shading_u8 {
        0 => ShadingModel::Unlit,
        1 => ShadingModel::PbrMetallicRoughness,
        v => {
            return Err(MaterialBinaryError::InvalidEnumValue {
                field: "shading_model",
                value: v,
            })
        }
    };

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