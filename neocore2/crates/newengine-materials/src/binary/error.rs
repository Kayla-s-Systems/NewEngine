pub type MaterialBinaryResult<T> = Result<T, MaterialBinaryError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialBinaryError {
    UnexpectedEof,
    InvalidMagic,
    UnsupportedVersion { found: u16 },
    InvalidHeader,
    InvalidUtf8,
    InvalidJson,
    JsonSerializeFailed,
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
            MaterialBinaryError::InvalidEnumValue { field, value } => {
                write!(f, "invalid enum value: {field}={value}")
            }
        }
    }
}

impl std::error::Error for MaterialBinaryError {}