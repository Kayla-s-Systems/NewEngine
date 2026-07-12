use serde::{Deserialize, Serialize};

use crate::TextShapedGlyph;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextAtlasPlanRequest {
    pub glyphs: Vec<TextShapedGlyph>,
    pub max_page_size_px: [u32; 2],
    pub format: String,
}

impl Default for TextAtlasPlanRequest {
    fn default() -> Self {
        Self {
            glyphs: Vec::new(),
            max_page_size_px: [1024, 1024],
            format: "rgba8".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextAtlasPage {
    pub atlas_id: String,
    pub page_index: u32,
    pub size_px: [u32; 2],
    pub format: String,
    pub glyph_count: u32,
}

impl Default for TextAtlasPage {
    fn default() -> Self {
        Self {
            atlas_id: String::new(),
            page_index: 0,
            size_px: [0, 0],
            format: "rgba8".to_owned(),
            glyph_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextAtlasPlanResponse {
    pub version: u32,
    pub atlas_id: String,
    pub pages: Vec<TextAtlasPage>,
    pub diagnostics: Vec<String>,
}

impl Default for TextAtlasPlanResponse {
    fn default() -> Self {
        Self {
            version: 1,
            atlas_id: "aurelia.default.font_atlas".to_owned(),
            pages: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
