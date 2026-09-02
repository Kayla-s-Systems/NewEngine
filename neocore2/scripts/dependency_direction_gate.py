#!/usr/bin/env python3
"""Dependency direction gate for neocore2 composition layers.

This gate complements kernel_dependency_gate.py.  The kernel gate protects the
minimum floor; this gate protects direction between upper runtime/composition
layers so convenience dependencies cannot slowly turn the host back into a
monolithic game/editor executable.
"""
from __future__ import annotations

import json
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
    'newengine-runtime-unit-api',  # provider-neutral static runtime-unit registration contract
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

    # 5) GameReady consumes map semantics through the stable gateway. Map ownership follows
    # the StarVault architecture: `.ymap` identity/selector metadata is a format module,
    # while AssetManager owns the engine.assets.maps service route and composes the internal
    # semantic runtime factory. There is deliberately no standalone MapsRuntime plugin DLL.
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
                "newengine-maps-runtime: must remain an internal semantic service factory, not self-register engine.assets.maps"
            )
        for required_marker in (
            "maps_gateway_service",
            "ENGINE_ASSETS_MAPS_SERVICE_ID",
            "AssetServiceClient",
        ):
            if required_marker not in maps_runtime_source:
                errors.append(
                    f"newengine-maps-runtime: semantic service factory missing '{required_marker}'"
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
                "newengine-game-ready-profile: concrete maps implementation reference is forbidden"
            )

    asset_manager_manifest = PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "Cargo.toml"
    asset_manager_workspace_manifest = PLUGINS_SRC / "AssetManager" / "Cargo.toml"
    asset_manager_maps = (
        PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "maps.rs"
    )
    asset_manager_plugin = (
        PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "mod.rs"
    )
    asset_manager_lifecycle = (
        PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "lifecycle.rs"
    )
    ymap_format_manifest = PLUGINS_SRC / "formats" / "ymap" / "Cargo.toml"
    ymap_format_descriptor = PLUGINS_SRC / "formats" / "ymap" / "src" / "descriptor.rs"
    build_manifest = PLUGINS_SRC / "build_manifest.json"

    for required_path in (
        asset_manager_manifest,
        asset_manager_workspace_manifest,
        asset_manager_maps,
        asset_manager_plugin,
        asset_manager_lifecycle,
        ymap_format_manifest,
        ymap_format_descriptor,
        build_manifest,
    ):
        if not required_path.is_file():
            errors.append(f"AssetManager/maps ownership artifact missing: {required_path}")

    if asset_manager_manifest.is_file():
        asset_manager_deps = set(dependency_map(load_manifest(asset_manager_manifest)))
        if "newengine-maps-runtime" not in asset_manager_deps:
            errors.append(
                "AssetManager: internal engine.assets.maps semantic runtime dependency is missing"
            )
    if asset_manager_workspace_manifest.is_file():
        workspace_manifest = load_manifest(asset_manager_workspace_manifest)
        workspace_deps = set(
            workspace_manifest.get("workspace", {}).get("dependencies", {})
        )
        if "newengine-maps-runtime" not in workspace_deps:
            errors.append(
                "AssetManager workspace: newengine-maps-runtime path dependency is missing"
            )
    if asset_manager_maps.is_file():
        maps_source = asset_manager_maps.read_text(encoding="utf-8", errors="replace")
        for source_marker in (
            "MAPS_BACKEND_SERVICE_SPEC",
            "BackendRouteDescriptor",
            "maps_gateway_service",
            "ENGINE_ASSET_SERVICE_ID",
            'PROVIDER_ROUTE_ID: &str = "engine.assets.maps.discrete"',
        ):
            if source_marker not in maps_source:
                errors.append(f"AssetManager maps route missing '{source_marker}'")
    if asset_manager_plugin.is_file():
        plugin_source = asset_manager_plugin.read_text(encoding="utf-8", errors="replace")
        for source_marker in (
            "MAPS_BACKEND_CAPABILITY_ID",
            "MAPS_SERVICE_ID",
            "maps::backend_route()",
        ):
            if source_marker not in plugin_source:
                errors.append(f"AssetManager descriptor missing maps capability marker '{source_marker}'")
    if asset_manager_lifecycle.is_file():
        lifecycle_source = asset_manager_lifecycle.read_text(encoding="utf-8", errors="replace")
        if "super::maps::register(&host)" not in lifecycle_source:
            errors.append("AssetManager lifecycle does not register engine.assets.maps")
    if ymap_format_descriptor.is_file():
        ymap_source = ymap_format_descriptor.read_text(encoding="utf-8", errors="replace")
        for source_marker in (
            'MODULE_ID: &str = "engine.assets.formats.maps.ymap"',
            'SEMANTIC_GATEWAY: &str = "engine.assets.maps"',
            'HANDLER_SERVICE: &str = "asset.codec.listfile"',
            'default_entry_gateway: "engine.assets.maps"',
        ):
            if source_marker not in ymap_source:
                errors.append(f"YMAP format descriptor missing '{source_marker}'")

    retired_maps_plugin = PLUGINS_SRC / "MapsRuntime"
    if retired_maps_plugin.exists():
        errors.append(
            f"standalone MapsRuntime plugin is forbidden; maps belong to AssetManager + formats: {retired_maps_plugin}"
        )
    if build_manifest.is_file():
        build_manifest_data = json.loads(build_manifest.read_text(encoding="utf-8", errors="replace"))
        for key in ("plugins", "runtimePlugins"):
            if "MapsRuntime" in build_manifest_data.get(key, []):
                errors.append(
                    f"PluginsSrc build manifest must not include standalone MapsRuntime in {key}"
                )

    # Texture runtime provider retirement. `.ytd` format identity is a StarVault-private
    # format module and runtime payloads decode through engine.assets::asset.decode_v1.
    # No standalone texture service/provider may re-enter the composition.
    asset_inspector_path = manifests.get("asset-inspector")
    retired_texture_crate = ROOT / "crates" / "newengine-textures-runtime"
    if retired_texture_crate.exists():
        errors.append(f"retired newengine-textures-runtime crate still exists: {retired_texture_crate}")

    retired_texture_plugin = PLUGINS_SRC / "TexturesRuntime"
    if retired_texture_plugin.exists():
        errors.append(f"retired TexturesRuntime plugin workspace still exists: {retired_texture_plugin}")

    if build_manifest.is_file():
        build_manifest_data = json.loads(build_manifest.read_text(encoding="utf-8", errors="replace"))
        for key in ("plugins", "runtimePlugins"):
            if "TexturesRuntime" in build_manifest_data.get(key, []):
                errors.append(f"PluginsSrc build manifest still includes retired TexturesRuntime in {key}")

    assets_api_source = ROOT / "crates" / "newengine-assets-api" / "src" / "lib.rs"
    if assets_api_source.is_file():
        assets_api_src = assets_api_source.parent
        assets_api_text = "\n".join(
            source.read_text(encoding="utf-8", errors="replace")
            for source in sorted(assets_api_src.rglob("*.rs"))
        )
        for retired_marker in (
            "TEXTURES_BACKEND_CAPABILITY_ID",
            "TEXTURES_BACKEND_SERVICE_SPEC",
            "TEXTURES_RUNTIME_CONTRACT_SPEC",
            "TEXTURES_RUNTIME_REQUIREMENT_SPEC",
            "pub const TEXTURES_SERVICE_ID",
        ):
            if retired_marker in assets_api_text:
                errors.append(f"newengine-assets-api still exposes retired texture service marker '{retired_marker}'")

    texture_client = ROOT / "crates" / "newengine-assets-api" / "src" / "asset_service_client" / "textures.rs"
    if texture_client.is_file():
        texture_client_text = texture_client.read_text(encoding="utf-8", errors="replace")
        for required_marker in ("AssetDecodeRequest", "self.decode_v1", "texture_decode_selector"):
            if required_marker not in texture_client_text:
                errors.append(f"texture client canonical StarVault decode path missing '{required_marker}'")
        if "call_service_typed" in texture_client_text and "ENGINE_ASSETS_TEXTURES_SERVICE_ID" in texture_client_text:
            errors.append("texture client must not call a standalone engine.assets.textures service")

    if game_ready_path is not None:
        game_ready_deps = set(dependency_map(load_manifest(game_ready_path)))
        if "newengine-textures-runtime" in game_ready_deps:
            errors.append("newengine-game-ready-profile depends on retired newengine-textures-runtime")
        provider_routes = (game_ready_path.parent / "src" / "provider_routes.rs").read_text(
            encoding="utf-8", errors="replace"
        )
        for retired_marker in ("TEXTURES_BACKEND_SERVICE_SPEC", "assets.textures.backend", "newengine_textures_runtime"):
            if retired_marker in provider_routes:
                errors.append(f"newengine-game-ready-profile still declares retired texture provider marker '{retired_marker}'")

    runtime_catalog = ROOT / "crates" / "newengine-core" / "src" / "startup" / "api_contracts" / "catalog.rs"
    if runtime_catalog.is_file() and "TEXTURES_RUNTIME_REQUIREMENT_SPEC" in runtime_catalog.read_text(encoding="utf-8", errors="replace"):
        errors.append("runtime service catalog still validates retired engine.assets.textures service")

    for consumer in (
        ROOT / "crates" / "newengine-game-ready-world" / "src" / "game_ready_parts" / "terrain_heightmap.rs",
        ROOT / "crates" / "newengine-windowed-host-runtime" / "src" / "platform_runtime" / "ui_gateway_frame_parts" / "draw_list_texture.rs",
    ):
        if consumer.is_file():
            body = consumer.read_text(encoding="utf-8", errors="replace")
            if "ENGINE_ASSETS_TEXTURES_SERVICE_ID" in body:
                errors.append(f"texture consumer still calls retired service route: {consumer}")
            if "textures_entry_rgba8_ref_v1_typed" not in body:
                errors.append(f"texture consumer is not on canonical AssetServiceClient decode path: {consumer}")

    if asset_inspector_path is None:
        errors.append("missing AssetInspector package")
    else:
        inspector_deps = set(dependency_map(load_manifest(asset_inspector_path)))
        if "newengine-textures-runtime" in inspector_deps:
            errors.append("asset-inspector depends on retired newengine-textures-runtime")
        inspector_source = "\n".join(
            source.read_text(encoding="utf-8", errors="replace")
            for source in (asset_inspector_path.parent / "src").rglob("*.rs")
        )
        if "newengine_textures_runtime" in inspector_source:
            errors.append("asset-inspector references retired newengine-textures-runtime")

    formats_workspace = PLUGINS_SRC / "formats"
    formats_manifest = formats_workspace / "Cargo.toml"
    format_api_source = formats_workspace / "newengine-format-api" / "src" / "lib.rs"
    if not formats_manifest.is_file():
        errors.append(f"StarVault format workspace missing: {formats_manifest}")
    if not format_api_source.is_file():
        errors.append(f"StarVault format ABI source missing: {format_api_source}")
    else:
        format_api_text = format_api_source.read_text(encoding="utf-8", errors="replace")
        for marker in (
            "FORMAT_ABI_VERSION_V1",
            "northstar_asset_format_root_v1",
            "AssetFormatModuleRootV1",
            "descriptor_json",
        ):
            if marker not in format_api_text:
                errors.append(f"StarVault format ABI missing '{marker}'")

    discovered_format_modules: dict[str, Path] = {}
    if formats_workspace.is_dir():
        for module_manifest in sorted(formats_workspace.glob("*/Cargo.toml")):
            module_root = module_manifest.parent
            if module_root.name == "newengine-format-api":
                continue
            try:
                module_data = load_manifest(module_manifest)
            except Exception as error:
                errors.append(f"StarVault format manifest unreadable: {module_manifest}: {error}")
                continue
            package = module_data.get("package", {})
            package_name = str(package.get("name", ""))
            crate_types = module_data.get("lib", {}).get("crate-type", [])
            if not package_name.startswith("newengine-format-") or "cdylib" not in crate_types:
                continue
            extension = package_name.removeprefix("newengine-format-").strip().lower()
            if not extension:
                errors.append(f"StarVault format package has empty extension identity: {module_manifest}")
                continue
            if extension in discovered_format_modules:
                errors.append(f"duplicate StarVault format extension '.{extension}'")
                continue
            discovered_format_modules[extension] = module_root

            module_descriptor = module_root / "src" / "descriptor.rs"
            module_lib = module_root / "src" / "lib.rs"
            for required_path in (module_descriptor, module_lib):
                if not required_path.is_file():
                    errors.append(f"StarVault format '.{extension}' artifact missing: {required_path}")

            lib_name = str(module_data.get("lib", {}).get("name", ""))
            if lib_name != extension:
                errors.append(
                    f"StarVault format '.{extension}' library must install as {extension}.dll/.so/.dylib; lib.name='{lib_name}'"
                )
            manifest_text = module_manifest.read_text(encoding="utf-8", errors="replace")
            if "northstar.provider" in manifest_text:
                errors.append(f"StarVault format '.{extension}' must not be an engine runtime plugin")

            if module_descriptor.is_file():
                descriptor_text = module_descriptor.read_text(encoding="utf-8", errors="replace")
                for marker in (
                    "AssetFormatDescriptorSpecV1",
                    "module_id:",
                    "handler_service:",
                    "semantic_gateway:",
                    "content_kind:",
                ):
                    if marker not in descriptor_text:
                        errors.append(f"StarVault format '.{extension}' descriptor missing '{marker}'")
                if "engine.assets.formats." not in descriptor_text:
                    errors.append(
                        f"StarVault format '.{extension}' descriptor must own an engine.assets.formats.* module id"
                    )
            if module_lib.is_file():
                lib_text = module_lib.read_text(encoding="utf-8", errors="replace")
                if "export_asset_format_module_v1" not in lib_text:
                    errors.append(f"StarVault format '.{extension}' does not export format-module ABI root")

    if not discovered_format_modules:
        errors.append("StarVault format workspace contains no dynamically discoverable format cdylibs")

    build_formats_script = formats_workspace / "build_formats.py"
    if not build_formats_script.is_file():
        errors.append(f"StarVault format build script missing: {build_formats_script}")
    else:
        build_formats_text = build_formats_script.read_text(encoding="utf-8", errors="replace")
        if "discover_formats" not in build_formats_text:
            errors.append("StarVault format build must discover format crates dynamically")
        if "FORMATS =" in build_formats_text:
            errors.append("StarVault format build must not contain a manual FORMATS registry")

    starvault_manifest = PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "Cargo.toml"
    starvault_format_loader = (
        PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "format_loader.rs"
    )
    starvault_config = (
        PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "config.rs"
    )
    if starvault_manifest.is_file():
        starvault_text = starvault_manifest.read_text(encoding="utf-8", errors="replace")
        if 'version = "3.8.0"' not in starvault_text:
            errors.append("StarVault format-module discovery requires engine-assets-starvault 3.8.0")
        if "newengine-format-api" not in starvault_text:
            errors.append("StarVault 3.8.0 must depend on newengine-format-api")
    if not starvault_format_loader.is_file():
        errors.append(f"StarVault format loader missing: {starvault_format_loader}")
    else:
        loader_text = starvault_format_loader.read_text(encoding="utf-8", errors="replace")
        for marker in (
            "FORMAT_ROOT_SYMBOL_V1",
            "FORMAT_ABI_VERSION_V1",
            "AssetFileTypeDescriptor",
            "register_asset_type_descriptor_best_effort",
            "format directory declared",
        ):
            if marker not in loader_text:
                errors.append(f"StarVault format loader missing '{marker}'")
    if starvault_config.is_file():
        config_text = starvault_config.read_text(encoding="utf-8", errors="replace")
        if 'formats_dir: PathBuf::from("formats")' not in config_text:
            errors.append("StarVault formats_dir must default to relative 'formats'")

    asset_types_runtime = ROOT / "crates" / "newengine-asset-types-runtime" / "src" / "lib.rs"
    if asset_types_runtime.is_file():
        asset_types_text = asset_types_runtime.read_text(encoding="utf-8", errors="replace")
        if "newengine_asset_format_nef8::descriptors()" in asset_types_text:
            errors.append("asset-types runtime must not statically register NEF8 descriptors; StarVault format modules own descriptors")
        if "register_asset_types_gateway_best_effort" in asset_types_text:
            errors.append("asset-types runtime must be readiness-only; RuntimeHost bootstrap owns asset.types.api creation")
        if "asset_types_gateway_service_seeded" in asset_types_text:
            errors.append("asset-types runtime must not seed or create asset.types.api")

    runtime_host_plugins = ROOT / "crates" / "newengine-runtime-host" / "src" / "app_launcher" / "plugins.rs"
    if not runtime_host_plugins.is_file():
        errors.append(f"RuntimeHost plugin bootstrap source missing: {runtime_host_plugins}")
    else:
        runtime_host_plugins_text = runtime_host_plugins.read_text(encoding="utf-8", errors="replace")
        if runtime_host_plugins_text.count("register_asset_types_gateway_best_effort()") != 1:
            errors.append("RuntimeHost bootstrap must contain exactly one asset.types.api registration call")
        for marker in (
            "engine.assets.host.types",
            "ensure_host_asset_types_registry()?",
            "before any first-party plugin",
        ):
            if marker not in runtime_host_plugins_text:
                errors.append(f"RuntimeHost asset-types bootstrap missing '{marker}'")

    asset_types_call_sites: list[str] = []
    for source in ROOT.rglob("*.rs"):
        body = source.read_text(encoding="utf-8", errors="replace")
        for line_no, line in enumerate(body.splitlines(), 1):
            if "register_asset_types_gateway_best_effort()" in line and not line.lstrip().startswith("pub fn "):
                asset_types_call_sites.append(f"{source}:{line_no}")
    if len(asset_types_call_sites) != 1:
        errors.append(
            "asset.types.api must have exactly one production registration site in RuntimeHost bootstrap; got "
            + repr(asset_types_call_sites)
        )

    windowed_host_source = ROOT / "crates" / "newengine-windowed-host-runtime" / "src" / "platform_runtime" / "runtime_host.rs"
    if windowed_host_source.is_file() and "register_asset_types_gateway_best_effort()" in windowed_host_source.read_text(encoding="utf-8", errors="replace"):
        errors.append("windowed host must not re-register asset.types.api; common RuntimeHost owns bootstrap ordering")

    asset_inspector_source = ROOT / "apps" / "AssetInspector" / "src" / "main.rs"
    if asset_inspector_source.is_file():
        asset_inspector_text = asset_inspector_source.read_text(encoding="utf-8", errors="replace")
        if "register_asset_types_gateway_best_effort()" in asset_inspector_text:
            errors.append("AssetInspector must not create asset.types.api; common RuntimeHost owns it")
        if "newengine_asset_format_nef8::descriptors()" in asset_inspector_text:
            errors.append("AssetInspector must not seed a static NEF8 format table")

    starvault_plugin_source = PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "mod.rs"
    starvault_types_source = PLUGINS_SRC / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "plugin" / "types.rs"
    if starvault_types_source.exists():
        errors.append("StarVault plugin/types.rs must not exist; asset.types.api is host-owned")
    if starvault_plugin_source.is_file():
        starvault_plugin_text = starvault_plugin_source.read_text(encoding="utf-8", errors="replace")
        production_text = starvault_plugin_text.split("#[cfg(test)]", 1)[0]
        for forbidden in (
            ".provides_service(ASSET_TYPES_SERVICE_ID",
            "ASSET_TYPES_BACKEND_CAPABILITY_ID",
            "mod types;",
            "types::register",
        ):
            if forbidden in production_text:
                errors.append(f"StarVault must not provide host-owned asset-types registry: found '{forbidden}'")

    asset_type_registry_service = ROOT / "crates" / "newengine-assets" / "src" / "asset_type_registry" / "service.rs"
    if asset_type_registry_service.is_file():
        registry_service_text = asset_type_registry_service.read_text(encoding="utf-8", errors="replace")
        if 'provider_route: "engine.assets.host.types"' not in registry_service_text:
            errors.append("host-owned asset-types registry route must be engine.assets.host.types")
        if 'provider_route: "engine.assets.starvault.types"' in registry_service_text:
            errors.append("host-owned asset-types registry must not use a StarVault provider route")

    if (PLUGINS_SRC / "AssetFormatsRuntime").exists():
        errors.append("retired AssetFormatsRuntime still exists; format modules belong in PluginsSrc/formats")

    if build_manifest.is_file():
        build_manifest_data = json.loads(build_manifest.read_text(encoding="utf-8", errors="replace"))
        if "AssetFormatsRuntime" in build_manifest_data.get("plugins", []):
            errors.append("PluginsSrc build manifest still includes retired AssetFormatsRuntime engine plugin")
        if "AssetFormatsRuntime" in build_manifest_data.get("runtimePlugins", []):
            errors.append("PluginsSrc runtime manifest still includes retired AssetFormatsRuntime engine plugin")
        if "formatModules" in build_manifest_data:
            errors.append(
                "PluginsSrc build manifest must not enumerate formatModules; PluginsSrc/formats/* is discovered dynamically"
            )

    # File-format dynamic libraries are valid only as StarVault-private modules.
    # They must never declare NorthStar engine-plugin provider metadata.
    forbidden_extension_plugins = {
        "textures-ytd",
        "models-ydd",
        "definitions-ytyp",
        "materials-nemat",
        "animations-ycd",
    }
    for plugin_manifest_path in PLUGINS_SRC.rglob("Cargo.toml"):
        try:
            candidate = load_manifest(plugin_manifest_path)
        except Exception:
            continue
        candidate_provider = (
            candidate.get("package", {})
            .get("metadata", {})
            .get("northstar", {})
            .get("provider", {})
        )
        install_name = candidate_provider.get("install_name")
        if install_name in forbidden_extension_plugins:
            errors.append(
                f"plugin granularity violation: '{install_name}' is an engine plugin; use StarVault-private pluginsRuntime/formats module ABI ({plugin_manifest_path})"
            )

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
        for required_gateway in ("ENGINE_ASSETS_MAPS_SERVICE_ID",):
            if required_gateway not in runtime_profile_plugin:
                errors.append(
                    f"GameReady runtime-profile descriptor missing required gateway {required_gateway}"
                )
        if "ENGINE_ASSETS_TEXTURES_SERVICE_ID" in runtime_profile_plugin:
            errors.append("GameReady runtime-profile descriptor still requires retired engine.assets.textures service")
        if runtime_profile_plugin.count(".requires_service(") < 1:
            errors.append(
                "GameReady runtime-profile descriptor must require maps and textures gateways"
            )

    # P0 Project-Driven boundary: a runtime profile may compose generic capabilities,
    # but it must never know concrete project, character, weapon or mission identities.
    # Runtime-profile product identities are forbidden outright; world-runtime character
    # identity is separately enforced below with a case-insensitive zero-tolerance gate.
    project_specific_markers = (
        "GameReadyFPS",
        "game-ready-fps.world",
        "Joel",
        "Abby",
        "Malorian",
        "BrutalismRoom",
        "SeattleStadium",
        "Projects/",
        "spawn_game_ready_mission",
    )

    project_driven_profile_sources = []
    if game_ready_path is not None:
        project_driven_profile_sources.extend(
            source
            for source in (game_ready_path.parent / "src").rglob("*.rs")
            if source.is_file()
        )
    if game_ready_plugin_source.is_file():
        project_driven_profile_sources.append(game_ready_plugin_source)

    for source in project_driven_profile_sources:
        text = source.read_text(encoding="utf-8", errors="replace")
        leaked = sorted(marker for marker in project_specific_markers if marker in text)
        if leaked:
            errors.append(
                "project-driven runtime-profile violation: "
                f"{source} contains product/project marker(s): {', '.join(leaked)}"
            )

    game_ready_world_path = manifests.get("newengine-game-ready-world")
    if game_ready_world_path is not None:
        world_src = game_ready_world_path.parent / "src"
        fixed_world_identity = "game-ready-fps.world"
        for source in world_src.rglob("*.rs"):
            if fixed_world_identity in source.read_text(encoding="utf-8", errors="replace"):
                errors.append(
                    "project-driven world-runtime violation: "
                    f"{source} hard-codes '{fixed_world_identity}'; world identity must come from authored project/map data"
                )

        # P0.4 closed: character-specific runtime identity is zero-tolerance.
        # Character content belongs to Project Content/definitions; no migration allowlist remains.
        for source in world_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            if "abby" in text.casefold():
                errors.append(
                    "project-driven character-content regression: "
                    f"{source} contains character identity 'Abby'; authored character data must remain in Project Content"
                )

        # P0.5 closed: authored world/mission domain types must not carry product-profile identity.
        retired_world_domain_markers = (
            "GameReadyMapProfile",
            "GameReadyMission",
            "RawGameReadyPayload",
            "load_game_ready_map_profile",
            "game-ready.mode-defaults",
        )
        for source in world_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(marker for marker in retired_world_domain_markers if marker in text)
            if leaked:
                errors.append(
                    "project-driven authored-domain regression: "
                    f"{source} contains retired product-domain marker(s): {', '.join(leaked)}"
                )

        # P0.7 strangler cut: authored map DTO/cell preparation ownership lives in newengine-authored-world-runtime.
        retired_streaming_markers = ("GameReadyAuthoredMapStreamingSpec", "GameReadyPrefabSpec")
        streaming_root = world_src / "game_ready_parts" / "world_model"
        for source in streaming_root.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(marker for marker in retired_streaming_markers if marker in text)
            if leaked:
                errors.append(
                    "project-driven authored-world ownership regression: "
                    f"{source} contains retired GameReady streaming DTO(s): {', '.join(leaked)}"
                )
            gateway_leaks = sorted(
                marker
                for marker in ("ENGINE_ASSETS_MAPS_SERVICE_ID", "ENGINE_ASSETS_DEFINITIONS_SERVICE_ID")
                if marker in text
            )
            if gateway_leaks:
                errors.append(
                    "project-driven authored-world gateway regression: "
                    f"{source} directly accesses authored map/definition gateways: {', '.join(gateway_leaks)}; use newengine-authored-world-runtime"
                )

        # P0.7.5 closed: prediction/residency/preparation scheduling belongs to the
        # generic authored-world controller. GameReady may retain only ECS/material
        # admission and presentation residency bookkeeping.
        game_ready_streaming_adapter = streaming_root / "authored_map_streaming.rs"
        if game_ready_streaming_adapter.is_file():
            adapter_text = game_ready_streaming_adapter.read_text(
                encoding="utf-8", errors="replace"
            )
            retired_controller_markers = (
                "struct CellLoadJob",
                "AuthoredMapDefinitionCache",
                "prepare_authored_map_cell(",
                "fn prediction_for_player(",
                "fn desired_domains(",
                "fn prepared_priority(",
                "fn submit_cell_jobs(",
                "fn poll_cell_jobs(",
                "fn prepare_cells_synchronously(",
                "TaskRequest::new(\"authored.map.cell.prepare\")",
                "desired_render:",
                "desired_simulation:",
                "pending_cells:",
                "load_jobs:",
                "ready_cells:",
                "failed_cells:",
                "last_center:",
                "last_predicted_center:",
            )
            leaked = sorted(
                marker for marker in retired_controller_markers if marker in adapter_text
            )
            if leaked:
                errors.append(
                    "project-driven authored-world controller regression: "
                    f"{game_ready_streaming_adapter} reclaims generic streaming ownership: {', '.join(leaked)}"
                )
            for required_marker in (
                "AuthoredMapStreamingController",
                ".controller.replan(",
                ".controller.process_preparation(",
                ".controller.take_next_prepared(",
            ):
                if required_marker not in adapter_text:
                    errors.append(
                        "project-driven authored-world controller regression: "
                        f"{game_ready_streaming_adapter} missing generic controller delegation {required_marker!r}"
                    )

    # P0.7.8-B authored-world scene-admission cut: ECS/material/static-YDD admission now
    # belongs to newengine-authored-world-runtime. The GameReady world package must not
    # recreate a world_model implementation or a transitional authored-world runtime adapter.
    if game_ready_world_path is not None:
        retired_world_model_file = world_src / "game_ready_parts" / "world_model.rs"
        retired_world_model_dir = world_src / "game_ready_parts" / "world_model"
        for retired_path in (retired_world_model_file, retired_world_model_dir):
            if retired_path.exists():
                errors.append(
                    "project-driven authored-world scene-admission regression: "
                    f"{retired_path} must not exist; scene admission belongs to newengine-authored-world-runtime"
                )
        retired_admission_markers = (
            "GameReadyAuthoredWorldStreamingAdapter",
            "GameReadyAuthoredMapStreamingState",
            "GameReadyAuthoredMapCellRoots",
            "GameReadyAuthoredMapPrimitiveResidency",
            "GameReadyStaticWorldStreamingState",
            "tick_game_ready_static_world_prefabs",
        )
        for source in world_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(marker for marker in retired_admission_markers if marker in text)
            if leaked:
                errors.append(
                    "project-driven authored-world scene-admission regression: "
                    f"{source} contains retired GameReady admission marker(s): {', '.join(leaked)}"
                )

    authored_world_path = manifests.get("newengine-authored-world-runtime")
    if authored_world_path is not None:
        authored_world_src = authored_world_path.parent / "src"
        scene_admission_source = authored_world_src / "scene_admission.rs"
        scene_admission_dir = authored_world_src / "scene_admission"
        authored_runtime_source = authored_world_src / "world_runtime.rs"
        if not scene_admission_source.is_file() or not scene_admission_dir.is_dir():
            errors.append(
                "project-driven authored-world scene-admission regression: "
                "newengine-authored-world-runtime must own scene_admission.rs and its implementation directory"
            )
        else:
            admission_text = scene_admission_source.read_text(encoding="utf-8", errors="replace")
            admission_text += "\n" + "\n".join(
                source.read_text(encoding="utf-8", errors="replace")
                for source in scene_admission_dir.rglob("*.rs")
            )
            for required_marker in (
                "AuthoredMapSceneStreamingState",
                "AuthoredMapCellRoots",
                "AuthoredMapPrimitiveResidency",
                "AuthoredStaticWorldStreamingState",
                "tick_authored_static_world_prefabs",
            ):
                if required_marker not in admission_text:
                    errors.append(
                        "project-driven authored-world scene-admission regression: "
                        f"newengine-authored-world-runtime missing generic admission marker {required_marker!r}"
                    )
        if not authored_runtime_source.is_file() or "install_default_authored_world_streaming_runtime_adapter" not in authored_runtime_source.read_text(encoding="utf-8", errors="replace"):
            errors.append(
                "project-driven authored-world runtime regression: "
                "newengine-authored-world-runtime must install its concrete authored scene-streaming adapter"
            )

    # P0.7.8-B character-presentation ownership: reusable FPS character presentation,
    # authored avatar DTOs, hair/skin, weapon presentation and their tests live in
    # newengine-fps-character-runtime. GameReady may consume the public boundary only.
    if game_ready_world_path is not None:
        retired_character_paths = (
            world_src / "game_ready_parts" / "player_model.rs",
            world_src / "game_ready_parts" / "player_model",
            world_src / "game_ready_parts" / "player_model_animation.rs",
            world_src / "game_ready_parts" / "player_model_animation",
            world_src / "game_ready_parts" / "player_model_assets.rs",
            world_src / "game_ready_parts" / "player_model_binding.rs",
            world_src / "game_ready_parts" / "player_model_sidecar.rs",
            world_src / "game_ready_parts" / "player_model_validation.rs",
            world_src / "game_ready_parts" / "player_hair.rs",
            world_src / "game_ready_parts" / "equipment_visual.rs",
            world_src / "game_ready_parts" / "equipment_visual",
            world_src / "game_ready_parts" / "weapon_animation.rs",
            world_src / "game_ready_parts" / "weapon_casing.rs",
            world_src / "game_ready_parts" / "weapon_grip.rs",
            world_src / "game_ready_parts" / "weapon_grip",
            world_src / "game_ready_parts" / "impact_debris.rs",
            world_src / "game_ready_parts" / "vfx_decal_materials.rs",
        )
        for retired_path in retired_character_paths:
            if retired_path.exists():
                errors.append(
                    "project-driven character-presentation regression: "
                    f"{retired_path} must not exist; FPS character presentation belongs to newengine-fps-character-runtime"
                )
        retired_character_markers = (
            "GameReadyCharacterPresentationAdapter",
            "spawn_game_ready_player_model",
            "player_model::tick_player_model_assignments",
            "equipment_visual::tick_equipped_weapon_visuals",
            "weapon_animation::tick_equipped_weapon_animations",
        )
        for source in world_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(marker for marker in retired_character_markers if marker in text)
            if leaked:
                errors.append(
                    "project-driven character-presentation regression: "
                    f"{source} contains retired GameReady character presentation marker(s): {', '.join(leaked)}"
                )

    fps_character_path = manifests.get("newengine-fps-character-runtime")
    if fps_character_path is not None:
        fps_character_src = fps_character_path.parent / "src"
        presentation_runtime_source = fps_character_src / "presentation_runtime.rs"
        authored_presentation_source = fps_character_src / "authored_presentation.rs"
        required_presentation_markers = (
            "struct FpsCharacterPresentationRuntime",
            "install_fps_character_presentation_runtime",
            "crate::player_model::tick_player_model_assignments",
            "crate::equipment_visual::tick_equipped_weapon_visuals",
            "crate::weapon_animation::tick_equipped_weapon_animations",
        )
        if not presentation_runtime_source.is_file():
            errors.append(
                "project-driven character-presentation regression: "
                "newengine-fps-character-runtime must own presentation_runtime.rs"
            )
        else:
            text = presentation_runtime_source.read_text(encoding="utf-8", errors="replace")
            missing = [marker for marker in required_presentation_markers if marker not in text]
            if missing:
                errors.append(
                    "project-driven character-presentation regression: "
                    f"{presentation_runtime_source} missing concrete domain presentation markers: {missing}"
                )
        if not authored_presentation_source.is_file() or "pub struct AuthoredPlayerModelSpec" not in authored_presentation_source.read_text(encoding="utf-8", errors="replace"):
            errors.append(
                "project-driven character-presentation regression: "
                "newengine-fps-character-runtime must own AuthoredPlayerModelSpec"
            )
        for source in fps_character_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            if "GameReady" in text:
                errors.append(
                    "project-driven character-presentation identity regression: "
                    f"{source} contains product identity 'GameReady' inside reusable FPS character runtime"
                )

    fps_content_path = manifests.get("newengine-fps-content-runtime")
    if fps_content_path is not None:
        fps_content_src = fps_content_path.parent / "src"
        for source in fps_content_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            if "pub struct GameReadyPlayerModelSpec" in text:
                errors.append(
                    "project-driven character-presentation DTO regression: "
                    f"{source} redefines GameReadyPlayerModelSpec; AuthoredPlayerModelSpec belongs to newengine-fps-character-runtime"
                )

    # P0.7.8-B mission/objective ownership: authored FPS mission spawning and world-item
    # presentation live in newengine-fps-content-runtime; objective event evaluation remains in
    # newengine-fps-objective-runtime. GameReady may only call the public FPS-content boundary.
    if game_ready_world_path is not None:
        retired_mission_paths = (
            world_src / "game_ready_parts" / "mission.rs",
            world_src / "game_ready_parts" / "mission",
        )
        for retired_path in retired_mission_paths:
            if retired_path.exists():
                errors.append(
                    "project-driven mission/objective ownership regression: "
                    f"{retired_path} must not exist; authored FPS mission content belongs to newengine-fps-content-runtime"
                )
        retired_mission_markers = (
            "GameReadyFpsContentRuntimeAdapter",
            "game-ready.mission",
            "super::mission::instantiate_authored_mission",
            "mission::tick_deferred_item_pickups",
            "mission::tick_runtime_world_item_visuals",
        )
        for source in world_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(marker for marker in retired_mission_markers if marker in text)
            if leaked:
                errors.append(
                    "project-driven mission/objective ownership regression: "
                    f"{source} contains retired GameReady mission/content marker(s): {', '.join(leaked)}"
                )

    if fps_content_path is not None:
        fps_content_src = fps_content_path.parent / "src"
        mission_source = fps_content_src / "mission.rs"
        mission_world_items = fps_content_src / "mission" / "world_items.rs"
        content_runtime_source = fps_content_src / "world_runtime.rs"
        if not mission_source.is_file() or not mission_world_items.is_file():
            errors.append(
                "project-driven mission/objective ownership regression: "
                "newengine-fps-content-runtime must own mission.rs and mission/world_items.rs"
            )
        else:
            # Mission ownership is a module-tree invariant, not a physical single-file layout.
            # Structural decomposition may move implementation into mission/*.rs while preserving
            # the same newengine-fps-content-runtime ownership boundary.
            mission_sources = [mission_source, *sorted((fps_content_src / "mission").rglob("*.rs"))]
            mission_text = "\n".join(
                source.read_text(encoding="utf-8", errors="replace")
                for source in mission_sources
            )
            for required_marker in (
                "pub fn instantiate_authored_mission(",
                'MISSION_STREAMING_PIN_OWNER: &str = "fps.content.mission"',
                "tick_deferred_item_pickups",
                "tick_runtime_world_item_visuals",
            ):
                if required_marker not in mission_text:
                    errors.append(
                        "project-driven mission/objective ownership regression: "
                        f"newengine-fps-content-runtime mission implementation missing {required_marker!r}"
                    )
        if not content_runtime_source.is_file():
            errors.append(
                "project-driven mission/objective runtime regression: "
                "newengine-fps-content-runtime must own world_runtime.rs"
            )
        else:
            runtime_text = content_runtime_source.read_text(encoding="utf-8", errors="replace")
            for required_marker in (
                "struct FpsContentRuntime",
                "install_fps_content_world_runtime",
                "crate::mission::tick_deferred_item_pickups",
                "crate::mission::tick_runtime_world_item_visuals",
            ):
                if required_marker not in runtime_text:
                    errors.append(
                        "project-driven mission/objective runtime regression: "
                        f"{content_runtime_source} missing concrete FPS-content runtime marker {required_marker!r}"
                    )

    # P0.7.7 closed: world runtime orchestration is a set of ordered domain contributions.
    # The profile must not regain a monolithic GameReadyWorldRuntimeProvider, and the world
    # package must not reintroduce top-level tick_prelaunch/tick_frame orchestration entrypoints.
    game_ready_world_lib = world_src / "lib.rs" if game_ready_world_path is not None else None
    if game_ready_world_lib is not None and game_ready_world_lib.is_file():
        world_lib_text = game_ready_world_lib.read_text(encoding="utf-8", errors="replace")
        for forbidden in (
            "pub fn tick_prelaunch(",
            "pub fn tick_frame(",
            "GameReadyFrameTiming",
        ):
            if forbidden in world_lib_text:
                errors.append(
                    "project-driven world-runtime orchestration regression: "
                    f"{game_ready_world_lib} reintroduces monolithic marker {forbidden!r}"
                )
        if "world_runtime_contributions" in world_lib_text:
            errors.append(
                "project-driven world-runtime ownership regression: "
                f"{game_ready_world_lib} must not export an aggregate runtime contribution set; providers belong to domain runtimes"
            )

    # P0.7.8-B final bootstrap assembly / P0.7.9 retirement:
    # FPS authored scene composition is owned by the selected FPS game module. The former
    # newengine-game-ready-world compatibility crate is retired and must not return.
    retired_game_ready_world_root = ROOT / "crates" / "newengine-game-ready-world"
    if retired_game_ready_world_root.exists():
        errors.append(
            "project-driven final bootstrap assembly regression: "
            f"{retired_game_ready_world_root} must not exist; FPS authored scene composition belongs to newengine-game-module-fps"
        )

    game_module_fps_path = manifests.get("newengine-game-module-fps")
    if game_module_fps_path is None:
        errors.append("project-driven final bootstrap assembly regression: missing newengine-game-module-fps")
    else:
        game_module_fps_src = game_module_fps_path.parent / "src"
        game_module_fps_lib = game_module_fps_src / "lib.rs"
        assembly_root = game_module_fps_src / "authored_world_assembly.rs"
        assembly_dir = game_module_fps_src / "authored_world_assembly"
        required_assembly_files = (
            assembly_root,
            assembly_dir / "assets_bootstrap.rs",
            assembly_dir / "runtime_contributions.rs",
            assembly_dir / "assets_bootstrap" / "mesh_assets.rs",
        )
        missing_assembly = [str(path) for path in required_assembly_files if not path.is_file()]
        if missing_assembly:
            errors.append(
                "project-driven final bootstrap assembly regression: "
                f"newengine-game-module-fps is missing authored-world assembly file(s): {missing_assembly}"
            )
        if game_module_fps_lib.is_file():
            game_module_fps_text = game_module_fps_lib.read_text(encoding="utf-8", errors="replace")
            for required_marker in (
                "mod authored_world_assembly;",
                "FpsAuthoredWorldAssemblyContributor",
                "authored_world_assembly::bootstrap_authored_fps_scene_with_resolved_map(",
            ):
                if required_marker not in game_module_fps_text:
                    errors.append(
                        "project-driven final bootstrap assembly regression: "
                        f"{game_module_fps_lib} missing local FPS assembly marker {required_marker!r}"
                    )
            if "newengine_game_ready_world" in game_module_fps_text:
                errors.append(
                    "project-driven final bootstrap assembly regression: "
                    f"{game_module_fps_lib} references retired newengine_game_ready_world"
                )
        game_module_manifest_text = game_module_fps_path.read_text(encoding="utf-8", errors="replace")
        if "newengine-game-ready-world" in game_module_manifest_text:
            errors.append(
                "project-driven final bootstrap assembly regression: "
                f"{game_module_fps_path} depends on retired newengine-game-ready-world"
            )
        for assembly_source in (
            [assembly_root] + list(assembly_dir.rglob("*.rs"))
            if assembly_root.is_file() and assembly_dir.is_dir()
            else []
        ):
            assembly_text = assembly_source.read_text(encoding="utf-8", errors="replace")
            leaked = [
                marker
                for marker in ("GameReady", "game-ready", "newengine_game_ready_world", "game_ready_parts")
                if marker in assembly_text
            ]
            if leaked:
                errors.append(
                    "project-driven final bootstrap assembly regression: "
                    f"{assembly_source} contains retired product-world marker(s): {', '.join(leaked)}"
                )

    for package_name, manifest_path in manifests.items():
        if package_name == "newengine-game-ready-world":
            continue
        if "newengine-game-ready-world" in dependency_map(load_manifest(manifest_path)):
            errors.append(
                "project-driven final bootstrap assembly regression: "
                f"{manifest_path} still depends on retired newengine-game-ready-world"
            )

    game_ready_profile_path = manifests.get("newengine-game-ready-profile")
    if game_ready_profile_path is not None:
        profile_src = game_ready_profile_path.parent / "src"
        retired_world_runtime_file = profile_src / "world_runtime.rs"
        if retired_world_runtime_file.exists():
            errors.append(
                "project-driven world-runtime orchestration regression: "
                f"{retired_world_runtime_file} must not exist; profile consumes domain contributions"
            )
        for source in profile_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            if "GameReadyWorldRuntimeProvider" in text:
                errors.append(
                    "project-driven world-runtime orchestration regression: "
                    f"{source} reintroduces GameReadyWorldRuntimeProvider"
                )
            for direct_tick in (
                "newengine_game_ready_world::tick_prelaunch",
                "newengine_game_ready_world::tick_frame",
            ):
                if direct_tick in text:
                    errors.append(
                        "project-driven world-runtime orchestration regression: "
                        f"{source} directly calls retired monolithic world tick {direct_tick!r}"
                    )
        runtime_units_source = profile_src / "runtime_units.rs"
        if runtime_units_source.is_file():
            runtime_units_text = runtime_units_source.read_text(
                encoding="utf-8", errors="replace"
            )
            required_domain_providers = (
                "FpsCharacterPresentationWorldRuntimeProvider::shared()",
                "AuthoredWorldStreamingWorldRuntimeProvider::shared()",
                "WorldEnvironmentAdmissionWorldRuntimeProvider::shared",
                "FpsContentWorldRuntimeProvider::shared()",
                "WorldEnvironmentSimulationWorldRuntimeProvider::shared",
            )
            missing = [marker for marker in required_domain_providers if marker not in runtime_units_text]
            if missing:
                errors.append(
                    "project-driven world-runtime ownership regression: "
                    f"{runtime_units_source} must register domain-owned runtime providers directly; missing={missing}"
                )
            if "newengine_game_ready_world" in runtime_units_text:
                errors.append(
                    "project-driven world-runtime ownership regression: "
                    f"{runtime_units_source} must not depend on newengine-game-ready-world"
                )
        profile_manifest_text = game_ready_profile_path.read_text(encoding="utf-8", errors="replace")
        for forbidden_dependency in ("newengine-game-ready-world", "newengine-game-module-fps"):
            if forbidden_dependency in profile_manifest_text:
                errors.append(
                    "project-driven profile dependency regression: "
                    f"{game_ready_profile_path} must not depend on {forbidden_dependency}"
                )
        for source in profile_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            for forbidden_marker in ("newengine_game_ready_world", "newengine_game_module_fps"):
                if forbidden_marker in text:
                    errors.append(
                        "project-driven profile dependency regression: "
                        f"{source} references concrete module {forbidden_marker}; distribution/game-module composition owns it"
                    )

        retired_bootstrap_identities = (
            "GameReadySceneBootstrapModule",
            "GameReadyWorldSceneBootstrapProvider",
            "app.game-ready.world-bootstrap",
        )
        for source in profile_src.rglob("*.rs"):
            text = source.read_text(encoding="utf-8", errors="replace")
            leaked = sorted(
                marker for marker in retired_bootstrap_identities if marker in text
            )
            if leaked:
                errors.append(
                    "project-driven bootstrap ownership regression: "
                    f"{source} contains retired GameReady bootstrap identity: {', '.join(leaked)}; provider/lifecycle belong to newengine-authored-world-runtime"
                )

        scene_bootstrap_source = profile_src / "scene_bootstrap.rs"
        if scene_bootstrap_source.is_file():
            text = scene_bootstrap_source.read_text(encoding="utf-8", errors="replace")
            if "SceneBootstrapProvider for" in text:
                errors.append(
                    "project-driven bootstrap ownership regression: "
                    f"{scene_bootstrap_source} implements SceneBootstrapProvider; GameReady may contribute assembly but generic authored-world owns provider identity"
                )
            if "AuthoredMapSceneBootstrapContributor" not in text:
                errors.append(
                    "project-driven bootstrap ownership regression: "
                    f"{scene_bootstrap_source} must use AuthoredMapSceneBootstrapContributor for transitional domain assembly"
                )

        runtime_units_source = profile_src / "runtime_units.rs"
        if runtime_units_source.is_file():
            text = runtime_units_source.read_text(encoding="utf-8", errors="replace")
            for required in (
                "AuthoredMapSceneBootstrapProvider::shared_with_contributor",
                "AuthoredWorldBootstrapModule::new",
            ):
                if required not in text:
                    errors.append(
                        "project-driven bootstrap ownership regression: "
                        f"{runtime_units_source} missing generic authored-world composition marker {required!r}"
                    )

    # P0.6 closed: reusable FPS runtime/API must not expose demo-named production semantics.
    retired_fps_demo_markers = ("FpsDemo", "Cores {}/{} · Targets {}/{}")
    for source in (ROOT / "crates").rglob("*.rs"):
        if "third_party" in source.parts:
            continue
        text = source.read_text(encoding="utf-8", errors="replace")
        leaked = sorted(marker for marker in retired_fps_demo_markers if marker in text)
        if leaked:
            errors.append(
                "project-driven FPS semantic regression: "
                f"{source} contains retired demo semantic marker(s): {', '.join(leaked)}"
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
    print("  engine.assets.maps -> AssetManager-owned semantic gateway + .ymap format module; GameReady consumes gateway only")
    print("  engine.assets.textures -> StarVault format/decode path through engine.assets; no standalone texture provider")
    print(f"  external provider routes checked against Host namespace: {external_provider_count}")
    print(f"  capability provider registrars checked: {len(provider_crates)}")
    print(f"  game/server distributions checked for editor implementation deps: {len(checked_distributions)}")
    print("  editor contract APIs may be shared; editor implementations are not required by game/server packages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
