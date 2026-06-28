use crate::format::{
    parse_pixel_format, TexturePixelFormat, PIXEL_FORMAT_BC1_RGBA_SRGB,
    PIXEL_FORMAT_BC1_RGBA_UNORM, PIXEL_FORMAT_BC3_RGBA_SRGB, PIXEL_FORMAT_BC3_RGBA_UNORM,
    PIXEL_FORMAT_BC5_RG_UNORM,
};
use crate::mips::{rgba8_len, TextureEncodedMipData, TextureMipData};
use crate::{COLOR_SPACE_LINEAR, COLOR_SPACE_SRGB};

#[derive(Debug, thiserror::Error)]
pub enum BcnEncodeError {
    #[error(
        "bcn: invalid RGBA8 payload bytes={bytes} expected={expected} extent={width}x{height}"
    )]
    InvalidRgbaPayload {
        bytes: usize,
        expected: usize,
        width: u32,
        height: u32,
    },
    #[error("bcn: unsupported encoder target '{0}'")]
    UnsupportedTarget(String),
    #[error("bcn: unsupported decoder source '{0}'")]
    UnsupportedSource(String),
    #[error("bcn: invalid BCn payload bytes={bytes} expected={expected} extent={width}x{height} format={format}")]
    InvalidBcnPayload {
        bytes: usize,
        expected: usize,
        width: u32,
        height: u32,
        format: String,
    },
}

pub fn infer_bcn_format(name: &str, color_space: &str, rgba: &[u8]) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("normal")
        || lower.contains("roughness")
        || lower.contains("metallic")
        || lower.contains("occlusion")
        || lower.contains("_ao")
    {
        return PIXEL_FORMAT_BC5_RG_UNORM;
    }

    let has_alpha = rgba.chunks_exact(4).any(|px| px[3] < 250)
        || lower.contains("opacity")
        || lower.contains("alpha")
        || lower.contains("leaf")
        || lower.contains("specularglossiness")
        || lower.contains("specular_glossiness");
    if has_alpha {
        return if color_space.eq_ignore_ascii_case(COLOR_SPACE_SRGB) {
            PIXEL_FORMAT_BC3_RGBA_SRGB
        } else {
            PIXEL_FORMAT_BC3_RGBA_UNORM
        };
    }

    if color_space.eq_ignore_ascii_case(COLOR_SPACE_LINEAR) {
        PIXEL_FORMAT_BC1_RGBA_UNORM
    } else {
        PIXEL_FORMAT_BC1_RGBA_SRGB
    }
}

pub fn encode_rgba8_mips_to_bcn(
    format: &str,
    mips: &[TextureMipData],
) -> Result<Vec<TextureEncodedMipData>, BcnEncodeError> {
    let target = parse_pixel_format(format, "<bcn>")
        .map_err(|_| BcnEncodeError::UnsupportedTarget(format.to_owned()))?;
    let mut out = Vec::with_capacity(mips.len());
    for mip in mips {
        let expected = rgba8_len(mip.width, mip.height);
        if mip.rgba.len() != expected {
            return Err(BcnEncodeError::InvalidRgbaPayload {
                bytes: mip.rgba.len(),
                expected,
                width: mip.width,
                height: mip.height,
            });
        }
        let bytes = match target {
            TexturePixelFormat::Bc1RgbaUnorm | TexturePixelFormat::Bc1RgbaSrgb => {
                encode_bc1(mip.width, mip.height, &mip.rgba)
            }
            TexturePixelFormat::Bc3RgbaUnorm | TexturePixelFormat::Bc3RgbaSrgb => {
                encode_bc3(mip.width, mip.height, &mip.rgba)
            }
            TexturePixelFormat::Bc5RgUnorm => encode_bc5(mip.width, mip.height, &mip.rgba),
            _ => return Err(BcnEncodeError::UnsupportedTarget(format.to_owned())),
        };
        out.push(TextureEncodedMipData {
            level: mip.level,
            width: mip.width,
            height: mip.height,
            bytes,
        });
    }
    Ok(out)
}

