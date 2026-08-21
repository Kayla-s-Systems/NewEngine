use std::sync::{Arc, Mutex, OnceLock};

use newengine_platform_api::{
    NativeWindowBackendV1, PlatformServiceInfo, PlatformWindowReadyV1, ENGINE_PLATFORM_SERVICE_ID,
    PLATFORM_BACKEND_CAPABILITY_ID, PLATFORM_SERVICE_METHOD_INVOKE,
    PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, register_engine_gateway_provider_service,
    EngineGatewayProviderDecl, JsonServiceRouter,
};

static PLATFORM_WINDOW_SNAPSHOT: OnceLock<Arc<Mutex<PlatformWindowReadyV1>>> = OnceLock::new();
const PLATFORM_WINDOW_GATEWAY_OWNER: &str = "newengine-runtime-host.platform-window-gateway";
const PLATFORM_HEADLESS_GATEWAY_OWNER: &str = "newengine-runtime-host.platform-headless-gateway";
const PLATFORM_WINDOW_PROVIDER_ROUTE: &str = "engine.platform.winit";
const PLATFORM_HEADLESS_PROVIDER_ROUTE: &str = "engine.platform.headless";

#[derive(Debug, Clone, Copy)]
struct PlatformRouteIdentity {
    owner: &'static str,
    provider_route: &'static str,
    feature: &'static str,
    note: &'static str,
}

fn platform_route_identity(initial: PlatformWindowReadyV1) -> PlatformRouteIdentity {
    match (initial.handles.backend, initial.handles.window) {
        (NativeWindowBackendV1::Unknown, _) | (_, 0) => PlatformRouteIdentity {
            owner: PLATFORM_HEADLESS_GATEWAY_OWNER,
            provider_route: PLATFORM_HEADLESS_PROVIDER_ROUTE,
            feature: "headless-platform-snapshot",
            note: "Headless platform route with synthetic surface metrics; no native window handles are available.",
        },
        _ => PlatformRouteIdentity {
            owner: PLATFORM_WINDOW_GATEWAY_OWNER,
            provider_route: PLATFORM_WINDOW_PROVIDER_ROUTE,
            feature: "native-window-snapshot",
            note: "Window-backed platform route with native handles and surface metrics.",
        },
    }
}

fn read_platform_window_snapshot(
    snapshot: &Arc<Mutex<PlatformWindowReadyV1>>,
) -> PlatformWindowReadyV1 {
    match snapshot.lock() {
        Ok(v) => *v,
        Err(e) => *e.into_inner(),
    }
}

fn platform_window_service(
    snapshot: Arc<Mutex<PlatformWindowReadyV1>>,
    identity: PlatformRouteIdentity,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = PlatformServiceInfo::default();
    let mut features = info.features.clone();
    features.push(identity.feature.to_owned());
    let description = engine_gateway_provider_service_description(
        ENGINE_PLATFORM_SERVICE_ID,
        identity.owner,
        PLATFORM_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .gateway(ENGINE_PLATFORM_SERVICE_ID)
    .protocol(info.protocol.clone())
    .features(features)
    .notes(identity.note);

    let window_snapshot = snapshot.clone();
    JsonServiceRouter::new(ENGINE_PLATFORM_SERVICE_ID)
        .describe_json(&description)
        .info(PlatformServiceInfo::default)
        .blob(PLATFORM_SERVICE_METHOD_INVOKE, |_unit, payload| {
            ok_json(serde_json::json!({
                "ok": false,
                "error": "engine.platform invoke_json has no generic command envelope yet",
                "payload_len": payload.as_slice().len()
            }))
        })
        .get_json(PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1, move |_| {
            read_platform_window_snapshot(&window_snapshot)
        })
        .shutdown_json(|_| serde_json::json!({ "ok": true }))
        .into_service_v1()
}

pub(crate) fn register_platform_window_service_best_effort(initial: PlatformWindowReadyV1) {
    let snapshot = PLATFORM_WINDOW_SNAPSHOT
        .get_or_init(|| Arc::new(Mutex::new(initial)))
        .clone();

    match snapshot.lock() {
        Ok(mut guard) => *guard = initial,
        Err(e) => *e.into_inner() = initial,
    }

    if newengine_core::has_engine_gateway_route(ENGINE_PLATFORM_SERVICE_ID) {
        return;
    }

    let identity = platform_route_identity(initial);
    let service = platform_window_service(snapshot, identity);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_PLATFORM_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Platform,
        provider_service: ENGINE_PLATFORM_SERVICE_ID,
        provider_route: identity.provider_route,
        capability: PLATFORM_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: identity.owner,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.platform gateway registered source=engine-runtime route='{}' service='{}' capability='{}'",
            identity.provider_route,
            ENGINE_PLATFORM_SERVICE_ID,
            PLATFORM_BACKEND_CAPABILITY_ID
        ),
        Err(e) => newengine_ulog_api::ulog::error!(
            "engine.platform gateway registration failed id='{}' err='{}'",
            ENGINE_PLATFORM_SERVICE_ID,
            e
        ),
    }
}

