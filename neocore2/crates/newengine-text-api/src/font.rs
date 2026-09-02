use serde::{Deserialize, Serialize};

/// Canonical editor font dictionary. Runtime code consumes this `.yfd`
/// reference rather than raw `.ttf` or `.otf` authoring files.
pub const TEXT_FONT_ASSET_EDITOR: &str = "ui/fonts/editor.yfd";
pub const TEXT_FONT_REF_EDITOR_SANS: &str = "ui/fonts/editor.yfd@tt_lakes_neue_trial_bold";
pub const TEXT_FONT_REF_EDITOR_DISPLAY: &str = "ui/fonts/editor.yfd@tt_lakes_neue_trial_black";
pub const TEXT_FONT_REF_EDITOR_BOLD: &str = "ui/fonts/editor.yfd@tt_lakes_neue_trial_bold";
pub const TEXT_FONT_REF_BRAND_DISPLAY: &str = "ui/fonts/editor.yfd@tt_lakes_neue_trial_black";
pub const TEXT_FONT_REF_SYMBOLS: &str = "Segoe UI Symbol";
pub const TEXT_FONT_REF_EMOJI: &str = "Segoe UI Emoji";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextFontSourceKind {
    #[default]
    ImportedFace,
    EmbeddedDebugFallback,
    SystemFallback,
    GeneratedAtlas,
}

/// Semantic font roles corresponding to the fixed style table used by classic
/// game text systems. Providers map these roles to concrete faces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextFontStyle {
    #[default]
    Standard,
    Cursive,
    BrandTag,
    Leaderboard,
    Condensed,
    FixedWidthNumbers,
    PriceDisplay,
    Taxi,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontVariationAxis {
    pub tag: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
}

impl Default for TextFontVariationAxis {
    fn default() -> Self {
        Self {
            tag: String::new(),
            min: 0.0,
            default: 0.0,
            max: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextFontCoverageSummary {
    pub unicode_ranges: Vec<String>,
    pub cmap_entries: usize,
    pub missing_codepoints: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextGlyphMetric {
    pub codepoint: u32,
    pub glyph_id: u32,
    pub advance_px: f32,
    pub bearing_px: [f32; 2],
    pub size_px: [f32; 2],
    pub atlas_rect_px: [f32; 4],
}

impl Default for TextGlyphMetric {
    fn default() -> Self {
        Self {
            codepoint: 0,
            glyph_id: 0,
            advance_px: 0.0,
            bearing_px: [0.0, 0.0],
            size_px: [0.0, 0.0],
            atlas_rect_px: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Provider-neutral equivalent of a font definition/font-store entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontFaceMetrics {
    pub font_id: String,
    pub line_height_px: f32,
    pub ascender_px: f32,
    pub descender_px: f32,
    pub character_spacing_px: f32,
    pub non_proportional_advance_px: f32,
    pub glyphs: Vec<TextGlyphMetric>,
}

impl Default for TextFontFaceMetrics {
    fn default() -> Self {
        Self {
            font_id: String::new(),
            line_height_px: 16.0,
            ascender_px: 12.0,
            descender_px: -4.0,
            character_spacing_px: 0.0,
            non_proportional_advance_px: 0.0,
            glyphs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontFaceDescriptor {
    pub id: String,
    pub family: String,
    pub source_kind: TextFontSourceKind,
    pub source_ref: String,
    pub source_blob_ref: String,
    pub source_blob_len: usize,
    pub weight: u16,
    pub weight_range: Option<[u16; 2]>,
    pub style: String,
    pub coverage: TextFontCoverageSummary,
    pub unicode_ranges: Vec<String>,
    pub features: Vec<String>,
    pub variations: Vec<TextFontVariationAxis>,
    pub atlas_policy: String,
    pub metrics: Option<TextFontFaceMetrics>,
}

impl Default for TextFontFaceDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            family: String::new(),
            source_kind: TextFontSourceKind::ImportedFace,
            source_ref: String::new(),
            source_blob_ref: String::new(),
            source_blob_len: 0,
            weight: 400,
            weight_range: None,
            style: "normal".to_owned(),
            coverage: TextFontCoverageSummary::default(),
            unicode_ranges: Vec::new(),
            features: Vec::new(),
            variations: Vec::new(),
            atlas_policy: "msdf_or_sdf_prebake".to_owned(),
            metrics: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontManifest {
    pub schema: String,
    pub asset_ref: String,
    pub faces: Vec<TextFontFaceDescriptor>,
    pub fallback_stack: Vec<String>,
}

impl Default for TextFontManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.font_dictionary.yfd.v1".to_owned(),
            asset_ref: TEXT_FONT_ASSET_EDITOR.to_owned(),
            faces: Vec::new(),
            fallback_stack: vec![
                TEXT_FONT_REF_EDITOR_SANS.to_owned(),
                "Segoe UI".to_owned(),
                "NotoSans".to_owned(),
                TEXT_FONT_REF_SYMBOLS.to_owned(),
                TEXT_FONT_REF_EMOJI.to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontFallbackRequest {
    pub text: String,
    pub locale: String,
    pub preferred_stack: Vec<String>,
    pub require_emoji: bool,
    pub require_symbols: bool,
}

impl Default for TextFontFallbackRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            locale: "und".to_owned(),
            preferred_stack: Vec::new(),
            require_emoji: false,
            require_symbols: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontFallbackResponse {
    pub version: u32,
    pub fallback_stack: Vec<String>,
    pub missing_codepoints: Vec<u32>,
    pub diagnostics: Vec<String>,
}

impl Default for TextFontFallbackResponse {
    fn default() -> Self {
        Self {
            version: 1,
            fallback_stack: TextFontManifest::default().fallback_stack,
            missing_codepoints: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}
