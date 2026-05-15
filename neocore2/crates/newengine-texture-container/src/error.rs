use crate::HEADER_LEN;

#[derive(Debug, thiserror::Error)]
pub enum TextureContainerError {
    #[error("neytd: input is shorter than header: bytes={0} expected={HEADER_LEN}")]
    ShortHeader(usize),
    #[error("neytd: bad magic")]
    BadMagic,
    #[error("neytd: unsupported version {0}")]
    UnsupportedVersion(u16),
    #[error("neytd: unsupported storage/header flags: flags=0x{0:04x}")]
    CompressedPayloadUnsupported(u16),
    #[error("neytd: compression error: {0}")]
    CompressionFailed(String),
    #[error("neytd: invalid range {what}: offset={offset} len={len} total={total}")]
    InvalidRange { what: &'static str, offset: u64, len: u64, total: usize },
    #[error("neytd: invalid binary directory: {0}")]
    InvalidDirectory(&'static str),
    #[error("neytd: binary directory is too large: {0}")]
    DirectoryTooLarge(&'static str),
    #[error("neytd: texture name is too long for binary directory: {0}")]
    NameTooLong(String),
    #[error("neytd: entry_count mismatch header={header} directory={directory}")]
    EntryCountMismatch { header: u32, directory: usize },
    #[error("neytd: duplicate texture entry '{0}'")]
    DuplicateEntry(String),
    #[error("neytd: missing texture entry '{0}'")]
    MissingEntry(String),
    #[error("neytd: empty texture dictionary")]
    EmptyDictionary,
    #[error("neytd: invalid texture extent for '{name}': {width}x{height}")]
    InvalidExtent { name: String, width: u32, height: u32 },
    #[error("neytd: invalid mip chain for '{0}'")]
    InvalidMipChain(String),
    #[error("neytd: payload size mismatch for '{name}' mip={mip} bytes={bytes} expected={expected}")]
    PayloadSizeMismatch { name: String, mip: u32, bytes: usize, expected: usize },
    #[error("neytd: invalid pixel format '{format}' for '{name}'")]
    InvalidFormat { name: String, format: String },
    #[error("neytd: invalid color space '{color_space}' for '{name}'")]
    InvalidColorSpace { name: String, color_space: String },
}

pub type Result<T> = std::result::Result<T, TextureContainerError>;
