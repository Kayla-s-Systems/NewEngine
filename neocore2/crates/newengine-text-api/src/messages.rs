use serde::{Deserialize, Serialize};

use crate::{TextColor, TextFormatArguments, TextLookupKey};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct TextMessageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextMessageChannel {
    #[default]
    Brief,
    Dialogue,
    Help,
    Mission,
    Subtitle,
    Saving,
    Loading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextMessageStyle {
    #[default]
    Normal,
    Taggable,
    Store,
    Freemode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextArrowOrientation {
    #[default]
    Normal,
    North,
    East,
    South,
    West,
    ForceReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextPreviousBriefOverride {
    #[default]
    None,
    Dialogue,
    Mission,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextMessageText {
    Localized(TextLookupKey),
    Literal(String),
}

impl Default for TextMessageText {
    fn default() -> Self {
        Self::Literal(String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageDescriptor {
    pub id: TextMessageId,
    pub channel: TextMessageChannel,
    pub text: TextMessageText,
    pub arguments: TextFormatArguments,
    pub text_block_id: String,
    pub duration_ms: u64,
    pub priority: i32,
    pub jump_queue: bool,
    pub add_to_history: bool,
    pub previous_brief_override: TextPreviousBriefOverride,
    pub display_when_subtitles_disabled: bool,
    pub use_underscore: bool,
    pub voice_name_hash: u32,
    pub mission_name_hash: u32,
    pub style: TextMessageStyle,
    pub arrow_orientation: TextArrowOrientation,
    pub screen_position: Option<[f32; 2]>,
    pub world_position: Option<[f32; 3]>,
    pub color: Option<TextColor>,
}

impl Default for TextMessageDescriptor {
    fn default() -> Self {
        Self {
            id: TextMessageId::default(),
            channel: TextMessageChannel::Brief,
            text: TextMessageText::default(),
            arguments: TextFormatArguments::default(),
            text_block_id: String::new(),
            duration_ms: 0,
            priority: 0,
            jump_queue: false,
            add_to_history: false,
            previous_brief_override: TextPreviousBriefOverride::None,
            display_when_subtitles_disabled: false,
            use_underscore: false,
            voice_name_hash: 0,
            mission_name_hash: 0,
            style: TextMessageStyle::Normal,
            arrow_orientation: TextArrowOrientation::Normal,
            screen_position: None,
            world_position: None,
            color: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextMessageState {
    #[default]
    Queued,
    Displaying,
    Fading,
    Completed,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextMessageEnqueueRequest {
    pub message: TextMessageDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageEnqueueResponse {
    pub version: u32,
    pub accepted: bool,
    pub id: TextMessageId,
    pub queue_position: usize,
    pub diagnostics: Vec<String>,
}

impl Default for TextMessageEnqueueResponse {
    fn default() -> Self {
        Self {
            version: 1,
            accepted: false,
            id: TextMessageId::default(),
            queue_position: 0,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextMessageDismissRequest {
    pub id: TextMessageId,
    pub channel: Option<TextMessageChannel>,
    pub clear_channel: bool,
    pub clear_history: bool,
    pub text_block_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageDismissResponse {
    pub version: u32,
    pub dismissed: usize,
    pub diagnostics: Vec<String>,
}

impl Default for TextMessageDismissResponse {
    fn default() -> Self {
        Self {
            version: 1,
            dismissed: 0,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageHistoryRequest {
    pub channel: Option<TextMessageChannel>,
    pub limit: usize,
    pub newest_first: bool,
}

impl Default for TextMessageHistoryRequest {
    fn default() -> Self {
        Self {
            channel: None,
            limit: 64,
            newest_first: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageRecord {
    pub message: TextMessageDescriptor,
    pub state: TextMessageState,
    pub rendered_text: String,
    pub enqueued_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
}

impl Default for TextMessageRecord {
    fn default() -> Self {
        Self {
            message: TextMessageDescriptor::default(),
            state: TextMessageState::Queued,
            rendered_text: String::new(),
            enqueued_at_ms: 0,
            started_at_ms: None,
            completed_at_ms: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMessageHistoryResponse {
    pub version: u32,
    pub messages: Vec<TextMessageRecord>,
    pub diagnostics: Vec<String>,
}

impl Default for TextMessageHistoryResponse {
    fn default() -> Self {
        Self {
            version: 1,
            messages: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
