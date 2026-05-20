#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_TEXT_SERVICE_ID: &str = "engine.ui.text";
pub const TEXT_SERVICE_ID: &str = "ui.text.api";
pub const TEXT_BACKEND_CAPABILITY_ID: &str = "ui.text.backend";

pub const TEXT_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const TEXT_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const TEXT_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const TEXT_SERVICE_METHOD_SHAPE_RUN_V1: &str = "text.shape_run_v1";
pub const TEXT_SERVICE_METHOD_FONT_FALLBACK_V1: &str = "text.font_fallback_v1";
pub const TEXT_SERVICE_METHOD_ATLAS_PLAN_V1: &str = "text.atlas_plan_v1";
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
                "glyph-atlas-v1".to_owned(),
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
    TEXT_SERVICE_METHOD_FONT_FALLBACK_V1,
    TEXT_SERVICE_METHOD_ATLAS_PLAN_V1,
    TEXT_SERVICE_METHOD_LOCALIZE_V1,
];

#[inline]
pub const fn text_service_methods() -> &'static [&'static str] { TEXT_SERVICE_METHODS }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextShapeRunRequest {
    pub text: String,
    pub locale: String,
    pub font_stack: Vec<String>,
    pub size_px: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextGlyphRun {
    pub font_id: String,
    pub atlas_id: String,
    pub glyph_count: u32,
    pub advance_px: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextShapeRunResponse {
    pub version: u32,
    pub runs: Vec<TextGlyphRun>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_gateway_is_engine_facing() {
        assert_eq!(ENGINE_TEXT_SERVICE_ID, "engine.ui.text");
        assert_eq!(TEXT_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_TEXT_SERVICE_ID);
    }
}
