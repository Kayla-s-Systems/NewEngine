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
    pub const fn validated_pitched_linearization(self) -> bool {
        // TLOU2 PC VRAM_DESC type=1 64-bit block-compressed resources use linear block rows with
        // a 256-byte physical row pitch. Earlier tooling incorrectly treated these resources as
        // Morton 8x8 microtiles; corpus validation on character and VFX textures showed that the
        // apparent "tiles" were actually row padding. 128-bit BC5/BC7 remain intentionally
        // rejected until their physical row/storage contract is validated independently.
        matches!(
            self,
            Self::Bc1Unorm | Self::Bc1Srgb | Self::Bc4Unorm | Self::Bc5Unorm
        )
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
        if !self.format.validated_pitched_linearization() {
            return Err(format!(
                "TLOU2 pitched base linearization is not validated for DXGI={} path='{}'",
                self.format.dxgi(),
                self.source_path
            ));
        }
        if self.texture_type != 1 || self.stream_flags & 0x2 == 0 {
            return Err(format!(
                "TLOU2 VRAM layout is not validated for type={} stream_flags=0x{:x} path='{}'",
                self.texture_type, self.stream_flags, self.source_path
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
        let row_bytes = element_width
            .checked_mul(bytes_per_element)
            .ok_or("TLOU2 base row size overflow")?;
        let row_pitch = align_up(row_bytes, 256)?;
        let physical_base_len = row_pitch
            .checked_mul(element_height)
            .ok_or("TLOU2 base texture allocation overflow")?;
        if physical_base_len > self.vram_size as usize {
            return Err(format!(
                "TLOU2 base texture allocation exceeds VRAM descriptor path='{}' base_bytes={} vram_size={}",
                self.source_path, physical_base_len, self.vram_size
            ));
        }
        let source = pak.slice(self.absolute_data_offset, physical_base_len)?;
        linearize_pitched_rows(
            source,
            element_width,
            element_height,
            bytes_per_element,
            256,
        )
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
            ImportedTextureFormat::Bc5Unorm => newengine_texture_container::decode_bcn_to_rgba8(
                newengine_texture_container::PIXEL_FORMAT_BC5_RG_UNORM,
                self.width,
                self.height,
                &linear,
            )
            .map_err(|error| format!("BC5 decode failed path='{}': {error}", self.source_path)),
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

fn align_up(value: usize, alignment: usize) -> Result<usize, String> {
    if alignment == 0 {
        return Err("alignment must be non-zero".to_owned());
    }
    value
        .checked_add(alignment - 1)
        .map(|v| (v / alignment) * alignment)
        .ok_or_else(|| "alignment overflow".to_owned())
}

fn linearize_pitched_rows(
    source: &[u8],
    width: usize,
    height: usize,
    bytes_per_element: usize,
    pitch_alignment: usize,
) -> Result<Vec<u8>, String> {
    let row_bytes = width
        .checked_mul(bytes_per_element)
        .ok_or("pitched row byte size overflow")?;
    let row_pitch = align_up(row_bytes, pitch_alignment)?;
    let required = row_pitch
        .checked_mul(height)
        .ok_or("pitched source size overflow")?;
    if source.len() < required {
        return Err(format!(
            "pitched source is truncated bytes={} required={required}",
            source.len()
        ));
    }
    let output_len = row_bytes
        .checked_mul(height)
        .ok_or("linear output size overflow")?;
    let mut out = vec![0u8; output_len];
    for y in 0..height {
        let source_start = y * row_pitch;
        let target_start = y * row_bytes;
        out[target_start..target_start + row_bytes]
            .copy_from_slice(&source[source_start..source_start + row_bytes]);
    }
    Ok(out)
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
    fn pitched_rows_strip_physical_padding_without_touching_payload() {
        let width = 16usize;
        let height = 4usize;
        let bytes_per_element = 8usize;
        let row_bytes = width * bytes_per_element;
        let row_pitch = 256usize;
        let mut source = vec![0xCD; row_pitch * height];
        for y in 0..height {
            for x in 0..width {
                let value = (y * width + x) as u64;
                let at = y * row_pitch + x * bytes_per_element;
                source[at..at + 8].copy_from_slice(&value.to_le_bytes());
            }
        }
        let linear = linearize_pitched_rows(&source, width, height, bytes_per_element, 256)
            .expect("linearize pitched rows");
        assert_eq!(linear.len(), row_bytes * height);
        for index in 0..width * height {
            let at = index * 8;
            assert_eq!(
                u64::from_le_bytes(linear[at..at + 8].try_into().unwrap()),
                index as u64
            );
        }
    }

    #[test]
    fn pitched_row_alignment_matches_tlou2_bc1_tail_layout() {
        assert_eq!(align_up(16 * 8, 256).unwrap(), 256);
        assert_eq!(align_up(32 * 8, 256).unwrap(), 256);
        assert_eq!(align_up(33 * 8, 256).unwrap(), 512);
    }

    #[test]
    fn unvalidated_128_bit_formats_are_rejected_explicitly() {
        assert!(ImportedTextureFormat::Bc5Unorm.validated_pitched_linearization());
        assert!(!ImportedTextureFormat::Bc7Srgb.validated_pitched_linearization());
        assert!(ImportedTextureFormat::Bc1Srgb.validated_pitched_linearization());
        assert!(ImportedTextureFormat::Bc4Unorm.validated_pitched_linearization());
    }
}
