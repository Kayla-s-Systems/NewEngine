#![forbid(unsafe_op_in_unsafe_fn)]
use serde::{Deserialize, Serialize};

pub use newengine_ui_draw::{
    reserved, TextureRef, UiBorderPaintCommand, UiClipPaintCommand, UiDrawCmd, UiDrawList,
    UiIconPaintCommand, UiImagePaintCommand, UiImageRef, UiLayerPaintCommand, UiMesh,
    UiPaintCommand, UiPaintList, UiPaintNodeRef, UiRect, UiRectPaintCommand,
    UiRoundedRectPaintCommand, UiScopePaintCommand, UiTexId, UiTextPaintCommand, UiTexture,
    UiTextureDelta, UiTexturePatch, UiVectorPaintCommand, UiVertex, VectorRef,
};
use std::collections::BTreeMap;

mod frame_binary;
pub use frame_binary::{
    decode_ui_frame_request_bin, decode_ui_frame_response_bin, encode_ui_frame_request_bin,
    encode_ui_frame_response_bin,
};

include!("input.rs");
include!("events.rs");
include!("draw_protocol.rs");
mod screen_profile;
pub use screen_profile::*;
include!("game_gui.rs");
include!("actions.rs");
include!("text.rs");
include!("style.rs");
include!("theme.rs");
include!("node.rs");
include!("surface.rs");
include!("layout.rs");
include!("debug.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_service_ids_are_engine_gateway_first() {
        assert_eq!(ENGINE_UI_SERVICE_ID, "engine.ui");
        assert_eq!(
            UI_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_UI_SERVICE_ID
        );
        assert_eq!(UI_BACKEND_SERVICE_SPEC.provider_service_id, UI_SERVICE_ID);
        assert_eq!(
            UI_BACKEND_SERVICE_SPEC.backend_capability_id,
            UI_BACKEND_CAPABILITY_ID
        );
    }

    #[test]
    fn ui_runtime_contract_contains_json_control_methods() {
        let methods = UI_RUNTIME_CONTRACT_SPEC.required_methods;
        assert!(methods.contains(&UI_SERVICE_METHOD_INFO));
        assert!(methods.contains(&UI_SERVICE_METHOD_INVOKE));
        assert!(methods.contains(&UI_SERVICE_METHOD_SHUTDOWN_V1));
    }

    #[test]
    fn ui_service_methods_include_draw_frame() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DRAW_FRAME_V1));
    }

    #[test]
    fn ui_service_methods_include_binary_draw_frame() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1));
    }

    #[test]
    fn ui_service_methods_include_neui_runtime_lifecycle() {
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_MOUNT_SURFACE_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DEBUG_TREE_V1));
        assert!(ui_service_methods().contains(&UI_SERVICE_METHOD_DEBUG_BINDINGS_V1));
    }

    #[test]
    fn ui_node_tree_request_converges_to_surface_node() {
        let request = UiNodeTreeRequest {
            surface_id: "engine.ui.test.generated".to_owned(),
            source_kind: UiNodeRequestSourceKind::Generated,
            root: UiNodeRequest::new("root", UiRuntimeNodeKind::Panel)
                .with_text("Generated")
                .with_child(
                    UiNodeRequest::new("button.play", UiRuntimeNodeKind::Action).with_text("Play"),
                ),
            ..UiNodeTreeRequest::default()
        };
        let node = request.to_surface_node();
        assert_eq!(node.surface_id, "engine.ui.test.generated");
        assert_eq!(node.components.len(), 1);
        assert_eq!(node.components[0].component_id, UI_COMPONENT_ACTION);
    }

    #[test]
    fn state_patch_is_surface_scoped() {
        let patch = UiStatePatch::new(42, UI_SURFACE_ENGINE_LOADING).with_change(
            "loading",
            "progress",
            serde_json::json!(0.5),
        );
        assert_eq!(patch.surface_id, UI_SURFACE_ENGINE_LOADING);
        assert_eq!(patch.changes.len(), 1);
    }

    #[test]
    fn telemetry_defaults_to_runtime_debug_surface() {
        let telemetry = UiRuntimeDebugOverlayTelemetry::new(7, "FPS 60");
        assert_eq!(telemetry.surface_id, UI_SURFACE_RUNTIME_DEBUG_OVERLAY);
        assert_eq!(telemetry.lines, vec!["FPS 60".to_owned()]);
    }
}
