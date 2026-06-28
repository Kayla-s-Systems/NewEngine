use crate::error::{Result, TextureContainerError};

#[derive(Debug, Clone)]
pub struct TextureMipData {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TextureEncodedMipData {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

pub fn generate_rgba8_mips(
    width: u32,
    height: u32,
    base_rgba: Vec<u8>,
) -> Result<Vec<TextureMipData>> {
    if width == 0 || height == 0 {
        return Err(TextureContainerError::InvalidExtent {
            name: "<generated>".to_owned(),
            width,
            height,
        });
    }
    let expected = rgba8_len(width, height);
    if base_rgba.len() != expected {
        return Err(TextureContainerError::PayloadSizeMismatch {
            name: "<generated>".to_owned(),
            mip: 0,
            bytes: base_rgba.len(),
            expected,
        });
    }

    let mut out = Vec::new();
    out.push(TextureMipData {
        level: 0,
        width,
        height,
        rgba: base_rgba,
    });
    while out
        .last()
        .map(|m| m.width > 1 || m.height > 1)
        .unwrap_or(false)
    {
        let prev = out.last().expect("at least base mip exists");
        let next_w = (prev.width / 2).max(1);
        let next_h = (prev.height / 2).max(1);
        let next = downsample_rgba8_box(prev.width, prev.height, &prev.rgba, next_w, next_h);
        out.push(TextureMipData {
            level: out.len() as u32,
            width: next_w,
            height: next_h,
            rgba: next,
        });
    }
    Ok(out)
}

#[inline]
pub fn rgba8_len(width: u32, height: u32) -> usize {
    (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4)
}

fn downsample_rgba8_box(src_w: u32, src_h: u32, src: &[u8], dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; rgba8_len(dst_w, dst_h)];
    for y in 0..dst_h {
        for x in 0..dst_w {
            let sx0 = (x * 2).min(src_w - 1);
            let sy0 = (y * 2).min(src_h - 1);
            let sx1 = (sx0 + 1).min(src_w - 1);
            let sy1 = (sy0 + 1).min(src_h - 1);
            let samples = [(sx0, sy0), (sx1, sy0), (sx0, sy1), (sx1, sy1)];
            let mut acc = [0u32; 4];
            for (sx, sy) in samples {
                let i = ((sy as usize * src_w as usize) + sx as usize) * 4;
                acc[0] += src[i] as u32;
                acc[1] += src[i + 1] as u32;
                acc[2] += src[i + 2] as u32;
                acc[3] += src[i + 3] as u32;
            }
            let o = ((y as usize * dst_w as usize) + x as usize) * 4;
            out[o] = (acc[0] / 4) as u8;
            out[o + 1] = (acc[1] / 4) as u8;
            out[o + 2] = (acc[2] / 4) as u8;
            out[o + 3] = (acc[3] / 4) as u8;
        }
    }
    out
}
