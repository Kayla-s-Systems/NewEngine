#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_render_api::{
    decode_json as decode_render_json, decode_unit_command_batch_bin,
    encode_json as encode_render_json, BindGroupId, BindGroupLayoutId, BufferId, PipelineId,
    RenderBackendCapabilities, RenderBackendInfo, RenderCommand, RenderCommandResponse,
    RenderGraphSubmitReport, RenderServiceRequest, RenderServiceResponse, RenderTargetId,
    RenderWorkBudget, SamplerId, ShaderId, TextureId, TextureResidencySnapshot,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json,
    register_null_engine_gateway_provider_service_dynamic_best_effort, JsonServiceRouter,
    NullEngineGatewayProviderDeclDynamic,
};
use newengine_ui_api::{
    decode_ui_frame_request_bin, encode_ui_frame_response_bin, UiAck, UiDrawList, UiFrameResponse,
    UiServiceInfo, UI_SERVICE_METHOD_ACTION_MANIFEST_V1, UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1,
    UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1, UI_SERVICE_METHOD_DEBUG_BINDINGS_V1,
    UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA, UI_SERVICE_METHOD_DEBUG_TREE_V1,
    UI_SERVICE_METHOD_DISPATCH_ACTION_V1, UI_SERVICE_METHOD_DISPATCH_INPUT_V1,
    UI_SERVICE_METHOD_DOCUMENT_XML_V1, UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1,
    UI_SERVICE_METHOD_DRAW_FRAME_V1, UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1,
    UI_SERVICE_METHOD_LOADING_SHELL_V1, UI_SERVICE_METHOD_MOUNT_SURFACE_V1,
    UI_SERVICE_METHOD_NAVIGATE_V1, UI_SERVICE_METHOD_REGISTRY_LOAD_V1,
    UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1, UI_SERVICE_METHOD_SURFACE_CATALOG_V1,
    UI_SERVICE_METHOD_SURFACE_MANIFEST_V1, UI_SERVICE_METHOD_SURFACE_NODE_V1,
    UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1,
};

const NULL_RENDER_SERVICE: &str = "null.render.api";
const NULL_RENDER_ROUTE: &str = "engine.render.null";
const NULL_PHYSICS_SERVICE: &str = "null.physics.api";
const NULL_PHYSICS_ROUTE: &str = "engine.physics.null";
const NULL_UI_SERVICE: &str = "null.ui.api";
const NULL_UI_ROUTE: &str = "engine.ui.null";
const NULL_AI_SERVICE: &str = "null.ai.api";

static NULL_PROVIDERS_REGISTERED: OnceLock<()> = OnceLock::new();

pub fn register_null_provider_routes_best_effort() {
    NULL_PROVIDERS_REGISTERED.get_or_init(|| {
        register_null_render_provider();
        register_null_physics_provider();
        register_null_ui_provider();
        register_null_ai_provider();
    });
}

#[derive(Debug, Default)]
struct NullRenderState {
    next_id: AtomicU32,
}

impl NullRenderState {
    fn alloc(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::AcqRel).max(1)
    }
}

fn null_render_info() -> RenderBackendInfo {
    RenderBackendInfo {
        backend_id: NULL_RENDER_ROUTE.to_owned(),
        backend_name: "NullRenderer".to_owned(),
        backend_version: "0.1.0".to_owned(),
        debug_text: "North Star | NullRenderer (degraded)".to_owned(),
        clear_color: [0.0, 0.0, 0.0, 1.0],
        capabilities: RenderBackendCapabilities::headless_default(),
        work_budget: RenderWorkBudget::default(),
        protocol_version: Default::default(),
    }
}

