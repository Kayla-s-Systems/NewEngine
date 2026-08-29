use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextImeComposition {
    pub active: bool,
    pub text: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub clauses: Vec<[usize; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextShapeRunRequest {
    pub text: String,
    pub locale: String,
    pub font_stack: Vec<String>,
    pub size_px: f32,
    pub direction: TextDirection,
    pub features: Vec<String>,
    pub ime: Option<TextImeComposition>,
}

impl Default for TextShapeRunRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            locale: "und".to_owned(),
            font_stack: vec![
                crate::TEXT_FONT_REF_EDITOR_SANS.to_owned(),
                "Segoe UI".to_owned(),
                "NotoSans".to_owned(),
            ],
            size_px: 16.0,
            direction: TextDirection::Auto,
            features: Vec::new(),
            ime: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextShapedGlyph {
    pub glyph_id: u32,
    pub codepoint: u32,
    pub cluster: u32,
    pub font_id: String,
    pub atlas_id: String,
    pub atlas_page: u32,
    pub atlas_rect_px: [f32; 4],
    pub x_advance_px: f32,
    pub y_advance_px: f32,
    pub x_offset_px: f32,
    pub y_offset_px: f32,
    pub kerning_px: f32,
}

impl Default for TextShapedGlyph {
    fn default() -> Self {
        Self {
            glyph_id: 0,
            codepoint: 0,
            cluster: 0,
            font_id: String::new(),
            atlas_id: String::new(),
            atlas_page: 0,
            atlas_rect_px: [0.0, 0.0, 0.0, 0.0],
            x_advance_px: 0.0,
            y_advance_px: 0.0,
            x_offset_px: 0.0,
            y_offset_px: 0.0,
            kerning_px: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextGlyphRun {
    pub font_id: String,
    pub atlas_id: String,
    pub glyph_count: u32,
    pub advance_px: f32,
    pub direction: TextDirection,
    pub language: String,
    pub glyphs: Vec<TextShapedGlyph>,
}

impl Default for TextGlyphRun {
    fn default() -> Self {
        Self {
            font_id: String::new(),
            atlas_id: String::new(),
            glyph_count: 0,
            advance_px: 0.0,
            direction: TextDirection::Auto,
            language: "und".to_owned(),
            glyphs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextSelectionRect {
    pub text_range: [usize; 2],
    pub rect_px: [f32; 4],
}

impl Default for TextSelectionRect {
    fn default() -> Self {
        Self {
            text_range: [0, 0],
            rect_px: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextShapeRunResponse {
    pub version: u32,
    pub runs: Vec<TextGlyphRun>,
    pub total_advance_px: f32,
    pub caret_positions_px: Vec<f32>,
    pub selection_rects: Vec<TextSelectionRect>,
    pub diagnostics: Vec<String>,
}

impl Default for TextShapeRunResponse {
    fn default() -> Self {
        Self {
            version: 1,
            runs: Vec::new(),
            total_advance_px: 0.0,
            caret_positions_px: vec![0.0],
            selection_rects: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
