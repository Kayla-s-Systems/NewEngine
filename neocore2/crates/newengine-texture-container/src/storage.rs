use crate::error::{Result, TextureContainerError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureBuildOptions;

impl TextureBuildOptions {
    #[inline]
    pub const fn uncompressed() -> Self {
        Self
    }

    #[inline]
    pub const fn raw_runtime() -> Self {
        Self
    }
}

#[inline]
pub(crate) fn validate_raw_header_flags(flags: u16) -> Result<()> {
    if flags == 0 {
        Ok(())
    } else {
        Err(TextureContainerError::CompressedPayloadUnsupported(flags))
    }
}

#[inline]
pub(crate) fn store_raw_data(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
