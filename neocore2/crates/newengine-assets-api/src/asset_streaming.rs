use serde::{Deserialize, Serialize};

pub const ASSET_STREAMING_SCHEMA_V1: &str = "newengine.assets.streaming.v1";

/// Semantic reason for keeping an asset non-evictable.
///
/// Pins are reference-counted by `(class, owner)` so independent mission,
/// cutscene, interior and script systems cannot accidentally unpin each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssetStreamingPinClassV1 {
    #[default]
    Runtime,
    Mission,
    Cutscene,
    Interior,
    Script,
    Editor,
    Manual,
}

impl AssetStreamingPinClassV1 {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Mission => "mission",
            Self::Cutscene => "cutscene",
            Self::Interior => "interior",
            Self::Script => "script",
            Self::Editor => "editor",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingRequestV1 {
    pub logical_path: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub pin: bool,
    #[serde(default)]
    pub pin_class: AssetStreamingPinClassV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingPinRequestV1 {
    pub logical_path: String,
    pub owner: String,
    #[serde(default)]
    pub class: AssetStreamingPinClassV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingTouchRequestV1 {
    pub logical_path: String,
    #[serde(default)]
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingCleanupRequestV1 {
    /// Desired maximum CPU decoded-blob footprint after cleanup. `None` uses the
    /// provider's configured/default budget.
    #[serde(default)]
    pub target_bytes: Option<u64>,
    /// Maximum number of blobs to evict in one call. Zero uses provider default.
    #[serde(default)]
    pub max_evictions: usize,
    /// Manual flush may evict every unpinned blob; required/reference pins still win.
    #[serde(default)]
    pub aggressive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingCleanupResponseV1 {
    pub ok: bool,
    pub before_bytes: u64,
    pub after_bytes: u64,
    pub freed_bytes: u64,
    pub evicted_assets: usize,
    pub pinned_assets: usize,
    pub target_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetStreamingStatsV1 {
    pub schema: String,
    pub resident_assets: usize,
    pub resident_bytes: u64,
    pub tracked_assets: usize,
    pub pinned_assets: usize,
    pub pin_references: u64,
    pub pending_requests: usize,
    pub cleanup_calls: u64,
    pub total_evictions: u64,
    pub total_freed_bytes: u64,
    pub compaction_calls: u64,
    pub budget_bytes: u64,
}
