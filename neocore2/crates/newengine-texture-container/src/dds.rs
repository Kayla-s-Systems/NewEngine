use crate::mips::{generate_rgba8_mips, rgba8_len, TextureMipData};

#[derive(Debug, thiserror::Error)]
pub enum DdsExportError {
    #[error("dds: invalid RGBA8 extent {width}x{height}")]
    InvalidExtent { width: u32, height: u32 },
    #[error("dds: invalid RGBA8 payload bytes={bytes} expected={expected} extent={width}x{height}")]
    InvalidPayload { bytes: usize, expected: usize, width: u32, height: u32 },
    #[error("dds: mip generation failed: {0}")]
    MipGeneration(String),
}

/// Writes a simple uncompressed RGBA8 DDS file with a full generated mip chain.
///
/// This is an authoring/export helper for the texture tool. Runtime never reads DDS.
pub fn write_dds_rgba8(width: u32, height: u32, rgba: &[u8]) -> std::result::Result<Vec<u8>, DdsExportError> {
    if width == 0 || height == 0 {
        return Err(DdsExportError::InvalidExtent { width, height });
    }
    let expected = rgba8_len(width, height);
    if rgba.len() != expected {
        return Err(DdsExportError::InvalidPayload { bytes: rgba.len(), expected, width, height });
    }
    let mips = generate_rgba8_mips(width, height, rgba.to_vec())
        .map_err(|e| DdsExportError::MipGeneration(e.to_string()))?;
    write_dds_rgba8_mip_chain(width, height, &mips)
}

pub fn write_dds_rgba8_mip_chain(width: u32, height: u32, mips: &[TextureMipData]) -> std::result::Result<Vec<u8>, DdsExportError> {
    if width == 0 || height == 0 {
        return Err(DdsExportError::InvalidExtent { width, height });
    }
    if mips.is_empty() {
        return Err(DdsExportError::InvalidPayload { bytes: 0, expected: rgba8_len(width, height), width, height });
    }
    for mip in mips {
        let expected = rgba8_len(mip.width, mip.height);
        if mip.rgba.len() != expected {
            return Err(DdsExportError::InvalidPayload { bytes: mip.rgba.len(), expected, width: mip.width, height: mip.height });
        }
    }

    let payload_len = mips.iter().map(|m| m.rgba.len()).sum::<usize>();
    let mip_count = mips.len() as u32;
    let has_mips = mip_count > 1;

    let mut out = Vec::with_capacity(4 + 124 + payload_len);
    out.extend_from_slice(b"DDS ");
    write_u32(&mut out, 124); // dwSize
    let mut flags = 0x0000_100F; // CAPS | HEIGHT | WIDTH | PITCH | PIXELFORMAT
    if has_mips {
        flags |= 0x0002_0000; // MIPMAPCOUNT
    }
    write_u32(&mut out, flags);
    write_u32(&mut out, height);
    write_u32(&mut out, width);
    write_u32(&mut out, width.saturating_mul(4)); // pitch
    write_u32(&mut out, 0); // depth
    write_u32(&mut out, mip_count);
    for _ in 0..11 { write_u32(&mut out, 0); }

    write_u32(&mut out, 32); // DDPIXELFORMAT size
    write_u32(&mut out, 0x0000_0041); // DDPF_RGB | DDPF_ALPHAPIXELS
    write_u32(&mut out, 0); // fourCC
    write_u32(&mut out, 32); // RGB bit count
    write_u32(&mut out, 0x0000_00ff); // R
    write_u32(&mut out, 0x0000_ff00); // G
    write_u32(&mut out, 0x00ff_0000); // B
    write_u32(&mut out, 0xff00_0000); // A

    let mut caps = 0x0000_1000; // DDSCAPS_TEXTURE
    if has_mips {
        caps |= 0x0000_0008 | 0x0040_0000; // COMPLEX | MIPMAP
    }
    write_u32(&mut out, caps);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);

    for mip in mips {
        out.extend_from_slice(&mip.rgba);
    }
    Ok(out)
}

#[inline]
fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
