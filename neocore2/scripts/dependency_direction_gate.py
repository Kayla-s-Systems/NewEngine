#!/usr/bin/env python3
"""Dependency direction gate for neocore2 composition layers.

This gate complements kernel_dependency_gate.py.  The kernel gate protects the
minimum floor; this gate protects direction between upper runtime/composition
layers so convenience dependencies cannot slowly turn the host back into a
monolithic game/editor executable.
"""
from __future__ import annotations

import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PLUGINS_SRC = ROOT.parents[1] / "PluginsSrc"

KERNEL_PACKAGES = {
    "newengine-core",
    "newengine-host-kernel",
    "newengine-plugin-host",
}

# Runtime-host may orchestrate contracts, generic host services and explicit
# runtime adapters.  Adding a new direct NewEngine dependency requires an
# intentional review/update here instead of silently expanding the host layer.
RUNTIME_HOST_ALLOWED_INTERNAL_DEPS = {
    "newengine-host-capabilities-api",
    "newengine-host-capabilities-runtime",
    "newengine-core",
    "newengine-console-runtime",  # optional engine.command provider
    "newengine-host-kernel",
    "newengine-startup-window-egui",  # optional concrete PreStart presenter
    "newengine-ulog-api",
    "newengine-assets",
    "newengine-assets-api",
    "newengine-asset-hot-reload-runtime",
    "newengine-asset-bootstrap-runtime",
    "newengine-math",
    "newengine-platform-api",
    "newengine-project-api",
    "newengine-project-runtime",
    "newengine-runtime-adapter-core",
    "newengine-render-runtime-adapter",
    "newengine-physics-runtime-adapter",
    "newengine-runtime-units",
    "newengine-scripting-api",
    "newengine-scripting-client",
    "newengine-task-api",
    "newengine-editor-command-api",  # contract only; editor implementations remain forbidden below
    "newengine-runtime-session-api",
    "newengine-runtime-session-runtime",
    "newengine-time-runtime",
    "newengine-input-api",
    "newengine-plugin-api",
    "newengine-service-api",
    "newengine-schema-api",
    "newengine-schema-runtime",
    "newengine-service-kit",
    "newengine-render-api",
    "newengine-physics-api",
    "newengine-system-runtime",
    "newengine-system-contracts",
    "newengine-plugin-host",
    "newengine-transform",
    "newengine-ui",
    "newengine-ui-api",
    "newengine-world-api",
}

RUNTIME_HOST_BANNED_FAMILIES = (
    "newengine-game-ready",
    "newengine-gameplay",
    "newengine-game-module",
    "newengine-game-events",
    "newengine-network-runtime",
    "newengine-replication-runtime",
)

RUNTIME_HOST_BANNED_SOURCE_MARKERS = (
    "newengine_game_ready_",
    "newengine_gameplay_",
    "newengine_game_module_",
    "newengine_game_events_",
    "newengine_network_runtime",
    "newengine_replication_runtime",
)

PROVIDER_REGISTRATION_MARKERS = (
    "EngineGatewayProviderDecl",
    "register_engine_gateway_provider_service_best_effort",
)
PROVIDER_AWARE_DEPS = {
    "newengine-plugin-api",
    "newengine-plugin-host",
    "newengine-service-kit",
}
PROVIDER_INFRASTRUCTURE_EXEMPT = {"newengine-service-kit"}


def load_manifest(path: Path) -> dict:
    with path.open("rb") as f:
        return tomllib.load(f)


def dependency_map(manifest: dict) -> dict[str, object]:
    out: dict[str, object] = dict(manifest.get("dependencies", {}))
    for target in manifest.get("target", {}).values():
        out.update(target.get("dependencies", {}))
    return out


def is_optional(spec: object) -> bool:
    return isinstance(spec, dict) and spec.get("optional") is True


def package_manifests() -> dict[str, Path]:
    result: dict[str, Path] = {}
    for manifest_path in ROOT.rglob("Cargo.toml"):
        # Build output/vendor trees are irrelevant architecture sources.
        if any(part in {"target", "Intermediate"} for part in manifest_path.parts):
            continue
        try:
            manifest = load_manifest(manifest_path)
        except (OSError, tomllib.TOMLDecodeError):
            continue
        package = manifest.get("package", {})
        name = package.get("name")
        if isinstance(name, str):
            result[name] = manifest_path
    return result


