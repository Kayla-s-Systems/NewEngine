#![forbid(unsafe_op_in_unsafe_fn)]
use serde::{Deserialize, Serialize};

pub const ENGINE_TEXT_SERVICE_ID: &str = "engine.ui.text";
pub const TEXT_SERVICE_ID: &str = "ui.text.api";
pub const TEXT_BACKEND_CAPABILITY_ID: &str = "ui.text.backend";

pub const TEXT_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const TEXT_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const TEXT_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const TEXT_SERVICE_METHOD_SHAPE_RUN_V1: &str = "text.shape_run_v1";
pub const TEXT_SERVICE_METHOD_LAYOUT_PARAGRAPH_V1: &str = "text.layout_paragraph_v1";
pub const TEXT_SERVICE_METHOD_FONT_FALLBACK_V1: &str = "text.font_fallback_v1";
pub const TEXT_SERVICE_METHOD_FONT_MANIFEST_V1: &str = "text.font_manifest_v1";
pub const TEXT_SERVICE_METHOD_ATLAS_PLAN_V1: &str = "text.atlas_plan_v1";
pub const TEXT_SERVICE_METHOD_MEASURE_TEXT_V1: &str = "text.measure_text_v1";
pub const TEXT_SERVICE_METHOD_LOCALIZE_V1: &str = "text.localize_v1";

pub const TEXT_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ui.text",
        ENGINE_TEXT_SERVICE_ID,
        TEXT_SERVICE_ID,
        TEXT_BACKEND_CAPABILITY_ID,
    );

pub const TEXT_REQUIRED_METHODS: &[&str] = &[
    TEXT_SERVICE_METHOD_INFO,
    TEXT_SERVICE_METHOD_INVOKE,
    TEXT_SERVICE_METHOD_SHUTDOWN_V1,
];

pub const TEXT_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_TEXT_SERVICE_ID,
        "newengine.ui.text-api >= 0.1.x",
        TEXT_REQUIRED_METHODS,
    );

pub const TEXT_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        TEXT_RUNTIME_CONTRACT_SPEC,
        Some(TEXT_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_UI_TEXT_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for TextServiceInfo {
    fn default() -> Self {
        Self {
            protocol: "newengine.ui.text-api/v1".to_owned(),
            features: vec![
                "font-fallback-v1".to_owned(),
                "unicode-shaping-v1".to_owned(),
                "glyph-id-advance-kerning-v1".to_owned(),
                "paragraph-layout-v1".to_owned(),
                "wrapping-ellipsis-v1".to_owned(),
                "caret-selection-geometry-v1".to_owned(),
                "ime-composition-v1".to_owned(),
                "cjk-rtl-bidi-declared-v1".to_owned(),
                "emoji-icon-font-fallback-v1".to_owned(),
                "glyph-atlas-pages-v1".to_owned(),
                "font-asset-neftd-v1".to_owned(),
                "harfbuzz-provider-implementation-v1".to_owned(),
                "imported-face-blob-source-v1".to_owned(),
                "localization-v1".to_owned(),
            ],
            methods: text_service_methods().iter().map(|it| (*it).to_owned()).collect(),
        }
    }
}

pub const TEXT_SERVICE_METHODS: &[&str] = &[
    TEXT_SERVICE_METHOD_INFO,
    TEXT_SERVICE_METHOD_INVOKE,
    TEXT_SERVICE_METHOD_SHUTDOWN_V1,
    TEXT_SERVICE_METHOD_SHAPE_RUN_V1,
    TEXT_SERVICE_METHOD_LAYOUT_PARAGRAPH_V1,
    TEXT_SERVICE_METHOD_FONT_FALLBACK_V1,
    TEXT_SERVICE_METHOD_FONT_MANIFEST_V1,
    TEXT_SERVICE_METHOD_ATLAS_PLAN_V1,
    TEXT_SERVICE_METHOD_MEASURE_TEXT_V1,
    TEXT_SERVICE_METHOD_LOCALIZE_V1,
];

#[inline]
pub const fn text_service_methods() -> &'static [&'static str] { TEXT_SERVICE_METHODS }

/// Canonical editor font dictionary. The file is a NEF8/ListFile `.neftd` asset;
/// entries name imported font faces and atlas policy. Runtime code consumes this
/// reference, not raw `.ttf` or `.otf` files.
pub const TEXT_FONT_ASSET_EDITOR: &str = "assets/ui/fonts/editor.neftd";
pub const TEXT_FONT_REF_EDITOR_SANS: &str = "assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold";
pub const TEXT_FONT_REF_EDITOR_DISPLAY: &str = "assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_black";
pub const TEXT_FONT_REF_EDITOR_BOLD: &str = "assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold";
pub const TEXT_FONT_REF_BRAND_DISPLAY: &str = "assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_black";
pub const TEXT_FONT_REF_SYMBOLS: &str = "Segoe UI Symbol";
pub const TEXT_FONT_REF_EMOJI: &str = "Segoe UI Emoji";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextFontSourceKind {
    ImportedFace,
    SystemFallback,
    GeneratedAtlas,
}

