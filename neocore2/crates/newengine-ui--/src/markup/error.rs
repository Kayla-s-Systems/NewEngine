#![forbid(unsafe_op_in_unsafe_fn)]

#[derive(Debug)]
pub enum UiMarkupError {
    Enqueue(String),
    Timeout { path: String },
    Failed(String),
    BlobMissing,
    TextRead(String),
    XmlParse(String),
    Invalid(String),
}

impl std::fmt::Display for UiMarkupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiMarkupError::Enqueue(e) => write!(f, "ui: load enqueue failed: {e}"),
            UiMarkupError::Timeout { path } => write!(f, "ui: timeout while loading '{path}'"),
            UiMarkupError::Failed(msg) => write!(f, "ui: asset failed: {msg}"),
            UiMarkupError::BlobMissing => write!(f, "ui: asset Ready but blob missing"),
            UiMarkupError::TextRead(e) => write!(f, "ui: TextReader failed: {e}"),
            UiMarkupError::XmlParse(e) => write!(f, "ui: xml parse failed: {e}"),
            UiMarkupError::Invalid(e) => write!(f, "ui: markup invalid: {e}"),
        }
    }
}

impl std::error::Error for UiMarkupError {}