pub fn decode_bcn_to_rgba8(
    format: &str,
    width: u32,
    height: u32,
    bytes: &[u8],
) -> Result<Vec<u8>, BcnEncodeError> {
    let source = parse_pixel_format(format, "<bcn>")
        .map_err(|_| BcnEncodeError::UnsupportedSource(format.to_owned()))?;
    match source {
        TexturePixelFormat::Bc1RgbaUnorm | TexturePixelFormat::Bc1RgbaSrgb => {
            decode_bc1(width, height, bytes, format)
        }
        TexturePixelFormat::Bc3RgbaUnorm | TexturePixelFormat::Bc3RgbaSrgb => {
            decode_bc3(width, height, bytes, format)
        }
        TexturePixelFormat::Bc5RgUnorm => decode_bc5(width, height, bytes, format),
        _ => Err(BcnEncodeError::UnsupportedSource(format.to_owned())),
    }
}

#[inline]
fn blocks(width: u32, height: u32) -> (usize, usize) {
    (((width as usize) + 3) / 4, ((height as usize) + 3) / 4)
}

#[inline]
fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    (((r as u16) >> 3) << 11) | (((g as u16) >> 2) << 5) | ((b as u16) >> 3)
}

#[inline]
fn unpack_rgb565(v: u16) -> [u8; 3] {
    let r = ((v >> 11) & 0x1f) as u8;
    let g = ((v >> 5) & 0x3f) as u8;
    let b = (v & 0x1f) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

#[inline]
fn sample_rgba(width: u32, height: u32, rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
    let sx = x.min(width.saturating_sub(1));
    let sy = y.min(height.saturating_sub(1));
    let i = ((sy as usize * width as usize) + sx as usize) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
}

fn encode_bc1(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let (bw, bh) = blocks(width, height);
    let mut out = Vec::with_capacity(bw * bh * 8);
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let mut min = [255u8; 3];
            let mut max = [0u8; 3];
            let mut px = [[0u8; 4]; 16];
            for y in 0..4u32 {
                for x in 0..4u32 {
                    let p = sample_rgba(width, height, rgba, bx * 4 + x, by * 4 + y);
                    px[(y * 4 + x) as usize] = p;
                    for c in 0..3 {
                        min[c] = min[c].min(p[c]);
                        max[c] = max[c].max(p[c]);
                    }
                }
            }
            let mut c0 = rgb565(max[0], max[1], max[2]);
            let mut c1 = rgb565(min[0], min[1], min[2]);
            if c0 <= c1 {
                core::mem::swap(&mut c0, &mut c1);
            }
            let p0 = unpack_rgb565(c0);
            let p1 = unpack_rgb565(c1);
            let palette = [
                p0,
                p1,
                [
                    ((2 * p0[0] as u16 + p1[0] as u16) / 3) as u8,
                    ((2 * p0[1] as u16 + p1[1] as u16) / 3) as u8,
                    ((2 * p0[2] as u16 + p1[2] as u16) / 3) as u8,
                ],
                [
                    ((p0[0] as u16 + 2 * p1[0] as u16) / 3) as u8,
                    ((p0[1] as u16 + 2 * p1[1] as u16) / 3) as u8,
                    ((p0[2] as u16 + 2 * p1[2] as u16) / 3) as u8,
                ],
            ];
            let mut indices = 0u32;
            for (i, p) in px.iter().enumerate() {
                let mut best = 0usize;
                let mut best_d = u32::MAX;
                for (j, q) in palette.iter().enumerate() {
                    let dr = p[0] as i32 - q[0] as i32;
                    let dg = p[1] as i32 - q[1] as i32;
                    let db = p[2] as i32 - q[2] as i32;
                    let d = (dr * dr + dg * dg + db * db) as u32;
                    if d < best_d {
                        best_d = d;
                        best = j;
                    }
                }
                indices |= (best as u32) << (i * 2);
            }
            out.extend_from_slice(&c0.to_le_bytes());
            out.extend_from_slice(&c1.to_le_bytes());
            out.extend_from_slice(&indices.to_le_bytes());
        }
    }
    out
}

