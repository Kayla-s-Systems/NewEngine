use crate::pak::PakFile;

/// DXGI formats observed in TLOU2 PC `VRAM_DESC` resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportedTextureFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Bc1Unorm,
    Bc1Srgb,
    Bc4Unorm,
    Bc5Unorm,
    Bc6hUf16,
    Bc7Unorm,
    Bc7Srgb,
    Other(u32),
}

impl ImportedTextureFormat {
    pub const fn from_dxgi(value: u32) -> Self {
        match value {
            28 => Self::Rgba8Unorm,
            29 => Self::Rgba8Srgb,
            71 => Self::Bc1Unorm,
            72 => Self::Bc1Srgb,
            80 => Self::Bc4Unorm,
            83 => Self::Bc5Unorm,
            95 => Self::Bc6hUf16,
            98 => Self::Bc7Unorm,
            99 => Self::Bc7Srgb,
            other => Self::Other(other),
        }
    }

    #[inline]
    pub const fn dxgi(self) -> u32 {
        match self {
            Self::Rgba8Unorm => 28,
            Self::Rgba8Srgb => 29,
            Self::Bc1Unorm => 71,
            Self::Bc1Srgb => 72,
            Self::Bc4Unorm => 80,
            Self::Bc5Unorm => 83,
            Self::Bc6hUf16 => 95,
            Self::Bc7Unorm => 98,
            Self::Bc7Srgb => 99,
            Self::Other(value) => value,
        }
    }

    #[inline]
    pub const fn is_srgb(self) -> bool {
        matches!(self, Self::Rgba8Srgb | Self::Bc1Srgb | Self::Bc7Srgb)
    }

    #[inline]
    pub const fn block_extent(self) -> Option<u32> {
        match self {
            Self::Rgba8Unorm | Self::Rgba8Srgb => Some(1),
            Self::Bc1Unorm
            | Self::Bc1Srgb
            | Self::Bc4Unorm
            | Self::Bc5Unorm
            | Self::Bc6hUf16
            | Self::Bc7Unorm
            | Self::Bc7Srgb => Some(4),
            Self::Other(_) => None,
        }
    }

    #[inline]
    pub const fn bytes_per_element(self) -> Option<usize> {
        match self {
            Self::Rgba8Unorm | Self::Rgba8Srgb => Some(4),
            Self::Bc1Unorm | Self::Bc1Srgb | Self::Bc4Unorm => Some(8),
            Self::Bc5Unorm | Self::Bc6hUf16 | Self::Bc7Unorm | Self::Bc7Srgb => Some(16),
            Self::Other(_) => None,
        }
    }

