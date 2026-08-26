#!/usr/bin/env python3
"""Conformance gate for first-party native PluginDescriptorV2 publication.

Production provider DLLs under PluginsSrc must export newengine_plugin_descriptor_v2
and must not implement that export through PluginDescriptorV2::from_legacy.
Compatibility normalization remains legal in the engine host and migration/tests.
"""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import sys


@dataclass(frozen=True)
class Provider:
    manifest: Path
    source_root: Path
    codec_legacy_root: bool
    native_v2: bool
    compatibility_conversion: bool


def repo_roots() -> tuple[Path, Path]:
    script = Path(__file__).resolve()
    northstar = script.parents[3]
    plugins = northstar / "PluginsSrc"
    if not plugins.is_dir():
        raise SystemExit(f"PluginsSrc not found: {plugins}")
    return northstar, plugins


def source_text(source_root: Path) -> str:
    chunks: list[str] = []
    for path in sorted(source_root.rglob("*.rs")):
        if any(part in {"target", ".git"} for part in path.parts):
            continue
        if ".bak" in path.name:
            continue
        chunks.append(path.read_text(encoding="utf-8", errors="ignore"))
    return "\n".join(chunks)


def discover(plugins_root: Path) -> list[Provider]:
    providers: list[Provider] = []
    for manifest in sorted(plugins_root.rglob("Cargo.toml")):
        if any(part in {"target", ".git"} for part in manifest.parts):
            continue
        cargo = manifest.read_text(encoding="utf-8", errors="ignore")
        if "cdylib" not in cargo:
            continue
        source_root = manifest.parent / "src"
        if not source_root.is_dir():
            continue
        source = source_text(source_root)
        canonical_plugin = any(
            marker in source
            for marker in (
                "newengine_plugin_signature_v1",
                "export_newengine_plugin_signature!",
                "export_plugin_root!",
                "export_newengine_plugin!",
                "newengine_plugin_root_v1",
            )
        )
        codec_provider = (
            "[package.metadata.newengine.codec]" in cargo
            and "enabled = true" in cargo
            and "runtime_provider_id" in cargo
        )
        if not (canonical_plugin or codec_provider):
            continue

        native_v2 = any(
            marker in source
            for marker in (
                "export_newengine_plugin_descriptor_v2!",
                "export_plugin_descriptor_v2!",
                "newengine_plugin_descriptor_v2",
            )
        )
        compatibility_conversion = "PluginDescriptorV2::from_legacy" in source
        providers.append(
            Provider(
                manifest=manifest,
                source_root=source_root,
                codec_legacy_root=codec_provider and not canonical_plugin,
                native_v2=native_v2,
                compatibility_conversion=compatibility_conversion,
            )
        )
    return providers


def main() -> int:
    _, plugins_root = repo_roots()
    providers = discover(plugins_root)
    if not providers:
        print("ERROR: no first-party production providers discovered", file=sys.stderr)
        return 2

    conforming = [
        provider
        for provider in providers
        if provider.native_v2 and not provider.compatibility_conversion
    ]
    total = len(providers)
    passed = len(conforming)
    percent = 100.0 * passed / total
    print(f"First-party native V2 providers: {passed} / {total} ({percent:.1f}%)")

    codec_legacy = sum(provider.codec_legacy_root for provider in providers)
    print(
        "Legacy codec-root ABI providers retaining native V2 discovery metadata: "
        f"{codec_legacy}"
    )

    failures = [provider for provider in providers if provider not in conforming]
    for provider in providers:
        rel = provider.manifest.parent.relative_to(plugins_root)
        state = "PASS" if provider in conforming else "FAIL"
        details: list[str] = []
        if not provider.native_v2:
            details.append("missing native V2 export")
        if provider.compatibility_conversion:
            details.append("uses PluginDescriptorV2::from_legacy")
        if provider.codec_legacy_root:
            details.append("legacy codec root retained")
        suffix = f" [{' ; '.join(details)}]" if details else ""
        print(f"{state}: {rel}{suffix}")

    if failures:
        print(
            "FAIL: first-party production providers must author and publish native DescriptorV2",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
