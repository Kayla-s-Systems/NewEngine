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
pub const TEXT_SERVICE_METHOD_CATALOG_MANIFEST_V1: &str = "text.catalog_manifest_v1";
pub const TEXT_SERVICE_METHOD_FORMAT_V1: &str = "text.format_v1";
pub const TEXT_SERVICE_METHOD_MESSAGE_ENQUEUE_V1: &str = "text.message_enqueue_v1";
pub const TEXT_SERVICE_METHOD_MESSAGE_DISMISS_V1: &str = "text.message_dismiss_v1";
pub const TEXT_SERVICE_METHOD_MESSAGE_HISTORY_V1: &str = "text.message_history_v1";
pub const TEXT_SERVICE_METHOD_PAGE_TEXT_V1: &str = "text.page_text_v1";
pub const TEXT_SERVICE_METHOD_CONVERT_V1: &str = "text.convert_v1";

pub const TEXT_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ui.text",
        ENGINE_TEXT_SERVICE_ID,
        TEXT_SERVICE_ID,
        TEXT_BACKEND_CAPABILITY_ID,
    );

/// Minimal lifecycle methods required from every provider. Domain methods remain
/// discoverable through `TextServiceInfo` so older providers can degrade cleanly.
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
    TEXT_SERVICE_METHOD_CATALOG_MANIFEST_V1,
    TEXT_SERVICE_METHOD_FORMAT_V1,
    TEXT_SERVICE_METHOD_MESSAGE_ENQUEUE_V1,
    TEXT_SERVICE_METHOD_MESSAGE_DISMISS_V1,
    TEXT_SERVICE_METHOD_MESSAGE_HISTORY_V1,
    TEXT_SERVICE_METHOD_PAGE_TEXT_V1,
    TEXT_SERVICE_METHOD_CONVERT_V1,
];

#[inline]
pub const fn text_service_methods() -> &'static [&'static str] {
    TEXT_SERVICE_METHODS
}

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
            features: [
                "localization-v1",
                "catalog-blocks-and-priority-v1",
                "hash-and-label-lookup-v1",
                "token-formatting-and-input-icons-v1",
                "message-queues-and-history-v1",
                "paged-text-v1",
                "text-conversion-v1",
                "font-fallback-v1",
                "unicode-shaping-v1",
                "glyph-id-advance-kerning-v1",
                "paragraph-layout-v1",
                "wrapping-ellipsis-v1",
                "caret-selection-geometry-v1",
                "ime-composition-v1",
                "cjk-rtl-bidi-declared-v1",
                "emoji-icon-font-fallback-v1",
                "glyph-atlas-pages-v1",
                "font-asset-yfd-v1",
                "harfbuzz-provider-implementation-v1",
                "imported-face-blob-source-v1",
                "provider-neutral-draw-state-v1",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            methods: text_service_methods()
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
        }
    }
}