fn null_render_command(
    state: &mut NullRenderState,
    command: RenderCommand,
) -> RenderCommandResponse {
    match command {
        RenderCommand::CreateRenderTarget(_) => {
            RenderCommandResponse::RenderTargetId(RenderTargetId::new(state.alloc()))
        }
        RenderCommand::RenderTargetUiTexId { id } => {
            RenderCommandResponse::UiTexId(newengine_render_api::UiTexId::new(id.get()))
        }
        RenderCommand::RenderTargetColorTextureId { id } => {
            RenderCommandResponse::TextureId(TextureId::new(id.get()))
        }
        RenderCommand::CreateBuffer(_) => {
            RenderCommandResponse::BufferId(BufferId::new(state.alloc()))
        }
        RenderCommand::CreateTexture(_) => {
            RenderCommandResponse::TextureId(TextureId::new(state.alloc()))
        }
        RenderCommand::CreateSampler(_) => {
            RenderCommandResponse::SamplerId(SamplerId::new(state.alloc()))
        }
        RenderCommand::CreateShader(_) => {
            RenderCommandResponse::ShaderId(ShaderId::new(state.alloc()))
        }
        RenderCommand::CreatePipeline(_) => {
            RenderCommandResponse::PipelineId(PipelineId::new(state.alloc()))
        }
        RenderCommand::CreateBindGroupLayout(_) => {
            RenderCommandResponse::BindGroupLayoutId(BindGroupLayoutId::new(state.alloc()))
        }
        RenderCommand::CreateBindGroup(_) => {
            RenderCommandResponse::BindGroupId(BindGroupId::new(state.alloc()))
        }
        RenderCommand::PumpUploads(_) => {
            RenderCommandResponse::UploadPumpReport(Default::default())
        }
        RenderCommand::TextureResidency { id } => {
            RenderCommandResponse::TextureResidency(TextureResidencySnapshot::missing(id))
        }
        RenderCommand::WarmupPipelines(_) => {
            RenderCommandResponse::PipelineWarmupReport(Default::default())
        }
        RenderCommand::ShaderCacheStats => {
            RenderCommandResponse::ShaderCacheStats(Default::default())
        }
        RenderCommand::DiagnosticsSnapshot => {
            RenderCommandResponse::DiagnosticsSnapshot(Box::default())
        }
        _ => RenderCommandResponse::Unit,
    }
}

