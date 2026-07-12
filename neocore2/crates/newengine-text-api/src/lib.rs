#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral text-domain contracts for the `engine.ui.text` gateway.
//!
//! The crate mirrors the major responsibilities of a production game text stack:
//! catalog/file lookup, token formatting, message queues/history, paged text,
//! conversion, font metadata, shaping, paragraph layout, atlas planning and
//! presentation DTOs. Implementations remain provider-owned.

mod atlas;
mod catalog;
mod conversion;
mod font;
mod format;
mod layout;
mod messages;
mod paged;
mod presentation;
mod service;
mod shaping;

pub use atlas::{TextAtlasPage, TextAtlasPlanRequest, TextAtlasPlanResponse};
pub use catalog::{
    stable_text_key_hash, TextCatalogBlockDescriptor, TextCatalogEntry, TextCatalogManifestRequest,
    TextCatalogManifestResponse, TextCatalogSourceKind, TextChunkDescriptor, TextLocalizeRequest,
    TextLocalizeResponse, TextLookupKey, TextTextBlockLocation,
};
pub use conversion::{
    format_human_integer, format_milliseconds_long, format_milliseconds_short, TextConversionInput,
    TextConversionRequest, TextConversionResponse, TextTimestampStyle,
};
pub use font::{
    TextFontCoverageSummary, TextFontFaceDescriptor, TextFontFaceMetrics, TextFontFallbackRequest,
    TextFontFallbackResponse, TextFontManifest, TextFontSourceKind, TextFontStyle,
    TextFontVariationAxis, TextGlyphMetric, TEXT_FONT_ASSET_EDITOR, TEXT_FONT_REF_BRAND_DISPLAY,
    TEXT_FONT_REF_EDITOR_BOLD, TEXT_FONT_REF_EDITOR_DISPLAY, TEXT_FONT_REF_EDITOR_SANS,
    TEXT_FONT_REF_EMOJI, TEXT_FONT_REF_SYMBOLS,
};
pub use format::{
    expected_format_components, filter_control_tokens, TextBufferKind, TextColor,
    TextFormatArguments, TextFormatRequest, TextFormatResponse, TextFormatValue,
    TextInputIconDescriptor, TextInputIconKind, TextInputIconPolicy, TextNumberArgument,
    TextOverlayColor, TextRichSpan, TextRichSpanKind, TextSubstringArgument, TextSubstringSource,
    TextTokenDescriptor, TextTokenKind,
};
pub use layout::{
    TextLayoutParagraphRequest, TextLayoutParagraphResponse, TextLineBox, TextMeasureTextRequest,
    TextMeasureTextResponse, TextOverflowMode, TextWrapMode,
};
pub use messages::{
    TextArrowOrientation, TextMessageChannel, TextMessageDescriptor, TextMessageDismissRequest,
    TextMessageDismissResponse, TextMessageEnqueueRequest, TextMessageEnqueueResponse,
    TextMessageHistoryRequest, TextMessageHistoryResponse, TextMessageId, TextMessageRecord,
    TextMessageState, TextMessageStyle, TextMessageText, TextPreviousBriefOverride,
};
pub use paged::{paginate_text, TextPageBreakMode, TextPageTextRequest, TextPageTextResponse};
pub use presentation::{
    TextAlignment, TextBackgroundStyle, TextDrawBatch, TextDrawRun, TextLayoutStyle,
    TextOutlineStyle, TextScissorRect,
};
pub use service::{
    text_service_methods, TextServiceInfo, ENGINE_TEXT_SERVICE_ID, TEXT_BACKEND_CAPABILITY_ID,
    TEXT_BACKEND_SERVICE_SPEC, TEXT_REQUIRED_METHODS, TEXT_RUNTIME_CONTRACT_SPEC,
    TEXT_RUNTIME_REQUIREMENT_SPEC, TEXT_SERVICE_ID, TEXT_SERVICE_METHODS,
    TEXT_SERVICE_METHOD_ATLAS_PLAN_V1, TEXT_SERVICE_METHOD_CATALOG_MANIFEST_V1,
    TEXT_SERVICE_METHOD_CONVERT_V1, TEXT_SERVICE_METHOD_FONT_FALLBACK_V1,
    TEXT_SERVICE_METHOD_FONT_MANIFEST_V1, TEXT_SERVICE_METHOD_FORMAT_V1, TEXT_SERVICE_METHOD_INFO,
    TEXT_SERVICE_METHOD_INVOKE, TEXT_SERVICE_METHOD_LAYOUT_PARAGRAPH_V1,
    TEXT_SERVICE_METHOD_LOCALIZE_V1, TEXT_SERVICE_METHOD_MEASURE_TEXT_V1,
    TEXT_SERVICE_METHOD_MESSAGE_DISMISS_V1, TEXT_SERVICE_METHOD_MESSAGE_ENQUEUE_V1,
    TEXT_SERVICE_METHOD_MESSAGE_HISTORY_V1, TEXT_SERVICE_METHOD_PAGE_TEXT_V1,
    TEXT_SERVICE_METHOD_SHAPE_RUN_V1, TEXT_SERVICE_METHOD_SHUTDOWN_V1,
};
pub use shaping::{
    TextDirection, TextGlyphRun, TextImeComposition, TextSelectionRect, TextShapeRunRequest,
    TextShapeRunResponse, TextShapedGlyph,
};

#[cfg(test)]
mod tests;
