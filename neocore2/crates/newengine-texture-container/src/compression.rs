use crate::error::{Result, TextureContainerError};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::{Read, Write};

pub const FLAG_DATA_DEFLATE: u16 = 0x0001;
pub const SUPPORTED_HEADER_FLAGS: u16 = FLAG_DATA_DEFLATE;

pub const PAYLOAD_COMPRESSION_NONE: &str = "none";
pub const PAYLOAD_COMPRESSION_DEFLATE: &str = "deflate";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TexturePayloadCompression {
    None,
    Deflate,
}

impl Default for TexturePayloadCompression {
    fn default() -> Self {
        Self::None
    }
}

impl TexturePayloadCompression {
    #[inline]
    pub const fn header_flags(self) -> u16 {
        match self {
            Self::None => 0,
            Self::Deflate => FLAG_DATA_DEFLATE,
        }
    }

    #[inline]
    pub const fn manifest_value(self) -> &'static str {
        match self {
            Self::None => PAYLOAD_COMPRESSION_NONE,
            Self::Deflate => PAYLOAD_COMPRESSION_DEFLATE,
        }
    }

    #[inline]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Deflate => "deflate",
        }
    }

    pub fn from_header_flags(flags: u16) -> Result<Self> {
        let unsupported = flags & !SUPPORTED_HEADER_FLAGS;
        if unsupported != 0 {
            return Err(TextureContainerError::UnsupportedFlags(unsupported));
        }
        if flags & FLAG_DATA_DEFLATE != 0 {
            Ok(Self::Deflate)
        } else {
            Ok(Self::None)
        }
    }

    pub fn from_manifest_value(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | PAYLOAD_COMPRESSION_NONE => Ok(Self::None),
            PAYLOAD_COMPRESSION_DEFLATE => Ok(Self::Deflate),
            other => Err(TextureContainerError::UnsupportedCompression(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureBuildOptions {
    pub payload_compression: TexturePayloadCompression,
    pub deflate_level: u32,
}

impl Default for TextureBuildOptions {
    fn default() -> Self {
        Self {
            payload_compression: TexturePayloadCompression::None,
            deflate_level: 6,
        }
    }
}

impl TextureBuildOptions {
    #[inline]
    pub const fn uncompressed() -> Self {
        Self {
            payload_compression: TexturePayloadCompression::None,
            deflate_level: 6,
        }
    }

    #[inline]
    pub const fn deflate() -> Self {
        Self {
            payload_compression: TexturePayloadCompression::Deflate,
            deflate_level: 6,
        }
    }
}

pub(crate) fn compress_data(compression: TexturePayloadCompression, data: &[u8], level: u32) -> Result<Vec<u8>> {
    match compression {
        TexturePayloadCompression::None => Ok(data.to_vec()),
        TexturePayloadCompression::Deflate => deflate_encode(data, level),
    }
}

pub(crate) fn decompress_data(
    compression: TexturePayloadCompression,
    data: &[u8],
    expected_uncompressed_len: u64,
) -> Result<Vec<u8>> {
    match compression {
        TexturePayloadCompression::None => Ok(data.to_vec()),
        TexturePayloadCompression::Deflate => deflate_decode(data, expected_uncompressed_len),
    }
}

fn deflate_encode(data: &[u8], level: u32) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(level.min(9)));
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

fn deflate_decode(data: &[u8], expected_uncompressed_len: u64) -> Result<Vec<u8>> {
    if expected_uncompressed_len == 0 {
        return Err(TextureContainerError::InvalidUncompressedDataLen(expected_uncompressed_len));
    }
    let capacity = usize::try_from(expected_uncompressed_len)
        .map_err(|_| TextureContainerError::InvalidUncompressedDataLen(expected_uncompressed_len))?;
    let read_limit = expected_uncompressed_len
        .checked_add(1)
        .ok_or(TextureContainerError::InvalidUncompressedDataLen(expected_uncompressed_len))?;
    let decoder = DeflateDecoder::new(data);
    let mut limited = decoder.take(read_limit);
    let mut out = Vec::with_capacity(capacity);
    limited.read_to_end(&mut out)?;
    if out.len() != capacity {
        return Err(TextureContainerError::DecompressedSizeMismatch {
            bytes: out.len(),
            expected: capacity,
        });
    }
    Ok(out)
}
