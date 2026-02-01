use crate::id::AssetId;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum AssetEvent {
    Ready {
        id: AssetId,
        type_name: &'static str,
    },
    Failed {
        id: AssetId,
        type_name: &'static str,
        error: Arc<str>,
    },
}