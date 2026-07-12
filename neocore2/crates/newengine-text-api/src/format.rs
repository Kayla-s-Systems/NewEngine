use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::TextLookupKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TextColor(pub [u8; 4]);

impl Default for TextColor {
    fn default() -> Self {
        Self([255, 255, 255, 255])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextBufferKind {
    #[default]
    Standard,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextOverlayColor {
    #[default]
    White,
    Pink,
    RedLight,
    Red,
    RedDark,
    OrangeLight,
    Orange,
    OrangeDark,
    YellowLight,
    Yellow,
    YellowDark,
    GreenLight,
    Green,
    GreenDark,
    BlueLight,
    Blue,
    BlueDark,
    Purple,
    PurpleDark,
    GreyLight,
    Grey,
    GreyDark,
    Black,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextFormatValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Literal(String),
    Localized(TextLookupKey),
}

impl Default for TextFormatValue {
    fn default() -> Self {
        Self::Literal(String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextNumberArgument {
    pub value: f64,
    pub decimal_places: Option<u8>,
    pub color: Option<TextColor>,
}

impl Default for TextNumberArgument {
    fn default() -> Self {
        Self {
            value: 0.0,
            decimal_places: None,
            color: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextSubstringSource {
    Localized(TextLookupKey),
    Literal(String),
    External(String),
}

impl Default for TextSubstringSource {
    fn default() -> Self {
        Self::Literal(String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextSubstringArgument {
    pub source: TextSubstringSource,
    pub color: Option<TextColor>,
    pub persistent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextFormatArguments {
    pub numbers: Vec<TextNumberArgument>,
    pub substrings: Vec<TextSubstringArgument>,
    pub named: BTreeMap<String, TextFormatValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextInputIconKind {
    #[default]
    Icon,
    KeyboardKey,
    MouseButton,
    MouseAxis,
    GamepadButton,
    GamepadAxis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextInputIconPolicy {
    pub device_id: String,
    pub mapping_slot: u8,
    pub allow_confirm_cancel_swap: bool,
    pub allow_fallback: bool,
    pub correct_button_order: bool,
}

impl Default for TextInputIconPolicy {
    fn default() -> Self {
        Self {
            device_id: "any".to_owned(),
            mapping_slot: 0,
            allow_confirm_cancel_swap: true,
            allow_fallback: true,
            correct_button_order: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextInputIconDescriptor {
    pub id: String,
    pub kind: TextInputIconKind,
    pub display_text: String,
    pub texture_ref: String,
    pub atlas_rect_px: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextTokenKind {
    #[default]
    Literal,
    Number,
    Substring,
    NamedValue,
    Color,
    InputIcon,
    RadarIcon,
    NewLine,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextTokenDescriptor {
    pub raw: String,
    pub kind: TextTokenKind,
    pub index: Option<usize>,
    pub name: String,
    pub byte_range: [usize; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextRichSpanKind {
    #[default]
    Text,
    Color,
    InputIcon,
    RadarIcon,
    LineBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextRichSpan {
    pub kind: TextRichSpanKind,
    pub text: String,
    pub color: Option<TextColor>,
    pub icon: Option<TextInputIconDescriptor>,
    pub source_range: [usize; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFormatRequest {
    pub template: String,
    pub locale: String,
    pub arguments: TextFormatArguments,
    pub buffer_kind: TextBufferKind,
    pub parse_color_tokens: bool,
    pub strip_color_tokens: bool,
    pub input_icons: TextInputIconPolicy,
    pub max_chars: usize,
}

impl Default for TextFormatRequest {
    fn default() -> Self {
        Self {
            template: String::new(),
            locale: "und".to_owned(),
            arguments: TextFormatArguments::default(),
            buffer_kind: TextBufferKind::Standard,
            parse_color_tokens: true,
            strip_color_tokens: false,
            input_icons: TextInputIconPolicy::default(),
            max_chars: 1200,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFormatResponse {
    pub version: u32,
    pub text: String,
    pub spans: Vec<TextRichSpan>,
    pub tokens: Vec<TextTokenDescriptor>,
    pub expected_numbers: usize,
    pub expected_substrings: usize,
    pub used_html: bool,
    pub truncated: bool,
    pub diagnostics: Vec<String>,
}

impl Default for TextFormatResponse {
    fn default() -> Self {
        Self {
            version: 1,
            text: String::new(),
            spans: Vec::new(),
            tokens: Vec::new(),
            expected_numbers: 0,
            expected_substrings: 0,
            used_html: false,
            truncated: false,
            diagnostics: Vec::new(),
        }
    }
}

/// Returns the number of numeric and substring placeholders in a classic
/// `~token~` template. Supported forms include `~1~`, `~1_0~`, `~a~`, `~a_0~`.
pub fn expected_format_components(template: &str) -> (usize, usize) {
    let mut numbers = 0usize;
    let mut substrings = 0usize;
    let mut rest = template;
    while let Some(start) = rest.find('~') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('~') else {
            break;
        };
        let token = &rest[..end];
        rest = &rest[end + 1..];
        if is_number_token(token) {
            numbers += 1;
        } else if is_substring_token(token) {
            substrings += 1;
        }
    }
    (numbers, substrings)
}

/// Removes control tokens while retaining ordinary text. Unknown and malformed
/// trailing tokens are preserved because they may be user-authored literals.
pub fn filter_control_tokens(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('~') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('~') else {
            output.push_str(&rest[start..]);
            return output;
        };
        let token = &after[..end];
        if matches!(classify_control_token(token), TextTokenKind::Unknown) {
            output.push('~');
            output.push_str(token);
            output.push('~');
        }
        rest = &after[end + 1..];
    }
    output.push_str(rest);
    output
}

fn classify_control_token(token: &str) -> TextTokenKind {
    if is_number_token(token) {
        TextTokenKind::Number
    } else if is_substring_token(token) {
        TextTokenKind::Substring
    } else if token.eq_ignore_ascii_case("n") || token.eq_ignore_ascii_case("newline") {
        TextTokenKind::NewLine
    } else if has_ascii_prefix(token, b"col") {
        TextTokenKind::Color
    } else if has_ascii_prefix(token, b"pad") {
        TextTokenKind::InputIcon
    } else if has_ascii_prefix(token, b"blip") {
        TextTokenKind::RadarIcon
    } else {
        TextTokenKind::Unknown
    }
}

fn has_ascii_prefix(value: &str, prefix: &[u8]) -> bool {
    value
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

fn is_number_token(token: &str) -> bool {
    let bytes = token.as_bytes();
    matches!(bytes, [b'0'..=b'9']) || matches!(bytes, [b'0'..=b'9', b'_', b'0'..=b'9'])
}

fn is_substring_token(token: &str) -> bool {
    token.eq_ignore_ascii_case("a")
        || (token.len() == 3
            && token.as_bytes()[0].eq_ignore_ascii_case(&b'a')
            && token.as_bytes()[1] == b'_'
            && token.as_bytes()[2].is_ascii_digit())
}