impl Default for TextFontSourceKind {
    #[inline]
    fn default() -> Self { Self::ImportedFace }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    Auto,
    Ltr,
    Rtl,
}

impl Default for TextDirection {
    #[inline]
    fn default() -> Self { Self::Auto }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextWrapMode {
    None,
    Word,
    Character,
    Anywhere,
}

impl Default for TextWrapMode {
    #[inline]
    fn default() -> Self { Self::Word }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOverflowMode {
    Clip,
    Ellipsis,
}

impl Default for TextOverflowMode {
    #[inline]
    fn default() -> Self { Self::Clip }
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
    fn default() -> Self { Self { tag: String::new(), min: 0.0, default: 0.0, max: 0.0 } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontCoverageSummary {
    pub unicode_ranges: Vec<String>,
    pub cmap_entries: usize,
    pub missing_codepoints: Vec<u32>,
}

impl Default for TextFontCoverageSummary {
    fn default() -> Self { Self { unicode_ranges: Vec::new(), cmap_entries: 0, missing_codepoints: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextFontFaceDescriptor {
    pub id: String,
    pub family: String,
    pub source_kind: TextFontSourceKind,
    /// Runtime font face reference. For imported North Star fonts this is a
    /// stable ListFile selector such as `assets/ui/fonts/editor.neftd@tt_lakes_neue_trial_bold`.
    pub source_ref: String,
    /// Imported face blob reference inside the `.neftd` dictionary. This is not an
    /// authoring `.ttf/.otf` path; providers resolve it through engine.assets.
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
            schema: "newengine.font_dictionary.neftd.v1".to_owned(),
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
pub struct TextImeComposition {
    pub active: bool,
    pub text: String,
    pub selection_start: usize,
    pub selection_end: usize,
    pub clauses: Vec<[usize; 2]>,
}

impl Default for TextImeComposition {
    fn default() -> Self {
        Self { active: false, text: String::new(), selection_start: 0, selection_end: 0, clauses: Vec::new() }
    }
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
            font_stack: vec![TEXT_FONT_REF_EDITOR_SANS.to_owned(), "Segoe UI".to_owned(), "NotoSans".to_owned()],
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
    fn default() -> Self { Self { text_range: [0, 0], rect_px: [0.0, 0.0, 0.0, 0.0] } }
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
        Self { version: 1, runs: Vec::new(), total_advance_px: 0.0, caret_positions_px: vec![0.0], selection_rects: Vec::new(), diagnostics: Vec::new() }
    }
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
    fn default() -> Self { Self { text_range: [0, 0], glyph_range: [0, 0], rect_px: [0.0, 0.0, 0.0, 0.0], baseline_y_px: 0.0, ellipsized: false } }
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
        Self { version: 1, lines: Vec::new(), glyph_runs: Vec::new(), caret_positions_px: Vec::new(), selection_rects: Vec::new(), size_px: [0.0, 0.0], diagnostics: Vec::new() }
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
        Self { text: String::new(), locale: "und".to_owned(), font_stack: TextFontManifest::default().fallback_stack, size_px: 16.0, max_width_px: 0.0, wrap: TextWrapMode::Word }
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
    fn default() -> Self { Self { version: 1, size_px: [0.0, 0.0], baseline_y_px: 0.0, line_count: 0, glyph_count: 0, diagnostics: Vec::new() } }
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
    fn default() -> Self { Self { text: String::new(), locale: "und".to_owned(), preferred_stack: Vec::new(), require_emoji: false, require_symbols: false } }
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
    fn default() -> Self { Self { version: 1, fallback_stack: TextFontManifest::default().fallback_stack, missing_codepoints: Vec::new(), diagnostics: Vec::new() } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TextAtlasPlanRequest {
    pub glyphs: Vec<TextShapedGlyph>,
    pub max_page_size_px: [u32; 2],
    pub format: String,
}

impl Default for TextAtlasPlanRequest {
    fn default() -> Self { Self { glyphs: Vec::new(), max_page_size_px: [1024, 1024], format: "rgba8".to_owned() } }
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
    fn default() -> Self { Self { atlas_id: String::new(), page_index: 0, size_px: [0, 0], format: "rgba8".to_owned(), glyph_count: 0 } }
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
    fn default() -> Self { Self { version: 1, atlas_id: "aurelia.default.font_atlas".to_owned(), pages: Vec::new(), diagnostics: Vec::new() } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_gateway_is_engine_facing() {
        assert_eq!(ENGINE_TEXT_SERVICE_ID, "engine.ui.text");
        assert_eq!(TEXT_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_TEXT_SERVICE_ID);
    }

    #[test]
    fn text_contract_contains_paragraph_and_measurement_methods() {
        assert!(text_service_methods().contains(&TEXT_SERVICE_METHOD_SHAPE_RUN_V1));
        assert!(text_service_methods().contains(&TEXT_SERVICE_METHOD_LAYOUT_PARAGRAPH_V1));
        assert!(text_service_methods().contains(&TEXT_SERVICE_METHOD_MEASURE_TEXT_V1));
        assert!(text_service_methods().contains(&TEXT_SERVICE_METHOD_ATLAS_PLAN_V1));
    }
}