fn encode_bc3(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let (bw, bh) = blocks(width, height);
    let mut out = Vec::with_capacity(bw * bh * 16);
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let mut alpha = [255u8; 16];
            let mut block_rgba = [0u8; 64];
            for y in 0..4u32 {
                for x in 0..4u32 {
                    let p = sample_rgba(width, height, rgba, bx * 4 + x, by * 4 + y);
                    let i = (y * 4 + x) as usize;
                    alpha[i] = p[3];
                    block_rgba[i * 4..i * 4 + 4].copy_from_slice(&p);
                }
            }
            out.extend_from_slice(&encode_bc4_block(&alpha));
            out.extend_from_slice(&encode_bc1(4, 4, &block_rgba));
        }
    }
    out
}

fn encode_bc5(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let (bw, bh) = blocks(width, height);
    let mut out = Vec::with_capacity(bw * bh * 16);
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let mut red = [128u8; 16];
            let mut green = [128u8; 16];
            for y in 0..4u32 {
                for x in 0..4u32 {
                    let p = sample_rgba(width, height, rgba, bx * 4 + x, by * 4 + y);
                    let i = (y * 4 + x) as usize;
                    red[i] = p[0];
                    green[i] = p[1];
                }
            }
            out.extend_from_slice(&encode_bc4_block(&red));
            out.extend_from_slice(&encode_bc4_block(&green));
        }
    }
    out
}

fn encode_bc4_block(values: &[u8; 16]) -> [u8; 8] {
    let mut min = 255u8;
    let mut max = 0u8;
    for &v in values {
        min = min.min(v);
        max = max.max(v);
    }
    let a0 = max;
    let a1 = min;
    let palette = alpha_palette(a0, a1);
    let mut bits = 0u64;
    for (i, &v) in values.iter().enumerate() {
        let mut best = 0u64;
        let mut best_d = u16::MAX;
        for (j, &p) in palette.iter().enumerate() {
            let d = (v as i16 - p as i16).unsigned_abs();
            if d < best_d {
                best_d = d;
                best = j as u64;
            }
        }
        bits |= best << (i * 3);
    }
    let mut out = [0u8; 8];
    out[0] = a0;
    out[1] = a1;
    for i in 0..6 {
        out[2 + i] = ((bits >> (8 * i)) & 0xff) as u8;
    }
    out
}

fn alpha_palette(a0: u8, a1: u8) -> [u8; 8] {
    if a0 > a1 {
        [
            a0,
            a1,
            ((6 * a0 as u16 + a1 as u16) / 7) as u8,
            ((5 * a0 as u16 + 2 * a1 as u16) / 7) as u8,
            ((4 * a0 as u16 + 3 * a1 as u16) / 7) as u8,
            ((3 * a0 as u16 + 4 * a1 as u16) / 7) as u8,
            ((2 * a0 as u16 + 5 * a1 as u16) / 7) as u8,
            ((a0 as u16 + 6 * a1 as u16) / 7) as u8,
        ]
    } else {
        [
            a0,
            a1,
            ((4 * a0 as u16 + a1 as u16) / 5) as u8,
            ((3 * a0 as u16 + 2 * a1 as u16) / 5) as u8,
            ((2 * a0 as u16 + 3 * a1 as u16) / 5) as u8,
            ((a0 as u16 + 4 * a1 as u16) / 5) as u8,
            0,
            255,
        ]
    }
}

