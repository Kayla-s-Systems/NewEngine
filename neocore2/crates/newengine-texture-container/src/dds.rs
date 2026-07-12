use crate::format::{parse_pixel_format, texture_payload_len, TexturePixelFormat};
use crate::mips::{generate_rgba8_mips, rgba8_len, TextureEncodedMipData, TextureMipData};
use crate::{
    COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB, PIXEL_FORMAT_BC1_RGBA_UNORM, PIXEL_FORMAT_BC2_RGBA_UNORM,
    PIXEL_FORMAT_BC3_RGBA_UNORM, PIXEL_FORMAT_BC5_RG_UNORM, PIXEL_FORMAT_RGBA8_SRGB,
    PIXEL_FORMAT_RGBA8_UNORM,
};

const DDPF_FOURCC: u32 = 0x0000_0004;
const DDPF_RGB: u32 = 0x0000_0040;
const DDPF_LUMINANCE: u32 = 0x0002_0000;
const DDSD_PITCH: u32 = 0x0000_0008;

#[derive(Debug, Clone)]
pub struct DdsRuntimeTexture {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub color_space: String,
    pub mips: Vec<TextureEncodedMipData>,
}

#[derive(Debug, thiserror::Error)]
pub enum DdsImportError {
    #[error("dds: invalid header: {0}")]
    InvalidHeader(String),
    #[error("dds: invalid extent {width}x{height}")]
    InvalidExtent { width: u32, height: u32 },
    #[error("dds: unsupported pixel format: {0}")]
    UnsupportedFormat(String),
    #[error("dds: payload layout does not match header: {0}")]
    InvalidPayload(String),
    #[error(
        "dds: mip payload truncated level={level} offset={offset} need={needed} available={available}"
    )]
    TruncatedMip {
        level: u32,
        offset: usize,
        needed: usize,
        available: usize,
    },
    #[error("dds: arithmetic overflow while computing {0}")]
    Overflow(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DdsSourceLayout {
    Native,
    Rgba8,
    Bgra8,
    Rgb24,
    Bgr24,
    L8,
}

impl DdsSourceLayout {
    #[inline]
    const fn is_native(self) -> bool {
        matches!(self, Self::Native)
    }

    fn packed_row_len(self, width: u32) -> Result<usize, DdsImportError> {
        let width = usize::try_from(width).map_err(|_| DdsImportError::Overflow("row width"))?;
        match self {
            Self::Native => Err(DdsImportError::InvalidPayload(
                "native block payload does not have a legacy source row layout".to_owned(),
            )),
            Self::Rgba8 | Self::Bgra8 => width
                .checked_mul(4)
                .ok_or(DdsImportError::Overflow("RGBA8 row length")),
            Self::Rgb24 | Self::Bgr24 => width
                .checked_mul(3)
                .ok_or(DdsImportError::Overflow("RGB24 row length")),
            Self::L8 => Ok(width),
        }
    }

    fn decode_rows(
        self,
        payload: &[u8],
        width: u32,
        height: u32,
        row_pitch: usize,
    ) -> Result<Vec<u8>, DdsImportError> {
        let packed_row = self.packed_row_len(width)?;
        if row_pitch < packed_row {
            return Err(DdsImportError::InvalidPayload(format!(
                "row pitch {row_pitch} is smaller than packed row {packed_row}"
            )));
        }
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or(DdsImportError::Overflow("decoded pixel count"))?;
        let output_len = pixel_count
            .checked_mul(4)
            .ok_or(DdsImportError::Overflow("decoded RGBA8 payload length"))?;
        let mut rgba = Vec::with_capacity(output_len);

        for row in 0..height as usize {
            let start = row
                .checked_mul(row_pitch)
                .ok_or(DdsImportError::Overflow("row offset"))?;
            let end = start
                .checked_add(packed_row)
                .ok_or(DdsImportError::Overflow("row end"))?;
            let row_bytes = payload.get(start..end).ok_or_else(|| {
                DdsImportError::InvalidPayload(format!(
                    "row {row} is truncated start={start} end={end} bytes={}",
                    payload.len()
                ))
            })?;
            match self {
                Self::Rgba8 => rgba.extend_from_slice(row_bytes),
                Self::Bgra8 => {
                    for pixel in row_bytes.chunks_exact(4) {
                        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                    }
                }
                Self::Rgb24 => {
                    for pixel in row_bytes.chunks_exact(3) {
                        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 0xff]);
                    }
                }
                Self::Bgr24 => {
                    for pixel in row_bytes.chunks_exact(3) {
                        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 0xff]);
                    }
                }
                Self::L8 => {
                    for &luma in row_bytes {
                        rgba.extend_from_slice(&[luma, luma, luma, 0xff]);
                    }
                }
                Self::Native => unreachable!("native DDS payload is not row-decoded"),
            }
        }
        debug_assert_eq!(rgba.len(), output_len);
        Ok(rgba)
    }
}

