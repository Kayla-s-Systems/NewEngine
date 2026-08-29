#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_navigation_api::{
    navigation_method, NavPathDtoV1, NavPathPointV1, NavPlanPathRequestV1, NavPlanPathResponseV1,
    NavProjectPointRequestV1, NavProjectPointResponseV1, NavQueryStatusRequestV1,
    NavQueryStatusResponseV1, NavigationServiceInfoV1, NAVIGATION_BACKEND_CAPABILITY_ID,
    NAVIGATION_SERVICE_ID, NAVIGATION_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service, EngineGatewayProviderDecl, JsonServiceRouter,
};
use serde_json::Value;

pub const PROVIDER_ROUTE: &str = "engine.navigation.foundation";
const OWNER: &str = "newengine-navigation-runtime.foundation-provider";
#[derive(Default)]
struct NavigationState;
impl NavigationState {
    fn info(&self) -> NavigationServiceInfoV1 {
        NavigationServiceInfoV1::default()
    }
    fn plan(&self, req: NavPlanPathRequestV1) -> NavPlanPathResponseV1 {
        let path = NavPathDtoV1 {
            points: vec![
                NavPathPointV1 {
                    position: req.start,
                    flags: req.tags.clone(),
                },
                NavPathPointV1 {
                    position: req.goal,
                    flags: req.tags,
                },
            ],
            cost: ((req.goal.x - req.start.x).powi(2)
                + (req.goal.y - req.start.y).powi(2)
                + (req.goal.z - req.start.z).powi(2))
            .sqrt(),
            complete: true,
        };
        NavPlanPathResponseV1 {
            accepted: true,
            path: Some(path),
            diagnostics: vec![
                "foundation provider returned deterministic straight-line path DTO".to_owned(),
            ],
        }
    }
    fn project(&self, req: NavProjectPointRequestV1) -> NavProjectPointResponseV1 {
        NavProjectPointResponseV1 {
            accepted: true,
            projected: Some(req.point),
            diagnostics: Vec::new(),
        }
    }
    fn status(&self, req: NavQueryStatusRequestV1) -> NavQueryStatusResponseV1 {
        NavQueryStatusResponseV1 {
            accepted: true,
            status: if req.query_id.is_empty() {
                "ready".to_owned()
            } else {
                "known".to_owned()
            },
            diagnostics: Vec::new(),
        }
    }
}
fn envelope(payload: &Blob, default_method: &str) -> Result<(String, Value), RString> {
    let value = payload_json(payload).map_err(RString::from)?;
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(default_method)
        .to_owned();
    Ok((method, value.get("request").cloned().unwrap_or(Value::Null)))
}
fn invoke(state: &mut NavigationState, payload: Blob) -> RResult<Blob, RString> {
    let (method, request) = match envelope(&payload, navigation_method::PLAN_PATH_JSON_V1) {
        Ok(v) => v,
        Err(e) => return RResult::RErr(e),
    };
    let decode = |e: serde_json::Error| RResult::RErr(RString::from(e.to_string()));
    match method.as_str() {
        navigation_method::INFO_JSON => ok_json(state.info()),
        navigation_method::PLAN_PATH_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.plan(v)),
            Err(e) => decode(e),
        },
        navigation_method::PROJECT_POINT_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.project(v)),
            Err(e) => decode(e),
        },
        navigation_method::QUERY_STATUS_JSON_V1 => match serde_json::from_value(request) {
            Ok(v) => ok_json(state.status(v)),
            Err(e) => decode(e),
        },
        other => RResult::RErr(RString::from(format!(
            "navigation.api: unknown invoke method '{other}'"
        ))),
    }
}
pub fn register_navigation_gateway_best_effort() -> bool {
    let description=engine_gateway_provider_service_description(NAVIGATION_SERVICE_ID,PROVIDER_ROUTE,NAVIGATION_BACKEND_CAPABILITY_ID,NAVIGATION_SERVICE_METHODS.iter().copied()).gateway("engine.navigation").protocol("newengine.navigation.foundation/v1").features(["single-purpose-provider","replaceable-gateway-route","dto-only-boundary"]).notes("Owns only baseline navigation query semantics; no tags registry, tasks, animation, AI, or world mutation.");
    let service = JsonServiceRouter::with_state(NAVIGATION_SERVICE_ID, NavigationState)
        .describe_json(&description)
        .get_json(navigation_method::INFO_JSON, |state| state.info())
        .post_json(navigation_method::PLAN_PATH_JSON_V1, |state, req| {
            state.plan(req)
        })
        .post_json(navigation_method::PROJECT_POINT_JSON_V1, |state, req| {
            state.project(req)
        })
        .post_json(navigation_method::QUERY_STATUS_JSON_V1, |state, req| {
            state.status(req)
        })
        .blob(navigation_method::INVOKE_JSON, invoke)
        .shutdown()
        .into_service_v1();
    register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: "engine.navigation",
        service_kind: newengine_service_api::EngineServiceKind::Navigation,
        provider_service: NAVIGATION_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: NAVIGATION_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service,
    })
    .is_ok()
}

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.navigation",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_navigation_api::NAVIGATION_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    _: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let _ = register_navigation_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );
