#!/usr/bin/env python3
"""Hard architecture gate for the neocore2 runtime host kernel.

The kernel owns lifecycle, scheduling, services, contracts and plugin composition.
Concrete engine/game domains must remain above it and occupy capability slots
through provider routes.
"""
from __future__ import annotations

import sys
import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

KERNEL_MANIFESTS = {
    "newengine-core": ROOT / "crates/newengine-core/Cargo.toml",
    "newengine-host-kernel": ROOT / "crates/newengine-host-kernel/Cargo.toml",
    "newengine-plugin-host": ROOT / "crates/newengine-plugin-host/Cargo.toml",
}

# Implementation crates are never valid dependencies of the host/kernel layer.
BANNED_IMPLEMENTATION_DEPS = {
    "newengine-assets",
    "newengine-asset-hot-reload-runtime",
    "newengine-engine-runtime",
    "newengine-host-capabilities-runtime",
    "newengine-game-events-runtime",
    "newengine-game-module-runtime",
    "newengine-game-ready-profile",
    "newengine-game-ready-world",
    "newengine-gameplay-runtime",
    "newengine-network-runtime",
    "newengine-replication-runtime",
    "newengine-null-providers-runtime",
    "newengine-render-feature-api",
    "newengine-render-feature-gameready",
    "newengine-render-feature-standard",
    "newengine-world-runtime",
    "newengine-ecs-runtime",
    "newengine-entity-runtime",
    "newengine-camera-runtime",
    "newengine-material-runtime",
    "newengine-model-runtime",
    "newengine-ui",
    "newengine-startup-window-egui",
    "newengine-console-runtime",
}

BANNED_KERNEL_UI_TOOLKIT_DEPS = {"eframe", "egui", "winit"}

# Domain contract/type crates may be referenced by newengine-core only as explicit
# opt-in features; they may not increase the default/minimum kernel graph.
CORE_OPTIONAL_DOMAIN_APIS = {
    "newengine-render-api",
    "newengine-physics-api",
    "newengine-platform-api",
    "newengine-assets-api",
    "newengine-audio-api",
    "newengine-camera-api",
    "newengine-ecs-api",
    "newengine-entity-api",
    "newengine-ui-api",
    "newengine-input-api",
    "newengine-input-bindings-api",
    "newengine-scene-io",
    "newengine-world-api",
    "newengine-materials",
}

HOST_KERNEL_ALLOWED_DEPS = {
    "crossbeam-channel",
    "newengine-core",
    "newengine-plugin-host",
}

RUNTIME_HOST_MINIMUM_DEPS = {
    "newengine-core",
    "newengine-host-kernel",
}

BANNED_SOURCE_MARKERS = {
    "newengine_gameplay_runtime",
    "newengine_game_events_runtime",
    "newengine_network_runtime",
    "newengine_replication_runtime",
    "newengine_game_ready_profile",
    "newengine_game_ready_world",
    "newengine_engine_runtime",
    "newengine_null_providers_runtime",
}


def load_manifest(path: Path) -> dict:
    with path.open("rb") as f:
        return tomllib.load(f)