/// Imports a 2D DDS texture into the canonical runtime mip representation used by YTD.
///
/// Legacy BGRA8, RGB24/BGR24 and L8 source payloads are normalized to RGBA8 while
/// preserving every supplied mip level. BC payloads remain GPU-native and are copied
/// without decompression. Runtime texture references remain `.ytd@entry`; this decoder
/// is the shared engine/import-tool boundary for authoring DDS sources.
pub fn read_dds_runtime_texture(bytes: &[u8]) -> Result<DdsRuntimeTexture, DdsImportError> {
    if bytes.len() < 128 || bytes.get(0..4) != Some(b"DDS ") {
        return Err(DdsImportError::InvalidHeader(
            "missing DDS magic or 124-byte legacy header".to_owned(),
        ));
    }
    let header_size = read_u32_at(bytes, 4)?;
    if header_size != 124 {
        return Err(DdsImportError::InvalidHeader(format!(
            "unsupported header size {header_size}; expected 124"
        )));
    }
    let header_flags = read_u32_at(bytes, 8)?;
    let height = read_u32_at(bytes, 12)?;
    let width = read_u32_at(bytes, 16)?;
    if width == 0 || height == 0 {
        return Err(DdsImportError::InvalidExtent { width, height });
    }
    let pitch_or_linear_size = read_u32_at(bytes, 20)? as usize;
    let mip_count = read_u32_at(bytes, 28)?.max(1);
    let pixel_format_size = read_u32_at(bytes, 76)?;
    if pixel_format_size != 32 {
        return Err(DdsImportError::InvalidHeader(format!(
            "unsupported pixel-format size {pixel_format_size}; expected 32"
        )));
    }

    let pixel_format_flags = read_u32_at(bytes, 80)?;
    let fourcc = read_fourcc_at(bytes, 84)?;
    let rgb_bit_count = read_u32_at(bytes, 88)?;
    let r_mask = read_u32_at(bytes, 92)?;
    let g_mask = read_u32_at(bytes, 96)?;
    let b_mask = read_u32_at(bytes, 100)?;
    let a_mask = read_u32_at(bytes, 104)?;

    let mut data_offset = 128usize;
    let (format, source_layout) = if (pixel_format_flags & DDPF_FOURCC) != 0 {
        match &fourcc {
            b"DXT1" => (PIXEL_FORMAT_BC1_RGBA_UNORM.to_owned(), DdsSourceLayout::Native),
            b"DXT3" => (PIXEL_FORMAT_BC2_RGBA_UNORM.to_owned(), DdsSourceLayout::Native),
            b"DXT5" => (PIXEL_FORMAT_BC3_RGBA_UNORM.to_owned(), DdsSourceLayout::Native),
            b"ATI2" | b"BC5U" => {
                (PIXEL_FORMAT_BC5_RG_UNORM.to_owned(), DdsSourceLayout::Native)
            }
            b"DX10" => {
                if bytes.len() < 148 {
                    return Err(DdsImportError::InvalidHeader(
                        "DX10 header is truncated".to_owned(),
                    ));
                }
                data_offset = 148;
                let dxgi = read_u32_at(bytes, 128)?;
                dxgi_runtime_format(dxgi)?
            }
            other => {
                return Err(DdsImportError::UnsupportedFormat(format!(
                    "FourCC '{}'; supported DXT1/DXT3/DXT5/ATI2/BC5U and DX10 RGBA8/BC1/BC2/BC3/BC5/BC6H/BC7",
                    String::from_utf8_lossy(other)
                )))
            }
        }
    } else if (pixel_format_flags & DDPF_RGB) != 0 {
        match (rgb_bit_count, r_mask, g_mask, b_mask, a_mask) {
            (32, 0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0xff00_0000) => {
                (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::Rgba8)
            }
            (32, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000) => {
                (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::Bgra8)
            }
            (24, 0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0) => {
                (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::Rgb24)
            }
            (24, 0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0) => {
                (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::Bgr24)
            }
            _ => {
                return Err(DdsImportError::UnsupportedFormat(format!(
                    "legacy RGB bits={rgb_bit_count} flags={pixel_format_flags:#x} masks r={r_mask:#x} g={g_mask:#x} b={b_mask:#x} a={a_mask:#x}"
                )))
            }
        }
    } else if (pixel_format_flags & DDPF_LUMINANCE) != 0 {
        match (rgb_bit_count, r_mask, g_mask, b_mask, a_mask) {
            (8, 0x0000_00ff, 0, 0, 0) => {
                (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::L8)
            }
            _ => {
                return Err(DdsImportError::UnsupportedFormat(format!(
                    "legacy luminance bits={rgb_bit_count} masks r={r_mask:#x} g={g_mask:#x} b={b_mask:#x} a={a_mask:#x}"
                )))
            }
        }
    } else {
        return Err(DdsImportError::UnsupportedFormat(format!(
            "flags={pixel_format_flags:#x} bits={rgb_bit_count}"
        )));
    };

    let color_space = if format.ends_with("_SRGB") {
        COLOR_SPACE_SRGB.to_owned()
    } else {
        COLOR_SPACE_LINEAR.to_owned()
    };
    let available_payload = bytes.len().saturating_sub(data_offset);
    let mut mips = Vec::with_capacity(mip_count as usize);
    let mut offset = data_offset;
    let mut mip_width = width;
    let mut mip_height = height;

    if source_layout.is_native() {
        for level in 0..mip_count {
            let len = texture_payload_len(&format, mip_width, mip_height).map_err(|e| {
                DdsImportError::InvalidPayload(format!(
                    "native mip length failed level={level} format={format}: {e}"
                ))
            })?;
            let payload = checked_mip_slice(bytes, offset, len, level)?;
            mips.push(TextureEncodedMipData {
                level,
                width: mip_width,
                height: mip_height,
                bytes: payload.to_vec(),
            });
            offset = offset
                .checked_add(len)
                .ok_or(DdsImportError::Overflow("native mip offset"))?;
            mip_width = (mip_width / 2).max(1);
            mip_height = (mip_height / 2).max(1);
        }
    } else {
        let row_pitches = choose_legacy_row_pitches(
            source_layout,
            width,
            height,
            mip_count,
            header_flags,
            pitch_or_linear_size,
            available_payload,
        )?;
        for (level, row_pitch) in row_pitches.into_iter().enumerate() {
            let level = level as u32;
            let len = row_pitch
                .checked_mul(mip_height as usize)
                .ok_or(DdsImportError::Overflow("legacy mip source length"))?;
            let payload = checked_mip_slice(bytes, offset, len, level)?;
            let rgba = source_layout.decode_rows(payload, mip_width, mip_height, row_pitch)?;
            mips.push(TextureEncodedMipData {
                level,
                width: mip_width,
                height: mip_height,
                bytes: rgba,
            });
            offset = offset
                .checked_add(len)
                .ok_or(DdsImportError::Overflow("legacy mip offset"))?;
            mip_width = (mip_width / 2).max(1);
            mip_height = (mip_height / 2).max(1);
        }
    }

    Ok(DdsRuntimeTexture {
        width,
        height,
        format,
        color_space,
        mips,
    })
}

