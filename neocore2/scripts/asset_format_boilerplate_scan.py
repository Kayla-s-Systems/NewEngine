#!/usr/bin/env python3
"""Asset-format ownership gate for the StarVault/AssetManager architecture.

Concrete file extensions are discovered as data-oriented modules under
`PluginsSrc/formats`. Engine crates named `newengine-asset-format-*` may own a
compiled wire/container representation, but they must never register a runtime
plugin/provider route or become an alternative extension registry.
"""
from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO = ROOT.parent.parent
PLUGINS_SRC = REPO / "PluginsSrc"
FORMATS = PLUGINS_SRC / "formats"
ASSET_MANAGER = PLUGINS_SRC / "AssetManager"
CRATES = ROOT / "crates"

FORMAT_DESCRIPTOR_TOKENS = (
    "AssetFormatDescriptorSpecV1",
    "MODULE_ID",
    "EXTENSION",
    "SEMANTIC_GATEWAY",
    "HANDLER_SERVICE",
)
FORMAT_MODULE_TOKENS = (
    "mod descriptor",
    "export_asset_format_module_v1!",
)
FORBIDDEN_INTERNAL_REGISTRATION = (
    "export_asset_format_module_v1!",
    "newengine_plugin_root_v1",
    "register_service_v1",
    "CapabilityDesc::backend_route",
)


def main() -> int:
    violations: list[str] = []
    modules: list[pathlib.Path] = []
    seen_extensions: dict[str, pathlib.Path] = {}
    full_topology = PLUGINS_SRC.is_dir()

    if full_topology:
        format_api = FORMATS / "newengine-format-api" / "Cargo.toml"
        if not format_api.is_file():
            violations.append(f"{format_api}: StarVault format ABI crate is missing")

        if FORMATS.is_dir():
            modules = [
                path
                for path in sorted(FORMATS.iterdir())
                if path.is_dir()
                and path.name not in {".git", "target", "newengine-format-api"}
                and (path / "Cargo.toml").is_file()
            ]
        if not modules:
            violations.append(f"{FORMATS}: no discoverable concrete format modules")

        for module in modules:
            manifest = module / "Cargo.toml"
            lib = module / "src" / "lib.rs"
            descriptor = module / "src" / "descriptor.rs"
            for required in (manifest, lib, descriptor):
                if not required.is_file():
                    violations.append(f"{required}: required format-module artifact missing")
            if not lib.is_file() or not descriptor.is_file():
                continue

            lib_text = lib.read_text(encoding="utf-8", errors="replace")
            descriptor_text = descriptor.read_text(encoding="utf-8", errors="replace")
            for token in FORMAT_MODULE_TOKENS:
                if token not in lib_text:
                    violations.append(f"{lib}: format module missing `{token}`")
            for token in FORMAT_DESCRIPTOR_TOKENS:
                if token not in descriptor_text:
                    violations.append(f"{descriptor}: format descriptor missing `{token}`")

            extension = None
            for line in descriptor_text.splitlines():
                if "pub const EXTENSION" in line and '"' in line:
                    parts = line.split('"')
                    if len(parts) >= 3:
                        extension = parts[1].strip().lower()
                        break
            if not extension:
                violations.append(f"{descriptor}: EXTENSION must be a literal non-empty identifier")
            elif extension in seen_extensions:
                violations.append(
                    f"{descriptor}: duplicate extension '{extension}', already owned by {seen_extensions[extension]}"
                )
            else:
                seen_extensions[extension] = descriptor

        asset_workspace = ASSET_MANAGER / "Cargo.toml"
        if not asset_workspace.is_file():
            violations.append(f"{asset_workspace}: AssetManager workspace manifest missing")
        else:
            text = asset_workspace.read_text(encoding="utf-8", errors="replace")
            if "newengine-format-api" not in text or "../formats/newengine-format-api" not in text:
                violations.append(
                    f"{asset_workspace}: AssetManager must consume the shared format ABI, not concrete format crates"
                )
            for module in modules:
                if f"../formats/{module.name}" in text:
                    violations.append(
                        f"{asset_workspace}: concrete format '{module.name}' is statically linked; formats must remain discovered modules"
                    )

    # Engine-side compiled-format crates are representation libraries only. They may be used by
    # runtime/model/render code but may not become a second plugin/extension discovery surface.
    # This invariant is self-contained and therefore remains enforceable in the standalone
    # NewEngine GitHub checkout where sibling PluginsSrc is intentionally unavailable.
    for crate_dir in sorted(CRATES.glob("newengine-asset-format-*")):
        source_root = crate_dir / "src"
        text = (
            "\n".join(
                path.read_text(encoding="utf-8", errors="replace")
                for path in sorted(source_root.rglob("*.rs"))
            )
            if source_root.is_dir()
            else ""
        )
        for token in FORBIDDEN_INTERNAL_REGISTRATION:
            if token in text:
                violations.append(
                    f"{crate_dir.relative_to(ROOT)}: compiled format library must not register plugin/provider surface `{token}`"
                )

    if violations:
        print("asset-format ownership scan failed:")
        for item in violations:
            print(f"  {item}")
        return 1

    if full_topology:
        print(
            "asset-format ownership scan passed: "
            f"AssetManager + discovered format modules authoritative; formats={len(modules)} extensions={len(seen_extensions)}"
        )
    else:
        print(
            "asset-format ownership scan passed: standalone NewEngine checkout; "
            "compiled format libraries expose no plugin/provider registration surface"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
