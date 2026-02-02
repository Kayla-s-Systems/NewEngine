use crate::id::AssetId;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum AssetEvent {
    Ready {
        id: AssetId,
        type_id: Arc<str>,
        format: Arc<str>,
    },
    Failed {
        id: AssetId,
        type_id: Arc<str>,
        error: Arc<str>,
    },
}