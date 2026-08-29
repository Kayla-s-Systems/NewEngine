#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.ui` semantic service.

//!

//! `.neui` is a NEF8/ListFile UI dictionary. This crate owns the UI-domain

//! meaning of that dictionary: XMLcentral validation, surface/document selection,

//! dependency extraction and runtime DTO compilation. Consumers only call the

//! `engine.assets.ui` gateway and receive a response DTO.

use abi_stable::std_types::{RResult, RString};

use newengine_assets_api::AssetServiceClient;

use newengine_assets_api::{
    assets_ui_method, ASSETS_UI_BACKEND_CAPABILITY_ID, ASSETS_UI_RUNTIME_CONTRACT,
    ASSETS_UI_SERVICE_ID, ASSETS_UI_SERVICE_METHODS, ENGINE_ASSETS_UI_SERVICE_ID,
    ENGINE_ASSET_SERVICE_ID, LIST_FILE_CONTENT_KIND_NEUI,
};

use newengine_plugin_api::Blob;

use newengine_service_api::EngineServiceKind;

use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};

use newengine_ui_api::{
    UiActionEdge, UiBindingEdge, UiBindingMode, UiBindingPlan, UiCompiledDocument,
    UiComponentLibraryRef, UiComponentTemplate, UiDocumentSource, UiDocumentSourceKind,
    UiNodeBindingRequest, UiNodeEventRoute, UiNodeEventTrigger, UiNodeRequest, UiNodeTone,
    UiRuntimeNodeKind, UiSourceSpan, UiStateSource, UiThemeLibraryRef, UiUpdatePolicy,
    UI_COMPONENT_ACTION, UI_COMPONENT_BUTTON, UI_COMPONENT_CHECKBOX, UI_COMPONENT_EXTERNAL_TEXTURE,
    UI_COMPONENT_GRID, UI_COMPONENT_INPUT, UI_COMPONENT_LIST, UI_COMPONENT_ROW,
    UI_COMPONENT_SCROLL_BAR, UI_COMPONENT_SELECT, UI_COMPONENT_SEPARATOR, UI_COMPONENT_SLIDER,
    UI_COMPONENT_SPACER, UI_COMPONENT_STACK, UI_COMPONENT_SURFACE, UI_COMPONENT_TEXT,
    UI_COMPONENT_TOGGLE, UI_COMPONENT_TREE, UI_COMPONENT_VIEWPORT,
};

use newengine_ui_navigation_api::{
    UiNodeActionRoute, UiNodeFeedbackEvent, UiNodeFeedbackSeverity, UiNodeNavigationDocument,
    UiNodeNavigationItem, UiNodeNavigationPage, UiNodeNavigationTone, UiNodeTransition,
    UiNodeTransitionKind,
};

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

pub const ASSETS_UI_GATEWAY_OWNER: &str = "newengine-assets-ui-runtime.engine-runtime-provider";

mod compile_helpers;
mod compile_request;
mod dto;
mod navigation;
mod neui_dialect;
mod node_compile;
mod service;
mod state;
mod theme;
mod xml;

pub use compile_helpers::{compile_neui_bytes_surface_root, compile_xmlcentral_surface_root};
pub use dto::{
    AssetsUiCompileRequest, AssetsUiCompileResponse, AssetsUiDiagnosticResponse,
    AssetsUiDialectInspectRequest, AssetsUiInvalidateRequest, AssetsUiRefRequest,
    AssetsUiServiceInfo,
};
pub use service::{
    assets_ui_gateway_service, assets_ui_service_info, register_assets_ui_gateway_best_effort,
};
pub use state::AssetsUiRuntimeState;
pub(crate) use state::CachedXmlCentral;

pub(crate) use navigation::*;
pub(crate) use neui_dialect::{
    is_metadata_element, sanitize_tag, NeUiDialect, DEFAULT_NEUI_DIALECT_REF,
};
pub(crate) use node_compile::*;
pub(crate) use theme::*;
pub(crate) use xml::*;

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.assets-ui",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_assets_api::ASSETS_UI_BACKEND_CAPABILITY_ID],
        &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let client =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let _ = register_assets_ui_gateway_best_effort(client);
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests;