fn dxgi_runtime_format(dxgi: u32) -> Result<(String, DdsSourceLayout), DdsImportError> {
    let value = match dxgi {
        28 => (PIXEL_FORMAT_RGBA8_UNORM.to_owned(), DdsSourceLayout::Rgba8),
        29 => (PIXEL_FORMAT_RGBA8_SRGB.to_owned(), DdsSourceLayout::Rgba8),
        71 => (
            crate::PIXEL_FORMAT_BC1_RGBA_UNORM.to_owned(),
            DdsSourceLayout::Native,
        ),
        72 => (
            crate::PIXEL_FORMAT_BC1_RGBA_SRGB.to_owned(),
            DdsSourceLayout::Native,
        ),
        74 => (
            crate::PIXEL_FORMAT_BC2_RGBA_UNORM.to_owned(),
            DdsSourceLayout::Native,
        ),
        75 => (
            crate::PIXEL_FORMAT_BC2_RGBA_SRGB.to_owned(),
            DdsSourceLayout::Native,
        ),
        77 => (
            crate::PIXEL_FORMAT_BC3_RGBA_UNORM.to_owned(),
            DdsSourceLayout::Native,
        ),
        78 => (
            crate::PIXEL_FORMAT_BC3_RGBA_SRGB.to_owned(),
            DdsSourceLayout::Native,
        ),
        83 => (
            crate::PIXEL_FORMAT_BC5_RG_UNORM.to_owned(),
            DdsSourceLayout::Native,
        ),
        95 => (
            crate::PIXEL_FORMAT_BC6H_UF16.to_owned(),
            DdsSourceLayout::Native,
        ),
        96 => (
            crate::PIXEL_FORMAT_BC6H_SF16.to_owned(),
            DdsSourceLayout::Native,
        ),
        98 => (
            crate::PIXEL_FORMAT_BC7_RGBA_UNORM.to_owned(),
            DdsSourceLayout::Native,
        ),
        99 => (
            crate::PIXEL_FORMAT_BC7_RGBA_SRGB.to_owned(),
            DdsSourceLayout::Native,
        ),
        other => {
            return Err(DdsImportError::UnsupportedFormat(format!(
                "DXGI format {other}; supported RGBA8, BC1, BC2, BC3, BC5, BC6H and BC7"
            )))
        }
    };
    Ok(value)
}