    #[inline]
    pub const fn validated_1d_thin_detile(self) -> bool {
        // The 64-bit BC block route was validated on eight independent TLOU2 PC VFX textures.
        // 128-bit BC5/BC7 source resources demonstrably require an additional layout rule and
        // remain intentionally rejected until that rule is proven from corpus/native tooling.
        matches!(self, Self::Bc1Unorm | Self::Bc1Srgb | Self::Bc4Unorm)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportedVramTexture {
    pub source_path: String,
    pub source_hash: u64,
    pub pak_offset: u32,
    pub vram_size: u32,
    pub texture_type: u32,
    pub format: ImportedTextureFormat,
    pub mip_count: u32,
    pub width: u32,
    pub height: u32,
    pub stream_flags: u64,
    pub absolute_data_offset: usize,
}

impl ImportedVramTexture {
    #[inline]
    pub fn logical_name(&self) -> &str {
        let path = self
            .source_path
            .strip_prefix("[BUILD_INTERMEDIATE]")
            .unwrap_or(&self.source_path);
        let before_hash = path.rsplit_once('/').map(|(head, _)| head).unwrap_or(path);
        before_hash.rsplit('/').next().unwrap_or(before_hash)
    }

    pub fn base_linear_bytes(&self, pak: &PakFile) -> Result<Vec<u8>, String> {
        if !self.format.validated_1d_thin_detile() {
            return Err(format!(
                "TLOU2 1D-thin base detile is not validated for DXGI={} path='{}'",
                self.format.dxgi(),
                self.source_path
            ));
        }
        let block_extent = self.format.block_extent().ok_or_else(|| {
            format!(
                "unsupported TLOU2 texture DXGI={} path='{}'",
                self.format.dxgi(),
                self.source_path
            )
        })?;
        let bytes_per_element = self.format.bytes_per_element().ok_or_else(|| {
            format!(
                "unsupported TLOU2 texture DXGI={} path='{}'",
                self.format.dxgi(),
                self.source_path
            )
        })?;
        let element_width = self.width.div_ceil(block_extent) as usize;
        let element_height = self.height.div_ceil(block_extent) as usize;
        let padded_width = element_width.div_ceil(8) * 8;
        let padded_height = element_height.div_ceil(8) * 8;
        let tiled_len = padded_width
            .checked_mul(padded_height)
            .and_then(|v| v.checked_mul(bytes_per_element))
            .ok_or("TLOU2 base texture allocation overflow")?;
        if tiled_len > self.vram_size as usize {
            return Err(format!(
                "TLOU2 base texture allocation exceeds VRAM descriptor path='{}' base_bytes={} vram_size={}",
                self.source_path, tiled_len, self.vram_size
            ));
        }
        let source = pak.slice(self.absolute_data_offset, tiled_len)?;
        detile_1d_thin_non_displayable(source, element_width, element_height, bytes_per_element)
    }

    /// Decode the validated base level into RGBA8 for canonical mip generation. BC4 is treated as
    /// an authored scalar/opacity source: RGB stays white and the recovered scalar is written to A.
    pub fn base_rgba8(&self, pak: &PakFile) -> Result<Vec<u8>, String> {
        let linear = self.base_linear_bytes(pak)?;
        match self.format {
            ImportedTextureFormat::Bc1Unorm => newengine_texture_container::decode_bcn_to_rgba8(
                newengine_texture_container::PIXEL_FORMAT_BC1_RGBA_UNORM,
                self.width,
                self.height,
                &linear,
            )
            .map_err(|error| format!("BC1 decode failed path='{}': {error}", self.source_path)),
            ImportedTextureFormat::Bc1Srgb => newengine_texture_container::decode_bcn_to_rgba8(
                newengine_texture_container::PIXEL_FORMAT_BC1_RGBA_SRGB,
                self.width,
                self.height,
                &linear,
            )
            .map_err(|error| format!("BC1 decode failed path='{}': {error}", self.source_path)),
            ImportedTextureFormat::Bc4Unorm => {
                decode_bc4_alpha_rgba8(self.width, self.height, &linear)
            }
            _ => Err(format!(
                "RGBA8 decode is not validated for DXGI={} path='{}'",
                self.format.dxgi(),
                self.source_path
            )),
        }
    }
}

/// Parse all TLOU2 PC `VRAM_DESC` metadata without decoding pixels. This is intentionally separate
/// from canonical texture production so offline tooling can select only the required source assets.
pub fn decode_vram_textures(pak: &PakFile) -> Result<Vec<ImportedVramTexture>, String> {
    let vram_base = pak.vram_data_base()?;
    let mut out = Vec::new();
    for resource in pak
        .resources()
        .iter()
        .filter(|resource| resource.kind == "VRAM_DESC")
    {
        let payload = pak.resource_payload(resource)?;
        let pak_offset = pak.read_u32(payload + 0x08)?;
        let vram_size = pak.read_u32(payload + 0x10)?;
        let source_hash = pak.read_u64(payload + 0x18)?;
        let texture_type = pak.read_u32(payload + 0x24)?;
        let format = ImportedTextureFormat::from_dxgi(pak.read_u32(payload + 0x28)?);
        let mip_count = pak.read_u32(payload + 0x30)?;
        let width = pak.read_u32(payload + 0x34)?;
        let height = pak.read_u32(payload + 0x38)?;
        let stream_flags = pak.read_u64(payload + 0x40)?;
        let source_path = pak
            .resolve_pointer(payload + 0x48)?
            .ok_or_else(|| format!("VRAM_DESC has no source path pointer at 0x{payload:x}"))
            .and_then(|pointer| pak.string_at(pointer))?;
        if width == 0 || height == 0 || mip_count == 0 {
            return Err(format!(
                "invalid VRAM_DESC extent/mips path='{source_path}' width={width} height={height} mips={mip_count}"
            ));
        }
        let absolute_data_offset = vram_base
            .checked_add(pak_offset as usize)
            .ok_or("VRAM_DESC data offset overflow")?;
        let end = absolute_data_offset
            .checked_add(vram_size as usize)
            .ok_or("VRAM_DESC data range overflow")?;
        if end > pak.bytes().len() {
            return Err(format!(
                "VRAM_DESC data outside package path='{source_path}' offset=0x{absolute_data_offset:x} bytes={vram_size} package_bytes={}",
                pak.bytes().len()
            ));
        }
        out.push(ImportedVramTexture {
            source_path,
            source_hash,
            pak_offset,
            vram_size,
            texture_type,
            format,
            mip_count,
            width,
            height,
            stream_flags,
            absolute_data_offset,
        });
    }
    Ok(out)
}

fn detile_1d_thin_non_displayable(
    source: &[u8],
    width: usize,
    height: usize,
    bytes_per_element: usize,
) -> Result<Vec<u8>, String> {
    if bytes_per_element != 8 {
        return Err(format!(
            "validated TLOU2 1D-thin detiler currently accepts 64-bit elements only, got {} bits",
            bytes_per_element * 8
        ));
    }
    let padded_width = width.div_ceil(8) * 8;
    let padded_height = height.div_ceil(8) * 8;
    let required = padded_width
        .checked_mul(padded_height)
        .and_then(|v| v.checked_mul(bytes_per_element))
        .ok_or("1D-thin source size overflow")?;
    if source.len() < required {
        return Err(format!(
            "1D-thin source is truncated bytes={} required={required}",
            source.len()
        ));
    }
    let tiles_per_row = padded_width / 8;
    let mut out = vec![0u8; width * height * bytes_per_element];
    for y in 0..height {
        for x in 0..width {
            let tile = (y / 8) * tiles_per_row + (x / 8);
            let within = morton_8x8(x & 7, y & 7);
            let source_offset = (tile * 64 + within) * bytes_per_element;
            let target_offset = (y * width + x) * bytes_per_element;
            out[target_offset..target_offset + bytes_per_element]
                .copy_from_slice(&source[source_offset..source_offset + bytes_per_element]);
        }
    }
    Ok(out)
}

#[inline]
fn morton_8x8(x: usize, y: usize) -> usize {
    ((x & 1) << 0)
        | ((y & 1) << 1)
        | (((x >> 1) & 1) << 2)
        | (((y >> 1) & 1) << 3)
        | (((x >> 2) & 1) << 4)
        | (((y >> 2) & 1) << 5)
}

fn decode_bc4_alpha_rgba8(width: u32, height: u32, blocks: &[u8]) -> Result<Vec<u8>, String> {
    let block_width = width.div_ceil(4) as usize;
    let block_height = height.div_ceil(4) as usize;
    let expected = block_width
        .checked_mul(block_height)
        .and_then(|value| value.checked_mul(8))
        .ok_or("BC4 byte size overflow")?;
    if blocks.len() != expected {
        return Err(format!(
            "BC4 payload bytes={} expected={expected}",
            blocks.len()
        ));
    }
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for block_y in 0..block_height {
        for block_x in 0..block_width {
            let at = (block_y * block_width + block_x) * 8;
            let block = &blocks[at..at + 8];
            let a0 = block[0];
            let a1 = block[1];
            let mut palette = [0u8; 8];
            palette[0] = a0;
            palette[1] = a1;
            if a0 > a1 {
                for i in 1..=6usize {
                    palette[i + 1] =
                        (((7 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 7) as u8;
                }
            } else {
                for i in 1..=4usize {
                    palette[i + 1] =
                        (((5 - i) as u16 * a0 as u16 + i as u16 * a1 as u16) / 5) as u8;
                }
                palette[6] = 0;
                palette[7] = 255;
            }
            let mut indices = 0u64;
            for (byte_index, byte) in block[2..8].iter().enumerate() {
                indices |= (*byte as u64) << (byte_index * 8);
            }
            for pixel_y in 0..4usize {
                for pixel_x in 0..4usize {
                    let x = block_x * 4 + pixel_x;
                    let y = block_y * 4 + pixel_y;
                    if x >= width as usize || y >= height as usize {
                        continue;
                    }
                    let pixel = pixel_y * 4 + pixel_x;
                    let alpha = palette[((indices >> (pixel * 3)) & 7) as usize];
                    let out = (y * width as usize + x) * 4;
                    rgba[out..out + 4].copy_from_slice(&[255, 255, 255, alpha]);
                }
            }
        }
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_microtile_covers_every_slot_once() {
        let mut slots = [false; 64];
        for y in 0..8 {
            for x in 0..8 {
                let slot = morton_8x8(x, y);
                assert!(slot < 64);
                assert!(!slots[slot]);
                slots[slot] = true;
            }
        }
        assert!(slots.into_iter().all(|value| value));
    }

    #[test]
    fn detile_non_displayable_roundtrips_known_microtile_coordinates() {
        let mut tiled = vec![0u8; 64 * 8];
        for y in 0..8usize {
            for x in 0..8usize {
                let slot = morton_8x8(x, y);
                let value = (y * 8 + x) as u64;
                tiled[slot * 8..slot * 8 + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        let linear = detile_1d_thin_non_displayable(&tiled, 8, 8, 8).expect("detile");
        for index in 0..64usize {
            let at = index * 8;
            assert_eq!(
                u64::from_le_bytes(linear[at..at + 8].try_into().unwrap()),
                index as u64
            );
        }
    }

    #[test]
    fn unvalidated_128_bit_formats_are_rejected_explicitly() {
        assert!(!ImportedTextureFormat::Bc5Unorm.validated_1d_thin_detile());
        assert!(!ImportedTextureFormat::Bc7Srgb.validated_1d_thin_detile());
        assert!(ImportedTextureFormat::Bc1Srgb.validated_1d_thin_detile());
        assert!(ImportedTextureFormat::Bc4Unorm.validated_1d_thin_detile());
    }
}