pub(crate) fn update_platform_window_snapshot(ready: PlatformWindowReadyV1) {
    if let Some(snapshot) = PLATFORM_WINDOW_SNAPSHOT.get() {
        match snapshot.lock() {
            Ok(mut guard) => *guard = ready,
            Err(e) => *e.into_inner() = ready,
        }
    }
}

#[cfg(test)]
mod loaded_provider_contract_tests {
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        crate_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("NorthStar repository root")
            .to_path_buf()
    }

    fn load_first_party(manager: &mut newengine_plugin_host::PluginManager, path: &Path) {
        assert!(path.is_file(), "missing runtime plugin {}", path.display());
        manager
            .load_path_with_origin(
                path,
                newengine_plugin_host::default_host_api(),
                newengine_plugin_host::PluginLoadOrigin::FirstPartyPlugin,
            )
            .unwrap_or_else(|error| panic!("load {} failed: {error}", path.display()));
    }

    #[test]
    fn loaded_headless_safe_first_party_provider_routes_conform_to_registry() {
        newengine_plugin_host::init_host_context();
        let runtime = repo_root().join("pluginsRuntime");
        let mut manager = newengine_plugin_host::PluginManager::new();

        // Vulkan is intentionally excluded here: its real init contract requires
        // a valid native Win32 window/surface. It is covered by descriptor
        // conformance plus the window-backed runtime smoke, not by fake handles.
        load_first_party(
            &mut manager,
            &runtime.join("gravitas-physics-0.3.0-release.dll"),
        );
        load_first_party(
            &mut manager,
            &runtime.join("egui-ui-0.1.0-release.dll"),
        );

        for (backend, abi, owner) in [
            (
                newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC,
                newengine_physics_api::PHYSICS_PROVIDER_ABI_CONTRACT_SPEC,
                "engine.physics.gravitas",
            ),
            (
                newengine_ui_api::UI_BACKEND_SERVICE_SPEC,
                newengine_ui_api::UI_PROVIDER_ABI_CONTRACT_SPEC,
                "engine.ui.egui",
            ),
        ] {
            let route = newengine_plugin_host::active_engine_gateway_route(backend.engine_gateway_id)
                .unwrap_or_else(|| panic!("missing active route {}", backend.engine_gateway_id));
            let report = newengine_contract_conformance::validate_active_route_abi(
                &route,
                backend,
                abi,
            )
            .unwrap_or_else(|errors| {
                panic!(
                    "loaded route {} failed conformance: {}",
                    backend.engine_gateway_id,
                    errors.join("; ")
                )
            });
            assert_eq!(report.provider_owner_id, owner);
            assert_eq!(route.origin, "first-party-plugin");
        }
    }
}

