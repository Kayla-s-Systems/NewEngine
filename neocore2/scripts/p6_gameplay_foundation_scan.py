#!/usr/bin/env python3
"""Gameplay-foundation architecture gate.

P6 is provider-oriented: tags, tasks, animation, navigation and AI are independent
runtime units. No aggregate gameplay runtime is allowed to own or imply the five
providers as one composition bundle.
"""
from __future__ import annotations

import json
import sys
import tomllib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
CAP_MATRIX = ROOT / "config/capabilities/engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ROOT / "config/conformance/provider_conformance_matrix.v1.json"
RUNTIME_UNITS_CARGO = CRATES / "newengine-runtime-units/Cargo.toml"
RUNTIME_UNITS_LIB = CRATES / "newengine-runtime-units/src/lib.rs"
WORKSPACE_CARGO = ROOT / "Cargo.toml"
AGGREGATE_RUNTIME = CRATES / "newengine-gameplay-runtime"


@dataclass(frozen=True)
class Family:
    name: str
    api_crate: str
    runtime_crate: str
    gateway: str
    capability: str
    provider_route: str
    register_fn: str


FAMILIES = (
    Family("tags", "newengine-tags-api", "newengine-tags-runtime", "engine.tags", "tags.registry", "engine.tags.foundation", "register_tags_gateway_best_effort"),
    Family("tasks", "newengine-tasks-api", "newengine-tasks-runtime", "engine.tasks", "tasks.backend", "engine.tasks.foundation", "register_tasks_gateway_best_effort"),
    Family("animation", "newengine-animation-api", "newengine-animation-foundation-runtime", "engine.animation", "animation.backend", "engine.animation.foundation", "register_animation_gateway_best_effort"),
    Family("navigation", "newengine-navigation-api", "newengine-navigation-runtime", "engine.navigation", "navigation.backend", "engine.navigation.foundation", "register_navigation_gateway_best_effort"),
    Family("ai", "newengine-ai-api", "newengine-ai-runtime", "engine.ai", "ai.backend", "engine.ai.foundation", "register_ai_gateway_best_effort"),
)

AI_FORBIDDEN_MARKERS = (
    "newengine_ecs::",
    "&mut World",
    "& mut World",
    "use newengine_entity_api::EntityId",
)


def source_text(root: Path) -> str:
    if not root.exists():
        return ""
    return "\n".join(
        path.read_text(encoding="utf-8", errors="replace")
        for path in sorted(root.rglob("*.rs"))
    )


def code_source_text(root: Path) -> str:
    """Rust source with line comments removed for syntax-oriented boundary checks."""
    lines: list[str] = []
    if not root.exists():
        return ""
    for path in sorted(root.rglob("*.rs")):
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.lstrip().startswith("//"):
                continue
            lines.append(line)
    return "\n".join(lines)


def load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def dependency_names(manifest: dict) -> set[str]:
    names = set(manifest.get("dependencies", {}))
    for target in manifest.get("target", {}).values():
        names.update(target.get("dependencies", {}))
    return names


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def require(errors: list[str], condition: bool, message: str) -> None:
    if not condition:
        errors.append(message)


def main() -> int:
    errors: list[str] = []

    require(errors, not AGGREGATE_RUNTIME.exists(), "aggregate newengine-gameplay-runtime must not exist")
    workspace_text = WORKSPACE_CARGO.read_text(encoding="utf-8")
    require(
        errors,
        '"crates/newengine-gameplay-runtime"' not in workspace_text,
        "workspace must not include aggregate newengine-gameplay-runtime",
    )

    runtime_units_manifest = load_toml(RUNTIME_UNITS_CARGO)
    runtime_unit_deps = dependency_names(runtime_units_manifest)
    runtime_units_source = RUNTIME_UNITS_LIB.read_text(encoding="utf-8", errors="replace")

    cap_text = json.dumps(load_json(CAP_MATRIX), sort_keys=True)
    conformance_text = json.dumps(load_json(CONFORMANCE_MATRIX), sort_keys=True)

    for family in FAMILIES:
        api_root = CRATES / family.api_crate
        runtime_root = CRATES / family.runtime_crate
        api_manifest_path = api_root / "Cargo.toml"
        runtime_manifest_path = runtime_root / "Cargo.toml"
        require(errors, api_manifest_path.exists(), f"{family.name}: API crate missing: {family.api_crate}")
        require(errors, runtime_manifest_path.exists(), f"{family.name}: runtime crate missing: {family.runtime_crate}")
        if not api_manifest_path.exists() or not runtime_manifest_path.exists():
            continue

        api_manifest = load_toml(api_manifest_path)
        runtime_manifest = load_toml(runtime_manifest_path)
        require(
            errors,
            api_manifest.get("package", {}).get("name") == family.api_crate,
            f"{family.name}: API package identity mismatch",
        )
        require(
            errors,
            runtime_manifest.get("package", {}).get("name") == family.runtime_crate,
            f"{family.name}: runtime package identity mismatch",
        )
        require(
            errors,
            family.api_crate in dependency_names(runtime_manifest),
            f"{family.name}: runtime must depend on its API contract crate {family.api_crate}",
        )

        runtime_source = source_text(runtime_root / "src")
        for token, label in (
            (family.provider_route, "provider route"),
            (family.register_fn, "gateway registration"),
            ("RUNTIME_UNIT_REGISTRATION", "runtime-unit registration"),
            (family.gateway, "gateway id"),
        ):
            require(errors, token in runtime_source, f"{family.name}: missing {label} token {token!r}")

        require(
            errors,
            family.runtime_crate in runtime_unit_deps,
            f"runtime-units: missing leaf dependency {family.runtime_crate}",
        )
        rust_module = family.runtime_crate.replace("-", "_")
        require(
            errors,
            f"{rust_module}::RUNTIME_UNIT_REGISTRATION" in runtime_units_source,
            f"runtime-units: {family.name} leaf registration is not catalogued",
        )
        require(errors, family.gateway in cap_text, f"capability matrix missing gateway {family.gateway}")
        require(errors, family.capability in cap_text, f"capability matrix missing capability {family.capability}")
        require(
            errors,
            f'"family": "{family.name}"' in conformance_text,
            f"conformance matrix missing provider family {family.name}",
        )

    ai_source = source_text(CRATES / "newengine-ai-api/src") + "\n" + source_text(CRATES / "newengine-ai-runtime/src")
    ai_code = code_source_text(CRATES / "newengine-ai-api/src") + "\n" + code_source_text(CRATES / "newengine-ai-runtime/src")
    for marker in AI_FORBIDDEN_MARKERS:
        require(errors, marker not in ai_code, f"AI provider boundary leaks world/ECS mutation marker {marker!r}")
    require(errors, "AiIntentDtoV1" in ai_source, "AI boundary must expose intent DTOs")
    require(errors, "intent-only" in ai_source.lower() or "intent only" in ai_source.lower(), "AI runtime must document intent-only ownership")

    if errors:
        print(f"P6 GAMEPLAY FOUNDATION GATE: FAIL errors={len(errors)}")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("P6 GAMEPLAY FOUNDATION GATE: PASS")
    print("  composition: five independent leaf runtime units")
    print("  aggregate gameplay runtime: absent")
    print("  runtime-units: authoritative registration catalog")
    print("  AI boundary: intent DTOs only; no World/ECS ownership")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