def deps(manifest: dict) -> dict:
    out = dict(manifest.get("dependencies", {}))
    for target in manifest.get("target", {}).values():
        out.update(target.get("dependencies", {}))
    return out


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def main() -> int:
    errors: list[str] = []

    for name, manifest_path in KERNEL_MANIFESTS.items():
        manifest = load_manifest(manifest_path)
        dependency_map = deps(manifest)
        banned = sorted(BANNED_IMPLEMENTATION_DEPS.intersection(dependency_map))
        if banned:
            fail(errors, f"{name}: concrete implementation dependencies in kernel: {', '.join(banned)}")

    core = load_manifest(KERNEL_MANIFESTS["newengine-core"])
    core_deps = deps(core)
    leaked_ui_toolkits = sorted(BANNED_KERNEL_UI_TOOLKIT_DEPS.intersection(core_deps))
    if leaked_ui_toolkits:
        fail(
            errors,
            "newengine-core: UI toolkit dependency leaked into kernel/core: " + ", ".join(leaked_ui_toolkits),
        )
    core_features = core.get("features", {})
    if "host-probe" in core_features:
        fail(errors, "newengine-core: hardware host-probe feature returned to kernel/core")
    if "sysinfo" in core_deps:
        fail(errors, "newengine-core: sysinfo hardware discovery dependency returned to kernel/core")
    core_probe_file = ROOT / "crates/newengine-core/src/startup/system_probe.rs"
    core_probe_dir = ROOT / "crates/newengine-core/src/startup/system_probe"
    if core_probe_file.exists() or core_probe_dir.exists():
        fail(errors, "newengine-core: OS/hardware SystemProbe returned to kernel/core")

    default_features = core.get("features", {}).get("default", [])
    if default_features:
        fail(errors, f"newengine-core: default features must stay empty, got {default_features!r}")

    for dep_name in sorted(CORE_OPTIONAL_DOMAIN_APIS):
        spec = core_deps.get(dep_name)
        if spec is None:
            continue
        if not isinstance(spec, dict) or spec.get("optional") is not True:
            fail(errors, f"newengine-core: domain API '{dep_name}' must be optional")

    host_kernel = load_manifest(KERNEL_MANIFESTS["newengine-host-kernel"])
    host_kernel_deps = set(deps(host_kernel))
    unexpected = sorted(host_kernel_deps - HOST_KERNEL_ALLOWED_DEPS)
    if unexpected:
        fail(errors, f"newengine-host-kernel: unexpected minimum dependencies: {', '.join(unexpected)}")

    runtime_host_manifest = load_manifest(ROOT / "crates/newengine-runtime-host/Cargo.toml")
    runtime_host_deps = deps(runtime_host_manifest)
    runtime_host_default_features = runtime_host_manifest.get("features", {}).get("default", [])
    if runtime_host_default_features:
        fail(
            errors,
            f"newengine-runtime-host: default features must stay empty for Void/Engine Host, got {runtime_host_default_features!r}",
        )
    non_optional_runtime_host = {
        name
        for name, spec in runtime_host_deps.items()
        if not (isinstance(spec, dict) and spec.get("optional") is True)
    }
    unexpected_runtime_host = sorted(non_optional_runtime_host - RUNTIME_HOST_MINIMUM_DEPS)
    if unexpected_runtime_host:
        fail(
            errors,
            "newengine-runtime-host: non-optional dependencies outside minimum floor: "
            + ", ".join(unexpected_runtime_host),
        )

    bootstrap_source = (ROOT / "crates/newengine-runtime-host/src/app_launcher/bootstrap.rs").read_text(encoding="utf-8")
    preinit_pos = bootstrap_source.find("crate::preinit::run_host_preinit()")
    engine_build = re.search(r"self\.build_engine\s*\(\s*&startup\b", bootstrap_source)
    engine_build_pos = engine_build.start() if engine_build else -1
    if preinit_pos < 0 or engine_build_pos < 0 or preinit_pos >= engine_build_pos:
        fail(errors, "newengine-runtime-host: Host PreInit must execute before Engine construction")
    preinit_source = (ROOT / "crates/newengine-runtime-host/src/preinit.rs").read_text(encoding="utf-8")
    if "discover_preinit_snapshot()" in preinit_source:
        fail(
            errors,
            "newengine-runtime-host: PreInit directly constructs native HostCapabilities instead of calling engine.host.capabilities",
        )
    if "ENGINE_HOST_CAPABILITIES_GATEWAY_ID" not in preinit_source or "call_service_v1(" not in preinit_source:
        fail(
            errors,
            "newengine-runtime-host: HostCapabilities must resolve through the engine.host.capabilities gateway",
        )
    snapshot_insert_pos = bootstrap_source.find("insert(Arc::clone(&host_preinit))")
    composition_pos = bootstrap_source.find("initialize_composition_services")
    if snapshot_insert_pos < 0 or composition_pos < 0 or snapshot_insert_pos >= composition_pos:
        fail(errors, "newengine-runtime-host: immutable HostPreInitSnapshot must be inserted before runtime composition")

    core_prestart_egui_file = ROOT / "crates/newengine-core/src/startup_window/egui_presenter.rs"
    core_prestart_egui_dir = ROOT / "crates/newengine-core/src/startup_window/egui_presenter"
    if core_prestart_egui_file.exists() or core_prestart_egui_dir.exists():
        fail(errors, "newengine-core: concrete Egui PreStart presenter returned to core source tree")

    core_console_dir = ROOT / "crates/newengine-core/src/console"
    if core_console_dir.exists():
        fail(errors, "newengine-core: command console implementation returned to core source tree")
    core_engine_source = (ROOT / "crates/newengine-core/src/engine/core.rs").read_text(encoding="utf-8")
    if "init_console_service" in core_engine_source or "install_console_provider" in core_engine_source:
        fail(errors, "newengine-core: Engine construction must not install a command console provider")

    runtime_host_lib = (ROOT / "crates/newengine-runtime-host/src/lib.rs").read_text(encoding="utf-8")
    for moved_module in ("pub mod render_runtime", "pub mod physics_runtime", "mod service_runtime"):
        if moved_module in runtime_host_lib:
            fail(errors, f"newengine-runtime-host: moved adapter module returned: {moved_module}")

    empty_host_source = (ROOT / "crates/newengine-host-kernel/src/lib.rs").read_text(encoding="utf-8")
    if ".with_implicit_plugin_discovery(false)" not in empty_host_source:
        fail(errors, "newengine-host-kernel: build_empty_host must disable implicit plugin discovery")

    # Source-level guard prevents a developer from bypassing Cargo policy with a
    # newly added direct implementation reference in kernel source.
    for crate_name in ("newengine-core", "newengine-host-kernel"):
        src_root = ROOT / "crates" / crate_name / "src"
        for source in src_root.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            for marker in BANNED_SOURCE_MARKERS:
                if marker in text:
                    fail(errors, f"{crate_name}: banned implementation reference '{marker}' in {source.relative_to(ROOT)}")

    # The old null provider implementation must not silently return to runtime-host.
    runtime_host = ROOT / "crates/newengine-runtime-host/src"
    for source in runtime_host.rglob("*.rs"):
        text = source.read_text(encoding="utf-8", errors="replace")
        if "register_null_provider_routes_best_effort" in text or "mod null_providers" in text:
            fail(errors, f"newengine-runtime-host: implicit null provider wiring found in {source.relative_to(ROOT)}")

    if errors:
        print("KERNEL DEPENDENCY GATE: FAIL")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("KERNEL DEPENDENCY GATE: PASS")
    print("  newengine-core default feature set: empty")
    print("  hardware discovery: provider-routed through engine.host.capabilities before Engine construction")
    print("  newengine-host-kernel: minimum dependency allowlist satisfied")
    print("  newengine-runtime-host default composition: empty (core + host-kernel floor only)")
    print("  render/physics/service adapters: physically outside runtime-host")
    print("  concrete domain implementations: absent from kernel manifests/source")
    print("  engine.command console provider: absent from empty Host and core source")
    print("  implicit discovery/null-provider wiring: absent from empty host")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
