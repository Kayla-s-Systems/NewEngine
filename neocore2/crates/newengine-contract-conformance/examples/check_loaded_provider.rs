use std::{env, path::PathBuf, process};

use newengine_plugin_host::{
    active_engine_gateway_route, default_host_api, init_host_context, PluginLoadOrigin,
    PluginManager,
};

fn main() {
    let mut args = env::args().skip(1);
    let dll = args.next().map(PathBuf::from).unwrap_or_else(|| usage());
    let domain = args.next().unwrap_or_else(|| usage());
    if !dll.is_file() {
        fail(format!("provider DLL not found: {}", dll.display()));
    }

    let (backend, abi) = match domain.as_str() {
        "render" => (
            newengine_render_api::RENDER_BACKEND_SERVICE_SPEC,
            newengine_render_api::RENDER_PROVIDER_ABI_CONTRACT_SPEC,
        ),
        "physics" => (
            newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC,
            newengine_physics_api::PHYSICS_PROVIDER_ABI_CONTRACT_SPEC,
        ),
        "ui" => (
            newengine_ui_api::UI_BACKEND_SERVICE_SPEC,
            newengine_ui_api::UI_PROVIDER_ABI_CONTRACT_SPEC,
        ),
        other => fail(format!("unsupported provider domain '{other}'")),
    };

    init_host_context();
    let mut manager = PluginManager::new();
    for prerequisite in args.map(PathBuf::from) {
        if !prerequisite.is_file() {
            fail(format!(
                "prerequisite DLL not found: {}",
                prerequisite.display()
            ));
        }
        manager
            .load_path_with_origin(
                &prerequisite,
                default_host_api(),
                PluginLoadOrigin::FirstPartyPlugin,
            )
            .unwrap_or_else(|error| {
                fail(format!(
                    "prerequisite load {} failed: {error}",
                    prerequisite.display()
                ))
            });
    }
    manager
        .load_path_with_origin(&dll, default_host_api(), PluginLoadOrigin::FirstPartyPlugin)
        .unwrap_or_else(|error| fail(format!("load {} failed: {error}", dll.display())));

    let route = active_engine_gateway_route(backend.engine_gateway_id).unwrap_or_else(|| {
        fail(format!(
            "loaded provider {} did not publish active gateway '{}'",
            dll.display(),
            backend.engine_gateway_id
        ))
    });
    let report = newengine_contract_conformance::validate_active_route_abi(&route, backend, abi)
        .unwrap_or_else(|errors| {
            fail(format!(
                "loaded provider {} route conformance failed: {}",
                dll.display(),
                errors.join("; ")
            ))
        });
    if route.origin != "first-party-plugin" {
        fail(format!(
            "loaded provider {} origin='{}' expected='first-party-plugin'",
            dll.display(),
            route.origin
        ));
    }
    println!(
        "PASS dll={} gateway={} owner={} service={} abi={} contract={} origin={}",
        dll.display(),
        report.gateway_id,
        report.provider_owner_id,
        report.provider_service_id,
        report.provider_abi,
        report.contract_key,
        route.origin,
    );
}

fn usage() -> ! {
    eprintln!(
        "usage: check_loaded_provider <provider.dll> <render|physics|ui> [prerequisite.dll ...]"
    );
    process::exit(2)
}

fn fail(message: String) -> ! {
    eprintln!("FAIL {message}");
    process::exit(1)
}