def is_game_or_server_distribution(name: str) -> bool:
    return (
        name.startswith("newengine-game-ready")
        or name.startswith("newengine-game-module")
        or name.startswith("newengine-gameplay")
        or name in {"newengine-network-runtime", "newengine-replication-runtime"}
        or "server" in name
    )


def is_editor_implementation(name: str) -> bool:
    # Editor contracts may be shared, editor executable/runtime implementations may not.
    return "editor" in name and not name.endswith("-api") and "command-api" not in name


def provider_route_extends_gateway_parent(gateway_id: str, provider_route_id: str) -> bool:
    """Mirror the Host route invariant used when provider metadata is loaded."""
    if provider_route_id == gateway_id:
        return False

    gateway_parts = gateway_id.split(".")
    provider_parts = provider_route_id.split(".")
    if len(gateway_parts) < 2 or len(provider_parts) <= len(gateway_parts):
        return False
    if (
        gateway_parts[0] != "engine"
        or provider_parts[0] != "engine"
        or gateway_parts[1] != provider_parts[1]
    ):
        return False

    if provider_parts[: len(gateway_parts)] == gateway_parts:
        return True

    child_tail = gateway_parts[2:]
    provider_tail = provider_parts[3:]
    return provider_tail[: len(child_tail)] == child_tail