fn choose_legacy_row_pitches(
    layout: DdsSourceLayout,
    width: u32,
    height: u32,
    mip_count: u32,
    header_flags: u32,
    header_pitch: usize,
    available_payload: usize,
) -> Result<Vec<usize>, DdsImportError> {
    let mut widths = Vec::with_capacity(mip_count as usize);
    let mut heights = Vec::with_capacity(mip_count as usize);
    let mut mip_width = width;
    let mut mip_height = height;
    for _ in 0..mip_count {
        widths.push(mip_width);
        heights.push(mip_height);
        mip_width = (mip_width / 2).max(1);
        mip_height = (mip_height / 2).max(1);
    }

    let tight = widths
        .iter()
        .map(|&w| layout.packed_row_len(w))
        .collect::<Result<Vec<_>, _>>()?;
    let aligned = tight
        .iter()
        .map(|&row| align_up(row, 4))
        .collect::<Result<Vec<_>, _>>()?;

    let mut candidates = Vec::<Vec<usize>>::new();
    if (header_flags & DDSD_PITCH) != 0 && header_pitch > 0 {
        if header_pitch < tight[0] {
            return Err(DdsImportError::InvalidPayload(format!(
                "header pitch {header_pitch} is smaller than packed top row {}",
                tight[0]
            )));
        }
        let mut header_tight = tight.clone();
        header_tight[0] = header_pitch;
        candidates.push(header_tight);
        let mut header_aligned = aligned.clone();
        header_aligned[0] = header_pitch;
        candidates.push(header_aligned);
    }
    candidates.push(tight);
    candidates.push(aligned);
    candidates.dedup();

    let mut exact = Vec::new();
    for candidate in candidates {
        let total = candidate_payload_len(&candidate, &heights)?;
        if total == available_payload {
            exact.push(candidate);
        }
    }
    if let Some(candidate) = exact.into_iter().next() {
        return Ok(candidate);
    }

    Err(DdsImportError::InvalidPayload(format!(
        "no tight/aligned row layout matches payload bytes={available_payload} extent={width}x{height} mips={mip_count} header_pitch={header_pitch}"
    )))
}

