use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextTimestampStyle {
    #[default]
    Short,
    Long,
    FileName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextConversionInput {
    TextToHtml {
        text: String,
    },
    StripNonRenderable {
        text: String,
    },
    HumanInteger {
        value: i64,
        prefix: String,
        group_digits: bool,
        separator: char,
    },
    DurationShort {
        milliseconds: u64,
        round_milliseconds: bool,
    },
    DurationLong {
        milliseconds: u64,
    },
    Timestamp {
        unix_time_ms: i64,
        style: TextTimestampStyle,
        shorthand_month: bool,
        shorthand_day: bool,
    },
}

impl Default for TextConversionInput {
    fn default() -> Self {
        Self::StripNonRenderable {
            text: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextConversionRequest {
    pub input: TextConversionInput,
    pub locale: String,
    pub max_chars: usize,
}

impl Default for TextConversionRequest {
    fn default() -> Self {
        Self {
            input: TextConversionInput::default(),
            locale: "und".to_owned(),
            max_chars: 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextConversionResponse {
    pub version: u32,
    pub output: String,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
}

impl Default for TextConversionResponse {
    fn default() -> Self {
        Self {
            version: 1,
            output: String::new(),
            truncated: false,
            diagnostics: Vec::new(),
        }
    }
}

pub fn format_human_integer(value: i64, separator: char) -> String {
    let negative = value.is_negative();
    let digits = value.unsigned_abs().to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3 + usize::from(negative));
    if negative {
        output.push('-');
    }
    for (index, byte) in digits.bytes().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(separator);
        }
        output.push(char::from(byte));
    }
    output
}

pub fn format_milliseconds_short(milliseconds: u64, round_milliseconds: bool) -> String {
    let total_seconds = if round_milliseconds {
        milliseconds.saturating_add(500) / 1000
    } else {
        milliseconds / 1000
    };
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

pub fn format_milliseconds_long(milliseconds: u64) -> String {
    let total_seconds = milliseconds / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds / 60) % 60;
    let seconds = total_seconds % 60;
    let millis = milliseconds % 1000;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
    } else {
        format!("{minutes:02}:{seconds:02}.{millis:03}")
    }
}