fn null_render_invoke(state: &mut NullRenderState, payload: Blob) -> RResult<Blob, RString> {
    let request = match decode_render_json::<RenderServiceRequest>(payload.as_slice()) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let response = match request {
        RenderServiceRequest::Command(command) => {
            RenderServiceResponse::Command(null_render_command(state, command))
        }
        RenderServiceRequest::CommandBatch(commands) => RenderServiceResponse::CommandBatch(
            commands
                .into_iter()
                .map(|cmd| null_render_command(state, cmd))
                .collect(),
        ),
        RenderServiceRequest::CompileRenderGraph(_graph) => {
            RenderServiceResponse::GraphCompileReport(Default::default())
        }
        RenderServiceRequest::ValidateRenderGraph(_graph) => {
            RenderServiceResponse::GraphValidationReport(Default::default())
        }
        RenderServiceRequest::SubmitRenderGraph(_) | RenderServiceRequest::SubmitFrame(_) => {
            RenderServiceResponse::GraphSubmitReport(RenderGraphSubmitReport::default())
        }
        RenderServiceRequest::PumpUploads(_) => {
            RenderServiceResponse::UploadPumpReport(Default::default())
        }
        RenderServiceRequest::DrainBackendEvents => {
            RenderServiceResponse::BackendEvents(Vec::new())
        }
        RenderServiceRequest::DiagnosticsSnapshot => {
            RenderServiceResponse::DiagnosticsSnapshot(Box::default())
        }
        RenderServiceRequest::Negotiate(req) => {
            let mut negotiation = newengine_render_api::negotiate_render_capabilities(
                req,
                newengine_render_api::RenderApiVersion::default(),
                &newengine_render_api::RenderBackendCapabilities::headless_default(),
            );
            negotiation.notices.push(newengine_render_api::RenderProtocolNotice::new(
                "null-renderer",
                "No concrete render backend route is active; NullRenderer provides degraded operation only.",
            ));
            RenderServiceResponse::Negotiation(negotiation)
        }
        RenderServiceRequest::SetRenderPhase { .. }
        | RenderServiceRequest::SetDrawListKind { .. }
        | RenderServiceRequest::DiscardRecordedCommands
        | RenderServiceRequest::SetWorkBudget(_) => RenderServiceResponse::Unit,
    };
    match encode_render_json(&response) {
        Ok(bytes) => RResult::ROk(Blob::from(bytes)),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn null_render_command_batch_bin(
    state: &mut NullRenderState,
    payload: Blob,
) -> RResult<Blob, RString> {
    let commands = match decode_unit_command_batch_bin(payload.as_slice()) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    for command in commands {
        let _ = null_render_command(state, command);
    }
    RResult::ROk(Blob::from(Vec::<u8>::new()))
}

fn register_null_render_provider() {
    let spec = newengine_render_api::RENDER_BACKEND_SERVICE_SPEC;
    let methods = [
        newengine_service_api::SERVICE_METHOD_INFO_JSON,
        newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
        newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
        newengine_render_api::RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V2,
    ];
    let description = engine_gateway_provider_service_description(
        NULL_RENDER_SERVICE,
        NULL_RENDER_ROUTE,
        spec.backend_capability_id,
        methods,
    )
    .gateway(spec.engine_gateway_id)
    .protocol("newengine.render-api/null-v2")
    .provider_abi(newengine_render_api::RENDER_PROVIDER_ABI_ID)
    .features(["degraded", "no-gpu-submit", "visible-null-provider"])
    .notes("Fallback is a real NullProvider route, not a hidden runtime branch.");
    let service = JsonServiceRouter::with_state(NULL_RENDER_SERVICE, NullRenderState::default())
        .describe_json(&description)
        .info(null_render_info)
        .blob(
            newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
            null_render_invoke,
        )
        .blob(
            newengine_render_api::RENDER_SERVICE_METHOD_COMMAND_BATCH_BIN_V2,
            null_render_command_batch_bin,
        )
        .shutdown()
        .into_service_v1();

    register_null_engine_gateway_provider_service_dynamic_best_effort(
        NullEngineGatewayProviderDeclDynamic {
            gateway: spec.engine_gateway_id,
            service_kind: spec.domain,
            provider_service: NULL_RENDER_SERVICE,
            provider_route: NULL_RENDER_ROUTE,
            provider_abi: Some(newengine_render_api::RENDER_PROVIDER_ABI_ID),
            capability: spec.backend_capability_id,
            owner: NULL_RENDER_ROUTE,
            service,
        },
    );
}

fn null_physics_info() -> newengine_physics_api::PhysicsBackendInfo {
    newengine_physics_api::PhysicsBackendInfo {
        backend_id: NULL_PHYSICS_ROUTE.to_owned(),
        backend_name: "NullPhysics".to_owned(),
        backend_version: "0.1.0".to_owned(),
        debug_text: "North Star | NullPhysics (degraded)".to_owned(),
        capabilities: newengine_physics_api::PhysicsBackendCapabilities::null_default(),
        protocol_version: Default::default(),
    }
}

fn null_physics_invoke(_state: &mut (), payload: Blob) -> RResult<Blob, RString> {
    let request = match serde_json::from_slice::<newengine_physics_api::PhysicsServiceRequest>(
        payload.as_slice(),
    ) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(RString::from(e.to_string())),
    };
    let response = match request {
        newengine_physics_api::PhysicsServiceRequest::Negotiate(req) => {
            newengine_physics_api::PhysicsServiceResponse::Negotiation(newengine_physics_api::PhysicsCapabilityNegotiationResponse {
                accepted_version: req.preferred_version,
                backend_version: Default::default(),
                ok: req.required_features.is_empty(),
                enabled_features: Vec::new(),
                missing_required_features: req.required_features,
                notices: vec![newengine_physics_api::PhysicsProtocolNotice::new(
                    "null-physics",
                    "No concrete physics backend route is active; NullPhysics accepted degraded operation.",
                )],
            })
        }
        newengine_physics_api::PhysicsServiceRequest::StepFrame(input) => {
            let mut output = newengine_physics_api::PhysicsFrameOutput {
                fixed_tick: input.fixed_tick,
                ..Default::default()
            };
            output.report.fixed_tick = input.fixed_tick;
            output.report.dt = input.dt;
            newengine_physics_api::PhysicsServiceResponse::FrameOutput(output)
        }
        newengine_physics_api::PhysicsServiceRequest::DiagnosticsSnapshot => {
            newengine_physics_api::PhysicsServiceResponse::DiagnosticsSnapshot(null_physics_info())
        }
    };
    ok_json(response)
}

fn register_null_physics_provider() {
    let spec = newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC;
    let methods = [
        newengine_service_api::SERVICE_METHOD_INFO_JSON,
        newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
        newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    ];
    let description = engine_gateway_provider_service_description(
        NULL_PHYSICS_SERVICE,
        NULL_PHYSICS_ROUTE,
        spec.backend_capability_id,
        methods,
    )
    .gateway(spec.engine_gateway_id)
    .protocol("newengine.physics-api/null-v1")
    .provider_abi(newengine_physics_api::PHYSICS_PROVIDER_ABI_ID)
    .features(["degraded", "no-contacts", "visible-null-provider"])
    .notes("Fallback is a real NullProvider route, not a hidden runtime branch.");
    let service = JsonServiceRouter::new(NULL_PHYSICS_SERVICE)
        .describe_json(&description)
        .info(null_physics_info)
        .blob(
            newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
            null_physics_invoke,
        )
        .shutdown()
        .into_service_v1();

    register_null_engine_gateway_provider_service_dynamic_best_effort(
        NullEngineGatewayProviderDeclDynamic {
            gateway: spec.engine_gateway_id,
            service_kind: spec.domain,
            provider_service: NULL_PHYSICS_SERVICE,
            provider_route: NULL_PHYSICS_ROUTE,
            provider_abi: Some(newengine_physics_api::PHYSICS_PROVIDER_ABI_ID),
            capability: spec.backend_capability_id,
            owner: NULL_PHYSICS_ROUTE,
            service,
        },
    );
}

fn null_ui_frame_json() -> UiFrameResponse {
    UiFrameResponse::new(UiDrawList::new())
}

fn null_ui_frame_bin(_state: &mut (), payload: Blob) -> RResult<Blob, RString> {
    if let Err(e) = decode_ui_frame_request_bin(payload.as_slice()) {
        return RResult::RErr(RString::from(e));
    }
    match encode_ui_frame_response_bin(&null_ui_frame_json()) {
        Ok(bytes) => RResult::ROk(Blob::from(bytes)),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn null_ui_ack(_state: &mut (), _value: serde_json::Value) -> Result<serde_json::Value, String> {
    serde_json::to_value(UiAck::ok(NULL_UI_ROUTE)).map_err(|e| e.to_string())
}

fn register_null_ui_provider() {
    let spec = newengine_ui_api::UI_BACKEND_SERVICE_SPEC;
    let methods = newengine_ui_api::ui_service_methods();
    let description = engine_gateway_provider_service_description(
        NULL_UI_SERVICE,
        NULL_UI_ROUTE,
        spec.backend_capability_id,
        methods.iter().copied(),
    )
    .gateway(spec.engine_gateway_id)
    .protocol("newengine.ui-api/null-v1")
    .provider_abi(newengine_ui_api::UI_PROVIDER_ABI_ID)
    .features(["degraded", "empty-draw-list", "visible-null-provider"])
    .notes("Fallback is a real NullProvider route, not a hidden runtime branch.");
    let info = || {
        let mut info = UiServiceInfo::default();
        info.features.push("null-provider".to_owned());
        info
    };
    let service = JsonServiceRouter::new(NULL_UI_SERVICE)
        .describe_json(&description)
        .info(info)
        .get_json(UI_SERVICE_METHOD_DRAW_FRAME_V1, |_state| null_ui_frame_json())
        .blob(UI_SERVICE_METHOD_DRAW_FRAME_BIN_V1, null_ui_frame_bin)
        .get_json(UI_SERVICE_METHOD_SURFACE_MANIFEST_V1, |_state| serde_json::json!({"surfaces": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_SURFACE_CATALOG_V1, |_state| serde_json::json!({"surfaces": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_LAYOUT_MANIFEST_V1, |_state| serde_json::json!({"layouts": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_ACTION_MANIFEST_V1, |_state| serde_json::json!({"actions": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_LOADING_SHELL_V1, |_state| serde_json::json!({"provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_DEBUG_TELEMETRY_SCHEMA, |_state| serde_json::json!({"schema": null, "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_DOCUMENT_XML_V1, |_state| serde_json::json!({"xml": "", "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_DEBUG_TREE_V1, |_state| serde_json::json!({"nodes": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .get_json(UI_SERVICE_METHOD_DEBUG_BINDINGS_V1, |_state| serde_json::json!({"bindings": [], "provider": NULL_UI_ROUTE, "degraded": true}))
        .json_value_result(UI_SERVICE_METHOD_SURFACE_NODE_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_REGISTRY_LOAD_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_MOUNT_SURFACE_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_UNMOUNT_SURFACE_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_SET_SURFACE_VISIBLE_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_APPLY_STATE_PATCH_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_DISPATCH_INPUT_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_DISPATCH_ACTION_V1, null_ui_ack)
        .json_value_result(UI_SERVICE_METHOD_NAVIGATE_V1, null_ui_ack)
        .shutdown()
        .into_service_v1();

    register_null_engine_gateway_provider_service_dynamic_best_effort(
        NullEngineGatewayProviderDeclDynamic {
            gateway: spec.engine_gateway_id,
            service_kind: spec.domain,
            provider_service: NULL_UI_SERVICE,
            provider_route: NULL_UI_ROUTE,
            provider_abi: Some(newengine_ui_api::UI_PROVIDER_ABI_ID),
            capability: spec.backend_capability_id,
            owner: NULL_UI_ROUTE,
            service,
        },
    );
}

fn register_null_ai_provider() {
    let methods = [
        newengine_service_api::SERVICE_METHOD_INFO_JSON,
        newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
        newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    ];
    let description = engine_gateway_provider_service_description(
        NULL_AI_SERVICE,
        "engine.ai.null",
        "ai.backend",
        methods,
    )
    .gateway("engine.ai")
    .protocol("northstar.ai-api/null-v1")
    .features(["degraded", "empty-intents", "visible-null-provider"])
    .notes("Fallback is a real NullProvider route, not a hidden runtime branch.");
    let service = JsonServiceRouter::new(NULL_AI_SERVICE)
        .describe_json(&description)
        .get_json(newengine_service_api::SERVICE_METHOD_INFO_JSON, |_state| {
            serde_json::json!({
                "backend_id": "engine.ai.null",
                "backend_name": "NullAI",
                "backend_version": "0.1.0",
                "debug_text": "North Star | NullAI (degraded)",
                "capabilities": [],
                "degraded": true
            })
        })
        .json_value_result(newengine_service_api::SERVICE_METHOD_INVOKE_JSON, |_state, _request| {
            Ok(serde_json::json!({
                "provider": "engine.ai.null",
                "degraded": true,
                "intents": [],
                "events": [],
                "warnings": ["No concrete AI backend route is active; NullAI returned an empty frame."]
            }))
        })
        .shutdown()
        .into_service_v1();

    register_null_engine_gateway_provider_service_dynamic_best_effort(
        NullEngineGatewayProviderDeclDynamic {
            gateway: "engine.ai",
            service_kind: "ai",
            provider_service: NULL_AI_SERVICE,
            provider_route: "engine.ai.null",
            provider_abi: None,
            capability: "ai.backend",
            owner: "engine.ai.null",
            service,
        },
    );
}

#[cfg(test)]
mod contract_conformance_tests {
    use super::*;

    #[test]
    fn null_render_provider_rejects_v1_protocol_negotiation() {
        let request = RenderServiceRequest::Negotiate(
            newengine_render_api::RenderCapabilityNegotiationRequest {
                preferred_version: newengine_render_api::RenderApiVersion::new(1, 0, 0),
                required_features: Vec::new(),
                optional_features: Vec::new(),
            },
        );
        let payload = Blob::from(encode_render_json(&request).expect("encode negotiation request"));
        let mut state = NullRenderState::default();
        let response = match null_render_invoke(&mut state, payload) {
            RResult::ROk(bytes) => decode_render_json::<RenderServiceResponse>(bytes.as_slice())
                .expect("decode negotiation response"),
            RResult::RErr(error) => panic!("null render negotiation failed: {error}"),
        };
        let RenderServiceResponse::Negotiation(response) = response else {
            panic!("expected render negotiation response");
        };
        assert!(!response.ok);
        assert_eq!(
            response.accepted_version,
            newengine_render_api::RenderApiVersion::new(2, 0, 0)
        );
        assert!(response
            .notices
            .iter()
            .any(|notice| notice.code == "render.protocol.major_mismatch"));
    }

    #[test]
    fn loaded_null_provider_routes_conform_to_registered_abis() {
        newengine_plugin_host::init_host_context();
        register_null_provider_routes_best_effort();
        for (backend, abi) in [
            (
                newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
                newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
            ),
            (
                newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC,
                newengine_physics_api::PHYSICS_PROVIDER_ABI_CONTRACT_SPEC,
            ),
            (
                newengine_ui_api::UI_BACKEND_SERVICE_SPEC,
                newengine_ui_api::UI_PROVIDER_ABI_CONTRACT_SPEC,
            ),
        ] {
            let route =
                newengine_plugin_host::active_engine_gateway_route(backend.engine_gateway_id)
                    .unwrap_or_else(|| {
                        panic!("missing loaded route {}", backend.engine_gateway_id)
                    });
            let report =
                newengine_contract_conformance::validate_active_route_abi(&route, backend, abi)
                    .unwrap_or_else(|errors| {
                        panic!(
                            "loaded route {} failed ABI conformance: {}",
                            backend.engine_gateway_id,
                            errors.join("; ")
                        )
                    });
            assert_eq!(report.provider_abi, abi.advertised_id.unwrap());
            assert_eq!(route.origin, "null-provider");
        }
    }
}