fn candidate_payload_len(row_pitches: &[usize], heights: &[u32]) -> Result<usize, DdsImportError> {
    row_pitches
        .iter()
        .zip(heights)
        .try_fold(0usize, |total, (&row_pitch, &height)| {
            let mip_len = row_pitch
                .checked_mul(height as usize)
                .ok_or(DdsImportError::Overflow("candidate mip length"))?;
            total
                .checked_add(mip_len)
                .ok_or(DdsImportError::Overflow("candidate payload length"))
        })
}

fn checked_mip_slice(
    bytes: &[u8],
    offset: usize,
    len: usize,
    level: u32,
) -> Result<&[u8], DdsImportError> {
    let end = offset
        .checked_add(len)
        .ok_or(DdsImportError::Overflow("mip end offset"))?;
    bytes.get(offset..end).ok_or(DdsImportError::TruncatedMip {
        level,
        offset,
        needed: len,
        available: bytes.len().saturating_sub(offset),
    })
}

fn align_up(value: usize, alignment: usize) -> Result<usize, DdsImportError> {
    let mask = alignment
        .checked_sub(1)
        .ok_or_else(|| DdsImportError::InvalidPayload("alignment must be non-zero".to_owned()))?;
    value
        .checked_add(mask)
        .map(|v| v & !mask)
        .ok_or(DdsImportError::Overflow("aligned row length"))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Result<u32, DdsImportError> {
    let slice = bytes.get(offset..offset + 4).ok_or_else(|| {
        DdsImportError::InvalidHeader(format!("truncated u32 at offset {offset}"))
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fourcc_at(bytes: &[u8], offset: usize) -> Result<[u8; 4], DdsImportError> {
    let slice = bytes.get(offset..offset + 4).ok_or_else(|| {
        DdsImportError::InvalidHeader(format!("truncated FourCC at offset {offset}"))
    })?;
    Ok([slice[0], slice[1], slice[2], slice[3]])
}

#[derive(Debug, thiserror::Error)]
pub enum DdsExportError {
    #[error("dds: invalid extent {width}x{height}")]
    InvalidExtent { width: u32, height: u32 },
    #[error("dds: invalid payload bytes={bytes} expected={expected} extent={width}x{height}")]
    InvalidPayload {
        bytes: usize,
        expected: usize,
        width: u32,
        height: u32,
    },
    #[error("dds: mip generation failed: {0}")]
    MipGeneration(String),
    #[error("dds: unsupported pixel format '{0}'")]
    UnsupportedFormat(String),
}

/// Writes a simple uncompressed RGBA8 DDS file with a full generated mip chain.
///
/// This is an authoring/export helper for the texture tool. Runtime never reads DDS.
pub fn write_dds_rgba8(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> std::result::Result<Vec<u8>, DdsExportError> {
    if width == 0 || height == 0 {
        return Err(DdsExportError::InvalidExtent { width, height });
    }
    let expected = rgba8_len(width, height);
    if rgba.len() != expected {
        return Err(DdsExportError::InvalidPayload {
            bytes: rgba.len(),
            expected,
            width,
            height,
        });
    }
    let mips = generate_rgba8_mips(width, height, rgba.to_vec())
        .map_err(|e| DdsExportError::MipGeneration(e.to_string()))?;
    write_dds_rgba8_mip_chain(width, height, &mips)
}

pub fn write_dds_rgba8_mip_chain(
    width: u32,
    height: u32,
    mips: &[TextureMipData],
) -> std::result::Result<Vec<u8>, DdsExportError> {
    let encoded = mips
        .iter()
        .map(|m| TextureEncodedMipData {
            level: m.level,
            width: m.width,
            height: m.height,
            bytes: m.rgba.clone(),
        })
        .collect::<Vec<_>>();
    write_dds_runtime_mip_chain(width, height, crate::PIXEL_FORMAT_RGBA8_UNORM, &encoded)
}

pub fn write_dds_runtime_mip_chain(
    width: u32,
    height: u32,
    format: &str,
    mips: &[TextureEncodedMipData],
) -> std::result::Result<Vec<u8>, DdsExportError> {
    if width == 0 || height == 0 {
        return Err(DdsExportError::InvalidExtent { width, height });
    }
    if mips.is_empty() {
        return Err(DdsExportError::InvalidPayload {
            bytes: 0,
            expected: rgba8_len(width, height),
            width,
            height,
        });
    }
    let pixel_format = parse_pixel_format(format, "dds")
        .map_err(|_| DdsExportError::UnsupportedFormat(format.to_owned()))?;
    let payload_len = mips.iter().map(|m| m.bytes.len()).sum::<usize>();
    let mip_count = mips.len() as u32;
    let has_mips = mip_count > 1;

    let mut out = Vec::with_capacity(4 + 124 + 20 + payload_len);
    out.extend_from_slice(b"DDS ");
    write_u32(&mut out, 124); // dwSize
    let mut flags = 0x0000_1007; // CAPS | HEIGHT | WIDTH | PIXELFORMAT
    if pixel_format.is_rgba8() {
        flags |= 0x0000_0008;
    }
    // PITCH
    else {
        flags |= 0x0008_0000;
    } // LINEARSIZE
    if has_mips {
        flags |= 0x0002_0000;
    }
    write_u32(&mut out, flags);
    write_u32(&mut out, height);
    write_u32(&mut out, width);
    write_u32(&mut out, pitch_or_linear_size(pixel_format, width, height));
    write_u32(&mut out, 0);
    write_u32(&mut out, mip_count);
    for _ in 0..11 {
        write_u32(&mut out, 0);
    }

    write_u32(&mut out, 32); // DDPIXELFORMAT size
    match pixel_format {
        TexturePixelFormat::Rgba8Unorm | TexturePixelFormat::Rgba8Srgb => {
            write_u32(&mut out, 0x0000_0041); // DDPF_RGB | 0x0000_0001
            write_u32(&mut out, 0);
            write_u32(&mut out, 32);
            write_u32(&mut out, 0x0000_00ff);
            write_u32(&mut out, 0x0000_ff00);
            write_u32(&mut out, 0x00ff_0000);
            write_u32(&mut out, 0xff00_0000);
        }
        TexturePixelFormat::Bc1RgbaUnorm | TexturePixelFormat::Bc1RgbaSrgb => {
            write_fourcc_pf(&mut out, *b"DXT1")
        }
        TexturePixelFormat::Bc2RgbaUnorm => write_fourcc_pf(&mut out, *b"DXT3"),
        TexturePixelFormat::Bc2RgbaSrgb => write_fourcc_pf(&mut out, *b"DX10"),
        TexturePixelFormat::Bc3RgbaUnorm | TexturePixelFormat::Bc3RgbaSrgb => {
            write_fourcc_pf(&mut out, *b"DXT5")
        }
        TexturePixelFormat::Bc5RgUnorm
        | TexturePixelFormat::Bc6hUf16
        | TexturePixelFormat::Bc6hSf16
        | TexturePixelFormat::Bc7RgbaUnorm
        | TexturePixelFormat::Bc7RgbaSrgb => {
            // BC5/BC6/BC7 and sRGB BC2 require DX10 header for unambiguous tooling import.
            write_fourcc_pf(&mut out, *b"DX10");
        }
    }

    let mut caps = 0x0000_1000;
    if has_mips {
        caps |= 0x0000_0008 | 0x0040_0000;
    }
    write_u32(&mut out, caps);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);
    write_u32(&mut out, 0);

    if matches!(
        pixel_format,
        TexturePixelFormat::Bc2RgbaSrgb
            | TexturePixelFormat::Bc5RgUnorm
            | TexturePixelFormat::Bc6hUf16
            | TexturePixelFormat::Bc6hSf16
            | TexturePixelFormat::Bc7RgbaUnorm
            | TexturePixelFormat::Bc7RgbaSrgb
    ) {
        write_u32(&mut out, dxgi_format(pixel_format));
        write_u32(&mut out, 3); // DDS_DIMENSION_TEXTURE2D
        write_u32(&mut out, 0);
        write_u32(&mut out, 1);
        write_u32(&mut out, 0);
    }

    for mip in mips {
        out.extend_from_slice(&mip.bytes);
    }
    Ok(out)
}

#[inline]
fn write_fourcc_pf(out: &mut Vec<u8>, cc: [u8; 4]) {
    write_u32(out, 0x0000_0004); // DDPF_FOURCC
    out.extend_from_slice(&cc);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0);
}

#[inline]
fn pitch_or_linear_size(format: TexturePixelFormat, width: u32, height: u32) -> u32 {
    match format {
        TexturePixelFormat::Rgba8Unorm | TexturePixelFormat::Rgba8Srgb => width.saturating_mul(4),
        TexturePixelFormat::Bc1RgbaUnorm | TexturePixelFormat::Bc1RgbaSrgb => width
            .div_ceil(4)
            .saturating_mul(height.div_ceil(4))
            .saturating_mul(8),
        _ => width
            .div_ceil(4)
            .saturating_mul(height.div_ceil(4))
            .saturating_mul(16),
    }
}

#[inline]
fn dxgi_format(format: TexturePixelFormat) -> u32 {
    match format {
        TexturePixelFormat::Bc2RgbaUnorm => 74, // DXGI_FORMAT_BC2_UNORM
        TexturePixelFormat::Bc2RgbaSrgb => 75,  // DXGI_FORMAT_BC2_UNORM_SRGB
        TexturePixelFormat::Bc5RgUnorm => 83,   // DXGI_FORMAT_BC5_UNORM
        TexturePixelFormat::Bc6hUf16 => 95,     // DXGI_FORMAT_BC6H_UF16
        TexturePixelFormat::Bc6hSf16 => 96,     // DXGI_FORMAT_BC6H_SF16
        TexturePixelFormat::Bc7RgbaUnorm => 98, // DXGI_FORMAT_BC7_UNORM
        TexturePixelFormat::Bc7RgbaSrgb => 99,  // DXGI_FORMAT_BC7_UNORM_SRGB
        _ => 0,
    }
}

#[inline]
fn write_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

#[cfg(test)]
mod import_tests {
    use super::*;

    fn legacy_dds(
        width: u32,
        height: u32,
        mip_count: u32,
        header_flags: u32,
        pitch: u32,
        pixel_format_flags: u32,
        bit_count: u32,
        masks: [u32; 4],
        payload: &[u8],
    ) -> Vec<u8> {
        let mut bytes = vec![0u8; 128];
        bytes[0..4].copy_from_slice(b"DDS ");
        write_at(&mut bytes, 4, 124);
        write_at(&mut bytes, 8, header_flags);
        write_at(&mut bytes, 12, height);
        write_at(&mut bytes, 16, width);
        write_at(&mut bytes, 20, pitch);
        write_at(&mut bytes, 28, mip_count);
        write_at(&mut bytes, 76, 32);
        write_at(&mut bytes, 80, pixel_format_flags);
        write_at(&mut bytes, 88, bit_count);
        write_at(&mut bytes, 92, masks[0]);
        write_at(&mut bytes, 96, masks[1]);
        write_at(&mut bytes, 100, masks[2]);
        write_at(&mut bytes, 104, masks[3]);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn write_at(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn imports_bgra8_and_preserves_mips() {
        let bytes = legacy_dds(
            2,
            1,
            2,
            0x0002_1007,
            0,
            DDPF_RGB | 0x0000_0001,
            32,
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000],
            &[1, 2, 3, 4, 10, 20, 30, 40, 5, 6, 7, 8],
        );
        let texture = read_dds_runtime_texture(&bytes).expect("BGRA8 import");
        assert_eq!(texture.format, PIXEL_FORMAT_RGBA8_UNORM);
        assert_eq!(texture.mips.len(), 2);
        assert_eq!(texture.mips[0].bytes, vec![3, 2, 1, 4, 30, 20, 10, 40]);
        assert_eq!(texture.mips[1].bytes, vec![7, 6, 5, 8]);
    }

    #[test]
    fn imports_rgb24_and_adds_opaque_alpha() {
        let bytes = legacy_dds(
            2,
            1,
            1,
            0x0000_1007,
            0,
            DDPF_RGB,
            24,
            [0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0],
            &[1, 2, 3, 10, 20, 30],
        );
        let texture = read_dds_runtime_texture(&bytes).expect("RGB24 import");
        assert_eq!(texture.mips[0].bytes, vec![1, 2, 3, 0xff, 10, 20, 30, 0xff]);
    }

    #[test]
    fn imports_bgr24_and_reorders_channels() {
        let bytes = legacy_dds(
            1,
            1,
            1,
            0x0000_1007,
            0,
            DDPF_RGB,
            24,
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0],
            &[1, 2, 3],
        );
        let texture = read_dds_runtime_texture(&bytes).expect("BGR24 import");
        assert_eq!(texture.mips[0].bytes, vec![3, 2, 1, 0xff]);
    }

    #[test]
    fn imports_l8_as_rgba8_across_mips() {
        let bytes = legacy_dds(
            2,
            1,
            2,
            0x0002_1007,
            0,
            DDPF_LUMINANCE,
            8,
            [0x0000_00ff, 0, 0, 0],
            &[7, 9, 11],
        );
        let texture = read_dds_runtime_texture(&bytes).expect("L8 import");
        assert_eq!(texture.mips[0].bytes, vec![7, 7, 7, 0xff, 9, 9, 9, 0xff]);
        assert_eq!(texture.mips[1].bytes, vec![11, 11, 11, 0xff]);
    }

    #[test]
    fn imported_bgra8_round_trips_through_runtime_dictionary() {
        let bytes = legacy_dds(
            1,
            1,
            1,
            0x0000_1007,
            0,
            DDPF_RGB | 0x0000_0001,
            32,
            [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000],
            &[10, 20, 30, 40],
        );
        let imported = read_dds_runtime_texture(&bytes).expect("BGRA8 import");
        let netd = crate::pack_encoded(vec![crate::TextureEncodedBuildEntry {
            name: "bgra_runtime".to_owned(),
            width: imported.width,
            height: imported.height,
            format: imported.format,
            color_space: imported.color_space,
            mips: imported.mips,
        }])
        .expect("runtime dictionary pack");
        let dictionary = crate::parse(&netd).expect("runtime dictionary parse");
        let entry = dictionary.entry("bgra_runtime").expect("runtime entry");
        assert_eq!(entry.meta.format, PIXEL_FORMAT_RGBA8_UNORM);
        assert_eq!(entry.mip_bytes(0), Some(&[30, 20, 10, 40][..]));
    }

    #[test]
    fn honors_dword_row_padding_for_rgb24_mips() {
        let bytes = legacy_dds(
            3,
            1,
            2,
            0x0002_100f,
            12,
            DDPF_RGB,
            24,
            [0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 10, 20, 30, 0],
        );
        let texture = read_dds_runtime_texture(&bytes).expect("padded RGB24 import");
        assert_eq!(texture.mips[0].bytes.len(), 3 * 4);
        assert_eq!(texture.mips[1].bytes, vec![10, 20, 30, 0xff]);
    }
}
