use serde::{Deserialize, Serialize};

use crate::{TextDirection, TextFontManifest, TextGlyphRun, TextImeComposition, TextSelectionRect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextWrapMode {
    None,
    #[default]
    Word,
    Character,
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextOverflowMode {
    #[default]
    Clip,
    Ellipsis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLayoutParagraphRequest {
    pub text: String,
    pub locale: String,
    pub font_stack: Vec<String>,
    pub size_px: f32,
    pub line_height_px: f32,
    pub max_width_px: f32,
    pub max_lines: usize,
    pub direction: TextDirection,
    pub wrap: TextWrapMode,
    pub overflow: TextOverflowMode,
    pub ime: Option<TextImeComposition>,
}

impl Default for TextLayoutParagraphRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            locale: "und".to_owned(),
            font_stack: TextFontManifest::default().fallback_stack,
            size_px: 16.0,
            line_height_px: 20.0,
            max_width_px: 0.0,
            max_lines: 0,
            direction: TextDirection::Auto,
            wrap: TextWrapMode::Word,
            overflow: TextOverflowMode::Clip,
            ime: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLineBox {
    pub text_range: [usize; 2],
    pub glyph_range: [usize; 2],
    pub rect_px: [f32; 4],
    pub baseline_y_px: f32,
    pub ellipsized: bool,
}

impl Default for TextLineBox {
    fn default() -> Self {
        Self {
            text_range: [0, 0],
            glyph_range: [0, 0],
            rect_px: [0.0, 0.0, 0.0, 0.0],
            baseline_y_px: 0.0,
            ellipsized: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextLayoutParagraphResponse {
    pub version: u32,
    pub lines: Vec<TextLineBox>,
    pub glyph_runs: Vec<TextGlyphRun>,
    pub caret_positions_px: Vec<[f32; 2]>,
    pub selection_rects: Vec<TextSelectionRect>,
    pub size_px: [f32; 2],
    pub diagnostics: Vec<String>,
}

impl Default for TextLayoutParagraphResponse {
    fn default() -> Self {
        Self {
            version: 1,
            lines: Vec::new(),
            glyph_runs: Vec::new(),
            caret_positions_px: Vec::new(),
            selection_rects: Vec::new(),
            size_px: [0.0, 0.0],
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMeasureTextRequest {
    pub text: String,
    pub locale: String,
    pub font_stack: Vec<String>,
    pub size_px: f32,
    pub max_width_px: f32,
    pub wrap: TextWrapMode,
}

impl Default for TextMeasureTextRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            locale: "und".to_owned(),
            font_stack: TextFontManifest::default().fallback_stack,
            size_px: 16.0,
            max_width_px: 0.0,
            wrap: TextWrapMode::Word,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextMeasureTextResponse {
    pub version: u32,
    pub size_px: [f32; 2],
    pub baseline_y_px: f32,
    pub line_count: usize,
    pub glyph_count: u32,
    pub diagnostics: Vec<String>,
}

impl Default for TextMeasureTextResponse {
    fn default() -> Self {
        Self {
            version: 1,
            size_px: [0.0, 0.0],
            baseline_y_px: 0.0,
            line_count: 0,
            glyph_count: 0,
            diagnostics: Vec::new(),
        }
    }
}
