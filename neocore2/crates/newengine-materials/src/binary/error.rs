/// Result alias for binary material format operations.
pub type MaterialBinaryResult<T> = Result<T, MaterialBinaryError>;

/// Errors produced by the `.nemat` binary material codec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialBinaryError {
    /// The input buffer ended before the expected number of bytes was read.
    UnexpectedEof,
    /// The file magic does not match [`super::MATERIAL_BINARY_MAGIC`].
    InvalidMagic,
    /// The binary container version is not supported by this decoder.
    UnsupportedVersion { found: u16 },
    /// The container header is malformed.
    InvalidHeader,
    /// A UTF-8 string field contains invalid bytes.
    InvalidUtf8,
    /// JSON input could not be deserialized into a material descriptor.
    InvalidJson,
    /// A material descriptor could not be serialized back to JSON.
    JsonSerializeFailed,
    /// A named material exceeds the maximum storable UTF-8 byte length.
    NameTooLong { len: usize, max: usize },
    /// A serialized payload exceeds the representable on-disk size.
    PayloadTooLarge { size: usize, max: usize },
    /// A standalone descriptor payload has an unexpected size.
    InvalidDescriptorSize { found: usize, expected: usize },
    /// A numeric enum discriminant could not be mapped to a known value.
    InvalidEnumValue { field: &'static str, value: u8 },
}

impl core::fmt::Display for MaterialBinaryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MaterialBinaryError::UnexpectedEof => write!(f, "unexpected EOF"),
            MaterialBinaryError::InvalidMagic => write!(f, "invalid magic"),
            MaterialBinaryError::UnsupportedVersion { found } => {
                write!(f, "unsupported version: {found}")
            }
            MaterialBinaryError::InvalidHeader => write!(f, "invalid header"),
            MaterialBinaryError::InvalidUtf8 => write!(f, "invalid utf8"),
            MaterialBinaryError::InvalidJson => write!(f, "invalid json"),
            MaterialBinaryError::JsonSerializeFailed => write!(f, "json serialize failed"),
            MaterialBinaryError::NameTooLong { len, max } => {
                write!(f, "material name too long: len={len}, max={max}")
            }
            MaterialBinaryError::PayloadTooLarge { size, max } => {
                write!(f, "payload too large: size={size}, max={max}")
            }
            MaterialBinaryError::InvalidDescriptorSize { found, expected } => {
                write!(f, "invalid descriptor size: found={found}, expected={expected}")
            }
            MaterialBinaryError::InvalidEnumValue { field, value } => {
                write!(f, "invalid enum value: {field}={value}")
            }
        }
    }
}

impl std::error::Error for MaterialBinaryError {}