fn decode_bc1(
    width: u32,
    height: u32,
    bytes: &[u8],
    format: &str,
) -> Result<Vec<u8>, BcnEncodeError> {
    let (bw, bh) = blocks(width, height);
    let expected = bw * bh * 8;
    if bytes.len() != expected {
        return Err(BcnEncodeError::InvalidBcnPayload {
            bytes: bytes.len(),
            expected,
            width,
            height,
            format: format.to_owned(),
        });
    }
    let mut out = vec![0u8; rgba8_len(width, height)];
    let mut cursor = 0usize;
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let c0 = u16::from_le_bytes([bytes[cursor], bytes[cursor + 1]]);
            let c1 = u16::from_le_bytes([bytes[cursor + 2], bytes[cursor + 3]]);
            let indices = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
            cursor += 8;
            let p0 = unpack_rgb565(c0);
            let p1 = unpack_rgb565(c1);
            let palette = [
                [p0[0], p0[1], p0[2], 255],
                [p1[0], p1[1], p1[2], 255],
                [
                    ((2 * p0[0] as u16 + p1[0] as u16) / 3) as u8,
                    ((2 * p0[1] as u16 + p1[1] as u16) / 3) as u8,
                    ((2 * p0[2] as u16 + p1[2] as u16) / 3) as u8,
                    255,
                ],
                [
                    ((p0[0] as u16 + 2 * p1[0] as u16) / 3) as u8,
                    ((p0[1] as u16 + 2 * p1[1] as u16) / 3) as u8,
                    ((p0[2] as u16 + 2 * p1[2] as u16) / 3) as u8,
                    255,
                ],
            ];
            write_decoded_block(width, height, &mut out, bx, by, |i| {
                palette[((indices >> (i * 2)) & 3) as usize]
            });
        }
    }
    Ok(out)
}

fn decode_bc3(
    width: u32,
    height: u32,
    bytes: &[u8],
    format: &str,
) -> Result<Vec<u8>, BcnEncodeError> {
    let (bw, bh) = blocks(width, height);
    let expected = bw * bh * 16;
    if bytes.len() != expected {
        return Err(BcnEncodeError::InvalidBcnPayload {
            bytes: bytes.len(),
            expected,
            width,
            height,
            format: format.to_owned(),
        });
    }
    let mut out = vec![0u8; rgba8_len(width, height)];
    let mut cursor = 0usize;
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let alphas = decode_bc4_block(&bytes[cursor..cursor + 8]);
            let colors = decode_bc1(4, 4, &bytes[cursor + 8..cursor + 16], format)?;
            cursor += 16;
            write_decoded_block(width, height, &mut out, bx, by, |i| {
                [
                    colors[i * 4],
                    colors[i * 4 + 1],
                    colors[i * 4 + 2],
                    alphas[i],
                ]
            });
        }
    }
    Ok(out)
}

fn decode_bc5(
    width: u32,
    height: u32,
    bytes: &[u8],
    format: &str,
) -> Result<Vec<u8>, BcnEncodeError> {
    let (bw, bh) = blocks(width, height);
    let expected = bw * bh * 16;
    if bytes.len() != expected {
        return Err(BcnEncodeError::InvalidBcnPayload {
            bytes: bytes.len(),
            expected,
            width,
            height,
            format: format.to_owned(),
        });
    }
    let mut out = vec![0u8; rgba8_len(width, height)];
    let mut cursor = 0usize;
    for by in 0..bh as u32 {
        for bx in 0..bw as u32 {
            let r = decode_bc4_block(&bytes[cursor..cursor + 8]);
            let g = decode_bc4_block(&bytes[cursor + 8..cursor + 16]);
            cursor += 16;
            write_decoded_block(width, height, &mut out, bx, by, |i| [r[i], g[i], 255, 255]);
        }
    }
    Ok(out)
}

fn decode_bc4_block(bytes: &[u8]) -> [u8; 16] {
    let palette = alpha_palette(bytes[0], bytes[1]);
    let mut bits = 0u64;
    for i in 0..6 {
        bits |= (bytes[2 + i] as u64) << (8 * i);
    }
    let mut out = [0u8; 16];
    for (i, v) in out.iter_mut().enumerate() {
        *v = palette[((bits >> (i * 3)) & 7) as usize];
    }
    out
}

fn write_decoded_block<F: Fn(usize) -> [u8; 4]>(
    width: u32,
    height: u32,
    out: &mut [u8],
    bx: u32,
    by: u32,
    sample: F,
) {
    for y in 0..4u32 {
        for x in 0..4u32 {
            let dx = bx * 4 + x;
            let dy = by * 4 + y;
            if dx >= width || dy >= height {
                continue;
            }
            let dst = ((dy as usize * width as usize) + dx as usize) * 4;
            let p = sample((y * 4 + x) as usize);
            out[dst..dst + 4].copy_from_slice(&p);
        }
    }
}
