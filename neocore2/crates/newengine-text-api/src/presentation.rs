use serde::{Deserialize, Serialize};

use crate::{TextColor, TextFontStyle, TextGlyphRun};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextAlignment {
    Center,
    #[default]
    Left,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextScissorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for TextScissorRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextOutlineStyle {
    pub enabled: bool,
    pub cutout: bool,
    pub width_px: f32,
    pub color: TextColor,
}

impl Default for TextOutlineStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            cutout: false,
            width_px: 0.0,
            color: TextColor([0, 0, 0, 255]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextBackgroundStyle {
    pub enabled: bool,
    pub outline: bool,
    pub color: TextColor,
    pub padding_px: [f32; 4],
}

impl Default for TextBackgroundStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            outline: false,
            color: TextColor([0, 0, 0, 0]),
            padding_px: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Provider-neutral presentation state corresponding to the classic mutable
/// text-layout object. Renderers still receive shaped glyphs, not raw strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLayoutStyle {
    pub scale: [f32; 2],
    pub leading_px: f32,
    pub font_style: TextFontStyle,
    pub alignment: TextAlignment,
    pub color: TextColor,
    pub use_inline_colors: bool,
    pub wrap_range_px: [f32; 2],
    pub scissor: Option<TextScissorRect>,
    pub outline: TextOutlineStyle,
    pub drop_shadow: bool,
    pub background: TextBackgroundStyle,
    pub render_upwards: bool,
    pub input_icon_scale: f32,
    pub adjust_for_non_widescreen: bool,
}

impl Default for TextLayoutStyle {
    fn default() -> Self {
        Self {
            scale: [1.0, 1.0],
            leading_px: 0.0,
            font_style: TextFontStyle::Standard,
            alignment: TextAlignment::Left,
            color: TextColor::default(),
            use_inline_colors: true,
            wrap_range_px: [0.0, 0.0],
            scissor: None,
            outline: TextOutlineStyle::default(),
            drop_shadow: false,
            background: TextBackgroundStyle::default(),
            render_upwards: false,
            input_icon_scale: 1.0,
            adjust_for_non_widescreen: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextDrawRun {
    pub position_px: [f32; 2],
    pub size_px: [f32; 2],
    pub source_text: String,
    pub glyph_runs: Vec<TextGlyphRun>,
    pub style: TextLayoutStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextDrawBatch {
    pub version: u32,
    pub frame_index: u64,
    pub clear_previous: bool,
    pub runs: Vec<TextDrawRun>,
    pub diagnostics: Vec<String>,
}

impl Default for TextDrawBatch {
    fn default() -> Self {
        Self {
            version: 1,
            frame_index: 0,
            clear_previous: false,
            runs: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