def main() -> int:
    errors: list[str] = []
    manifests = package_manifests()

    # Every external provider declaration must use the same route namespace
    # rule as the Host. Invalid package metadata otherwise survives the build
    # and is rejected only when the dynamic library is loaded.
    external_provider_count = 0
    external_routes: dict[str, Path] = {}
    if not PLUGINS_SRC.is_dir():
        errors.append(f"external plugin source root is missing: {PLUGINS_SRC}")
    else:
        for provider_manifest in sorted(PLUGINS_SRC.rglob("Cargo.toml")):
            if any(part in {"target", "Intermediate"} for part in provider_manifest.parts):
                continue
            try:
                manifest = load_manifest(provider_manifest)
            except (OSError, tomllib.TOMLDecodeError):
                continue
            provider = (
                manifest.get("package", {})
                .get("metadata", {})
                .get("northstar", {})
                .get("provider")
            )
            if not isinstance(provider, dict):
                continue

            external_provider_count += 1
            label = provider_manifest.relative_to(PLUGINS_SRC)
            route = provider.get("route")
            gateway = provider.get("implements")
            if not isinstance(route, str) or not route.strip():
                errors.append(f"{label}: external provider route must be a non-empty string")
                continue
            if not isinstance(gateway, str) or not gateway.strip():
                errors.append(f"{label}: external provider implements must be a non-empty string")
                continue
            if not provider_route_extends_gateway_parent(gateway, route):
                errors.append(
                    f"{label}: provider route '{route}' does not extend gateway '{gateway}'"
                )

            previous = external_routes.get(route)
            if previous is not None:
                errors.append(
                    f"duplicate external provider route '{route}': "
                    f"{previous.relative_to(PLUGINS_SRC)} and {label}"
                )
            else:
                external_routes[route] = provider_manifest

    # 1) Gameplay direction is strictly downward: gameplay/compositions may use
    # the kernel, but the kernel must never know gameplay packages.
    for kernel_name in sorted(KERNEL_PACKAGES):
        path = manifests.get(kernel_name)
        if path is None:
            errors.append(f"missing kernel package manifest: {kernel_name}")
            continue
        deps = dependency_map(load_manifest(path))
        leaked = sorted(
            dep for dep in deps
            if dep.startswith("newengine-game")
            or dep.startswith("newengine-network")
            or dep.startswith("newengine-replication")
        )
        if leaked:
            errors.append(f"{kernel_name}: gameplay/network dependency points into kernel: {', '.join(leaked)}")

    # 2) Runtime host is an orchestration/adapters layer, not a dumping ground for
    # arbitrary engine/game implementations.
    runtime_host_path = manifests["newengine-runtime-host"]
    runtime_host_manifest = load_manifest(runtime_host_path)
    runtime_host_deps = dependency_map(runtime_host_manifest)
    internal = {name for name in runtime_host_deps if name.startswith("newengine-")}
    unexpected = sorted(internal - RUNTIME_HOST_ALLOWED_INTERNAL_DEPS)
    if unexpected:
        errors.append("newengine-runtime-host: direct dependency outside orchestration allowlist: " + ", ".join(unexpected))
    banned = sorted(
        name for name in internal if any(name.startswith(prefix) for prefix in RUNTIME_HOST_BANNED_FAMILIES)
    )
    if banned:
        errors.append("newengine-runtime-host: gameplay/composition implementation dependency: " + ", ".join(banned))

    runtime_src = runtime_host_path.parent / "src"
    for source in runtime_src.rglob("*.rs"):
        body = source.read_text(encoding="utf-8", errors="replace")
        for marker in RUNTIME_HOST_BANNED_SOURCE_MARKERS:
            if marker in body:
                errors.append(
                    f"newengine-runtime-host: gameplay/composition source reference '{marker}' in {source.relative_to(ROOT)}"
                )

    # 3) The command console contract is provider-neutral and its implementation
    # remains an optional upper provider, never a kernel-owned built-in.
    console_api = manifests.get("newengine-console-api")
    console_runtime = manifests.get("newengine-console-runtime")
    if console_api is None or console_runtime is None:
        errors.append("missing engine.command console API/runtime package")
    else:
        console_api_deps = set(dependency_map(load_manifest(console_api)))
        console_api_internal = sorted(dep for dep in console_api_deps if dep.startswith("newengine-"))
        if console_api_internal:
            errors.append(
                "newengine-console-api: must remain provider-neutral DTO contract; internal deps: "
                + ", ".join(console_api_internal)
            )
        console_runtime_deps = set(dependency_map(load_manifest(console_runtime)))
        required_console_runtime_deps = {
            "newengine-console-api",
            "newengine-core",
            "newengine-service-kit",
        }
        missing_console_runtime_deps = sorted(required_console_runtime_deps - console_runtime_deps)
        if missing_console_runtime_deps:
            errors.append(
                "newengine-console-runtime: missing provider boundary dependencies: "
                + ", ".join(missing_console_runtime_deps)
            )
        leaked_console_domains = sorted(
            dep for dep in console_runtime_deps
            if dep.startswith("newengine-game")
            or dep.startswith("newengine-network")
            or dep.startswith("newengine-replication")
        )
        if leaked_console_domains:
            errors.append(
                "newengine-console-runtime: gameplay/network dependency forbidden: "
                + ", ".join(leaked_console_domains)
            )

    # 4) Host capability discovery is an upper host layer: the DTO crate stays
    # implementation-free and the concrete runtime may not depend downward on
    # newengine-core/runtime-host/plugin-host or any game/domain runtime.
    host_caps_api = manifests.get("newengine-host-capabilities-api")
    host_caps_runtime = manifests.get("newengine-host-capabilities-runtime")
    if host_caps_api is None or host_caps_runtime is None:
        errors.append("missing HostCapabilities API/runtime package")
    else:
        api_deps = set(dependency_map(load_manifest(host_caps_api)))
        api_internal = sorted(dep for dep in api_deps if dep.startswith("newengine-"))
        if api_internal:
            errors.append(
                "newengine-host-capabilities-api: must remain pure DTO contract; internal deps: "
                + ", ".join(api_internal)
            )
        runtime_deps = set(dependency_map(load_manifest(host_caps_runtime)))
        forbidden_host_cap_runtime = sorted(
            runtime_deps.intersection({
                "newengine-core",
                "newengine-host-kernel",
                "newengine-runtime-host",
                "newengine-plugin-host",
                "newengine-engine-runtime",
                "newengine-game-ready-profile",
            })
        )
        if forbidden_host_cap_runtime:
            errors.append(
                "newengine-host-capabilities-runtime: upward/domain dependency forbidden: "
                + ", ".join(forbidden_host_cap_runtime)
            )

    # 5) GameReady consumes map semantics through the stable gateway. The
    # implementation and provider route belong to a standalone plugin artifact,
    # so replacing engine.assets.maps never requires rebuilding the profile.
    maps_runtime_path = manifests.get("newengine-maps-runtime")
    game_ready_path = manifests.get("newengine-game-ready-profile")
    if maps_runtime_path is None or game_ready_path is None:
        errors.append("missing maps runtime or GameReady profile package")
    else:
        maps_runtime_deps = set(dependency_map(load_manifest(maps_runtime_path)))
        if "newengine-service-api" in maps_runtime_deps:
            errors.append(
                "newengine-maps-runtime: service factory must not own host registration dependencies"
            )
        maps_runtime_source = "\n".join(
            source.read_text(encoding="utf-8", errors="replace")
            for source in (maps_runtime_path.parent / "src").rglob("*.rs")
        )
        if "register_maps_gateway_best_effort" in maps_runtime_source:
            errors.append(
                "newengine-maps-runtime: must expose a service factory, not self-register engine.assets.maps"
            )

        game_ready_deps = set(dependency_map(load_manifest(game_ready_path)))
        if "newengine-maps-runtime" in game_ready_deps:
            errors.append(
                "newengine-game-ready-profile: concrete maps runtime dependency is forbidden"
            )
        provider_routes = (
            game_ready_path.parent / "src" / "provider_routes.rs"
        ).read_text(encoding="utf-8", errors="replace")
        if "newengine_assets_api::MAPS_BACKEND_SERVICE_SPEC.capability()" not in provider_routes:
            errors.append(
                "newengine-game-ready-profile: required maps capability must use typed MAPS_BACKEND_SERVICE_SPEC"
            )
        if "newengine_maps_runtime" in provider_routes:
            errors.append(
                "newengine-game-ready-profile: concrete maps provider registration is forbidden"
            )

    maps_plugin_manifest = (
        PLUGINS_SRC / "MapsRuntime" / "newengine-maps-provider" / "Cargo.toml"
    )
    maps_plugin_source = (
        PLUGINS_SRC / "MapsRuntime" / "newengine-maps-provider" / "src" / "lib.rs"
    )
    build_manifest = PLUGINS_SRC / "build_manifest.json"
    for required_path in (maps_plugin_manifest, maps_plugin_source, build_manifest):
        if not required_path.is_file():
            errors.append(
                f"standalone engine.assets.maps provider artifact missing: {required_path}"
            )
    if maps_plugin_manifest.is_file():
        plugin_manifest = load_manifest(maps_plugin_manifest)
        provider = (
            plugin_manifest.get("package", {})
            .get("metadata", {})
            .get("northstar", {})
            .get("provider", {})
        )
        if provider.get("route") != "engine.assets.maps.discrete":
            errors.append("MapsRuntime: provider route metadata is not canonical")
        if provider.get("implements") != "engine.assets.maps":
            errors.append("MapsRuntime: provider must implement engine.assets.maps")
    if maps_plugin_source.is_file():
        plugin_source = maps_plugin_source.read_text(
            encoding="utf-8", errors="replace"
        )
        for source_marker in (
            "MAPS_BACKEND_SERVICE_SPEC",
            "CapabilityDesc::backend_route",
            "maps_gateway_service",
            "ENGINE_ASSET_SERVICE_ID",
        ):
            if source_marker not in plugin_source:
                errors.append(
                    f"MapsRuntime: standalone provider missing '{source_marker}'"
                )
    if build_manifest.is_file() and '"MapsRuntime"' not in build_manifest.read_text(
        encoding="utf-8", errors="replace"
    ):
        errors.append("PluginsSrc build manifest does not include MapsRuntime")

    # Texture dictionary semantics follow the same factory/plugin split.
    textures_runtime_path = manifests.get("newengine-textures-runtime")
    asset_inspector_path = manifests.get("asset-inspector")
    if textures_runtime_path is None:
        errors.append("missing newengine-textures-runtime service factory")
    else:
        textures_runtime_deps = set(
            dependency_map(load_manifest(textures_runtime_path))
        )
        if "newengine-service-api" in textures_runtime_deps:
            errors.append(
                "newengine-textures-runtime: service factory must not own host registration dependencies"
            )
        textures_runtime_source = "\n".join(
            source.read_text(encoding="utf-8", errors="replace")
            for source in (textures_runtime_path.parent / "src").rglob("*.rs")
        )
        if "register_textures_gateway_best_effort" in textures_runtime_source:
            errors.append(
                "newengine-textures-runtime: must expose a service factory, not self-register engine.assets.textures"
            )

    if game_ready_path is not None:
        game_ready_deps = set(dependency_map(load_manifest(game_ready_path)))
        if "newengine-textures-runtime" in game_ready_deps:
            errors.append(
                "newengine-game-ready-profile: concrete textures runtime dependency is forbidden"
            )
        provider_routes = (
            game_ready_path.parent / "src" / "provider_routes.rs"
        ).read_text(encoding="utf-8", errors="replace")
        if "newengine_assets_api::TEXTURES_BACKEND_SERVICE_SPEC.capability()" not in provider_routes:
            errors.append(
                "newengine-game-ready-profile: required textures capability must use typed TEXTURES_BACKEND_SERVICE_SPEC"
            )
        if "newengine_textures_runtime" in provider_routes:
            errors.append(
                "newengine-game-ready-profile: concrete textures provider registration is forbidden"
            )

    if asset_inspector_path is None:
        errors.append("missing AssetInspector package")
    else:
        inspector_deps = set(dependency_map(load_manifest(asset_inspector_path)))
        if "newengine-textures-runtime" in inspector_deps:
            errors.append(
                "asset-inspector: concrete textures runtime dependency is forbidden"
            )
        inspector_source = "\n".join(
            source.read_text(encoding="utf-8", errors="replace")
            for source in (asset_inspector_path.parent / "src").rglob("*.rs")
        )
        if "newengine_textures_runtime" in inspector_source:
            errors.append(
                "asset-inspector: must consume engine.assets.textures, not install a concrete provider"
            )

    textures_plugin_manifest = (
        PLUGINS_SRC
        / "TexturesRuntime"
        / "newengine-textures-provider"
        / "Cargo.toml"
    )
    textures_plugin_source = (
        PLUGINS_SRC
        / "TexturesRuntime"
        / "newengine-textures-provider"
        / "src"
        / "lib.rs"
    )
    for required_path in (textures_plugin_manifest, textures_plugin_source):
        if not required_path.is_file():
            errors.append(
                f"standalone engine.assets.textures provider artifact missing: {required_path}"
            )
    if textures_plugin_manifest.is_file():
        plugin_manifest = load_manifest(textures_plugin_manifest)
        provider = (
            plugin_manifest.get("package", {})
            .get("metadata", {})
            .get("northstar", {})
            .get("provider", {})
        )
        if provider.get("route") != "engine.assets.textures.ytd":
            errors.append("TexturesRuntime: provider route metadata is not canonical")
        if provider.get("implements") != "engine.assets.textures":
            errors.append(
                "TexturesRuntime: provider must implement engine.assets.textures"
            )
    if textures_plugin_source.is_file():
        plugin_source = textures_plugin_source.read_text(
            encoding="utf-8", errors="replace"
        )
        for source_marker in (
            "TEXTURES_BACKEND_SERVICE_SPEC",
            "CapabilityDesc::backend_route",
            "textures_gateway_service",
            "ENGINE_ASSET_SERVICE_ID",
        ):
            if source_marker not in plugin_source:
                errors.append(
                    f"TexturesRuntime: standalone provider missing '{source_marker}'"
                )
    if build_manifest.is_file() and '"TexturesRuntime"' not in build_manifest.read_text(
        encoding="utf-8", errors="replace"
    ):
        errors.append("PluginsSrc build manifest does not include TexturesRuntime")

    game_ready_plugin_source = (
        PLUGINS_SRC
        / "GameReadyRuntime"
        / "newengine-runtime-profile-gameready"
        / "src"
        / "lib.rs"
    )
    if not game_ready_plugin_source.is_file():
        errors.append("GameReady runtime-profile plugin source is missing")
    else:
        runtime_profile_plugin = game_ready_plugin_source.read_text(
            encoding="utf-8", errors="replace"
        )
        for required_gateway in (
            "ENGINE_ASSETS_MAPS_SERVICE_ID",
            "ENGINE_ASSETS_TEXTURES_SERVICE_ID",
        ):
            if required_gateway not in runtime_profile_plugin:
                errors.append(
                    f"GameReady runtime-profile descriptor missing required gateway {required_gateway}"
                )
        if runtime_profile_plugin.count(".requires_service(") < 2:
            errors.append(
                "GameReady runtime-profile descriptor must require maps and textures gateways"
            )

    # 6) Host consumers must disable package defaults explicitly. This keeps
    # product composition opt-in even if someone later attempts to broaden the
    # runtime-host default feature set.
    for consumer_name, consumer_path in sorted(manifests.items()):
        if consumer_name == "newengine-runtime-host":
            continue
        consumer_deps = dependency_map(load_manifest(consumer_path))
        host_spec = consumer_deps.get("newengine-runtime-host")
        if host_spec is None:
            continue
        if not isinstance(host_spec, dict) or host_spec.get("default-features") is not False:
            errors.append(
                f"{consumer_name}: newengine-runtime-host dependency must set default-features = false"
            )

    # 5) Domain runtimes already detached from the Host must never grow an upward
    # dependency back to process/bootstrap orchestration.
    for detached_runtime in ("newengine-engine-runtime", "newengine-world-runtime", "newengine-scene-runtime"):
        detached_path = manifests.get(detached_runtime)
        if detached_path is None:
            errors.append(f"missing detached runtime package manifest: {detached_runtime}")
            continue
        detached_deps = dependency_map(load_manifest(detached_path))
        if "newengine-runtime-host" in detached_deps:
            errors.append(f"{detached_runtime}: upward dependency to newengine-runtime-host is forbidden")

    # 6) A first-party crate that registers an engine capability route must be
    # explicitly provider-aware.  This prevents invisible capability
    # implementations from appearing in ordinary data/helper crates.
    provider_crates: list[str] = []
    for name, manifest_path in sorted(manifests.items()):
        src = manifest_path.parent / "src"
        if not src.exists():
            continue
        registers_provider = False
        for source in src.rglob("*.rs"):
            body = source.read_text(encoding="utf-8", errors="replace")
            if any(marker in body for marker in PROVIDER_REGISTRATION_MARKERS):
                registers_provider = True
                break
        if not registers_provider or name in PROVIDER_INFRASTRUCTURE_EXEMPT:
            continue
        provider_crates.append(name)
        deps = set(dependency_map(load_manifest(manifest_path)))
        if not deps.intersection(PROVIDER_AWARE_DEPS):
            errors.append(
                f"{name}: registers engine capability provider but has no plugin/provider infrastructure dependency"
            )
        if name in KERNEL_PACKAGES:
            errors.append(f"{name}: kernel package must not implement an engine capability provider")

    # 7) Game/server distributions cannot require editor implementation crates.
    # API/command contracts are allowed; concrete editor runtimes/executables are not.
    checked_distributions: list[str] = []
    for name, manifest_path in sorted(manifests.items()):
        if not is_game_or_server_distribution(name):
            continue
        checked_distributions.append(name)
        for dep, spec in dependency_map(load_manifest(manifest_path)).items():
            if is_editor_implementation(dep) and not is_optional(spec):
                errors.append(f"{name}: required editor implementation dependency '{dep}'")

    if errors:
        print("DEPENDENCY DIRECTION GATE: FAIL")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("DEPENDENCY DIRECTION GATE: PASS")
    print("  kernel -> gameplay/network: forbidden")
    print("  runtime-host -> reviewed orchestration/API/adapter allowlist only")
    print("  engine.command -> optional provider-neutral console API/runtime boundary")
    print("  engine.assets.maps -> standalone replaceable provider; GameReady consumes gateway only")
    print("  engine.assets.textures -> standalone replaceable provider; profiles/apps consume gateway only")
    print(f"  external provider routes checked against Host namespace: {external_provider_count}")
    print(f"  capability provider registrars checked: {len(provider_crates)}")
    print(f"  game/server distributions checked for editor implementation deps: {len(checked_distributions)}")
    print("  editor contract APIs may be shared; editor implementations are not required by game/server packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
