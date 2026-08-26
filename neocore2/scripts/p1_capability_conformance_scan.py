#!/usr/bin/env python3
"""P1 capability matrix and provider conformance gate for North Star Engine.

This is a source-level harness. It does not pretend to exercise GPU/physics/UI
backends inside Python; it validates that the project has the data contracts,
null-provider routes, diagnostics visibility and provider-family test matrix that
runtime/provider conformance tests must consume.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Iterable

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

CAPABILITY_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
SERVICE_GATEWAYS = ENGINE_ROOT / "crates" / "newengine-service-api" / "src" / "gateway.rs"
SERVICE_KINDS = ENGINE_ROOT / "crates" / "newengine-service-api" / "src" / "kind.rs"
RENDER_API = ENGINE_ROOT / "crates" / "newengine-render-api" / "src" / "constants.rs"
PHYSICS_API = ENGINE_ROOT / "crates" / "newengine-physics-api" / "src" / "service.rs"
UI_API = ENGINE_ROOT / "crates" / "newengine-ui-api" / "src" / "draw_protocol.rs"
SCRIPTING_API = ENGINE_ROOT / "crates" / "newengine-scripting-api" / "src" / "protocol.rs"
ASSETS_API = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "lib.rs"
PLUGIN_HOST_STATE = ENGINE_ROOT / "crates" / "newengine-plugin-host" / "src" / "host_context" / "state.rs"
PLUGIN_HOST_GATEWAY = ENGINE_ROOT / "crates" / "newengine-plugin-host" / "src" / "host_context" / "gateway" / "routes.rs"
CORE_PLUGIN_DIAGNOSTICS = ENGINE_ROOT / "crates" / "newengine-core" / "src" / "engine" / "plugins" / "diagnostics.rs"
NULL_PROVIDERS = ENGINE_ROOT / "crates" / "newengine-null-providers-runtime" / "src" / "lib.rs"
REGISTRY_TESTS = ENGINE_ROOT / "crates" / "newengine-plugin-host" / "src" / "service_gateway" / "registry" / "tests.rs"
ASSET_BROWSER_PRESENTATION = ENGINE_ROOT / "crates" / "newengine-assets-catalog-ui-runtime" / "src" / "entry_presentation.rs"

P1_REQUIRED_FAMILIES = {"render", "physics", "assets", "input", "ui", "time", "jobs", "scripting"}
P1_REQUIRED_NULL_ROUTES = {
    "engine.render": "engine.render.null",
    "engine.physics": "engine.physics.null",
    "engine.ui": "engine.ui.null",
    "engine.ai": "engine.ai.null",
}
REQUIRED_DESCRIPTOR_FIELDS = {
    "domain",
    "capability_id",
    "owner_service",
    "engine_gateway",
    "contract",
    "quality_tier",
    "backend_priority",
    "requires",
    "conflicts",
    "fallback_policy",
    "diagnostics",
}
REQUIRED_TEST_CASES = {
    "registers_expected_service",
    "declares_expected_capability",
    "declares_expected_engine_gateway",
    "accepts_valid_input_dto",
    "rejects_invalid_input_dto_cleanly",
    "returns_stable_error_payload",
    "shutdown_is_idempotent",
    "does_not_require_concrete_runtime_internals",
    "route_is_descriptor_driven",
    "active_and_shadowed_routes_visible_in_diagnostics",
}
FORBIDDEN_ASSET_BROWSER_EXTENSION_TOKENS = {
    "ytd",
    "ydd",
    "ydr",
    "ytyp",
    "ymap",
    "nemat",
    "png",
    "jpg",
    "jpeg",
    "dds",
    "gltf",
    "glb",
    "obj",
}


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    message: str
    excerpt: str = ""

    def render(self) -> str:
        rel = self.path.relative_to(REPO_ROOT) if self.path.is_absolute() and self.path.is_relative_to(REPO_ROOT) else self.path
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {rel}: {self.message}{suffix}"


def load_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p1-matrix", path, "required matrix file is missing")]
    try:
        return json.loads(path.read_text(encoding="utf-8")), []
    except json.JSONDecodeError as exc:
        return {}, [Finding("ERROR", "p1-matrix", path, f"invalid JSON: {exc}")]


def rust_const_str(path: pathlib.Path, name: str) -> str | None:
    text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
    match = re.search(
        rf'pub\s+const\s+{re.escape(name)}\s*:\s*&str\s*=\s*"([^"]+)"',
        text,
    )
    return match.group(1) if match else None


def rust_engine_gateways() -> set[str]:
    gateway_text = (
        SERVICE_GATEWAYS.read_text(encoding="utf-8", errors="replace")
        if SERVICE_GATEWAYS.exists()
        else ""
    )
    kind_text = (
        SERVICE_KINDS.read_text(encoding="utf-8", errors="replace")
        if SERVICE_KINDS.exists()
        else ""
    )
    canonical = set(
        re.findall(
            r'pub\s+const\s+ENGINE_[A-Z0-9_]+_GATEWAY_ID\s*:\s*&str\s*=\s*"(engine\.[^"]+)"',
            gateway_text,
        )
    )
    # Keep literals from EngineServiceKind for domains that have not yet migrated
    # to named gateway constants. This makes the gate useful during P1 migration
    # instead of silently dropping legacy declarations.
    legacy = set(
        re.findall(
            r'Self::[A-Za-z0-9_]+\s*=>\s*"(engine\.[^"]+)"',
            kind_text,
        )
    )
    return canonical | legacy


def provider_family_authorities() -> dict[str, tuple[str, str, str]]:
    rows = {
        "render": (
            rust_const_str(SERVICE_GATEWAYS, "ENGINE_RENDER_GATEWAY_ID"),
            rust_const_str(RENDER_API, "RENDER_BACKEND_CAPABILITY_ID"),
            rust_const_str(RENDER_API, "RENDER_SERVICE_ID"),
        ),
        "physics": (
            rust_const_str(SERVICE_GATEWAYS, "ENGINE_PHYSICS_GATEWAY_ID"),
            rust_const_str(PHYSICS_API, "PHYSICS_BACKEND_CAPABILITY_ID"),
            rust_const_str(PHYSICS_API, "PHYSICS_SERVICE_ID"),
        ),
        "ui": (
            rust_const_str(SERVICE_GATEWAYS, "ENGINE_UI_GATEWAY_ID"),
            rust_const_str(UI_API, "UI_BACKEND_CAPABILITY_ID"),
            rust_const_str(UI_API, "UI_SERVICE_ID"),
        ),
        "scripting": (
            rust_const_str(SERVICE_GATEWAYS, "ENGINE_SCRIPTING_GATEWAY_ID"),
            rust_const_str(SCRIPTING_API, "SCRIPTING_BACKEND_CAPABILITY_ID"),
            rust_const_str(SCRIPTING_API, "SCRIPTING_SERVICE_ID"),
        ),
        "assets": (
            rust_const_str(SERVICE_GATEWAYS, "ENGINE_ASSETS_GATEWAY_ID"),
            rust_const_str(ASSETS_API, "ASSET_BACKEND_CAPABILITY_ID"),
            rust_const_str(ASSETS_API, "ASSET_PROVIDER_SERVICE_ID"),
        ),
    }
    return {
        family: (gateway or "", capability or "", service or "")
        for family, (gateway, capability, service) in rows.items()
    }

def scan_capability_matrix() -> list[Finding]:
    matrix, findings = load_json(CAPABILITY_MATRIX)
    if findings:
        return findings
    records = matrix.get("records")
    if not isinstance(records, list):
        return [Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, "records must be a list")]

    out: list[Finding] = []
    seen_gateways: set[str] = set()
    seen_caps: set[str] = set()
    by_gateway: dict[str, dict] = {}
    for idx, rec in enumerate(records):
        if not isinstance(rec, dict):
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] must be an object"))
            continue
        missing = sorted(REQUIRED_DESCRIPTOR_FIELDS.difference(rec))
        if missing:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] missing fields: {', '.join(missing)}"))
        gateway = str(rec.get("engine_gateway", "")).strip()
        cap = str(rec.get("capability_id", "")).strip()
        contract = str(rec.get("contract", "")).strip()
        if not gateway.startswith("engine."):
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] engine_gateway must start with engine.*", gateway))
        if not cap:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] capability_id is empty"))
        if not contract:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] contract is empty"))
        diagnostics = rec.get("diagnostics") or {}
        for key in ("must_show_active_route", "must_show_shadowed_routes", "must_show_capability_id"):
            if diagnostics.get(key) is not True:
                out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, f"record[{idx}] diagnostics.{key} must be true", gateway))
        if gateway in seen_gateways:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, "duplicate engine_gateway descriptor", gateway))
        if cap in seen_caps:
            # Most descriptors should have unique capability ids. Let variants
            # model separate capabilities instead of aliasing one hidden feature.
            out.append(Finding("WARN", "capability-matrix", CAPABILITY_MATRIX, "duplicate capability_id; verify this is intentional", cap))
        seen_gateways.add(gateway)
        seen_caps.add(cap)
        by_gateway[gateway] = rec

    declared_gateways = rust_engine_gateways()
    missing_from_matrix = sorted(declared_gateways - seen_gateways)
    for gateway in missing_from_matrix:
        out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, "engine-service-api gateway has no capability descriptor", gateway))

    for gateway, provider_route in P1_REQUIRED_NULL_ROUTES.items():
        rec = by_gateway.get(gateway)
        if not rec:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, "required null-provider gateway missing from matrix", gateway))
            continue
        if rec.get("null_provider_route") != provider_route:
            out.append(Finding("ERROR", "capability-matrix", CAPABILITY_MATRIX, "wrong/null missing null_provider_route", f"{gateway} -> {rec.get('null_provider_route')}"))
    return out


def scan_diagnostics_visibility() -> list[Finding]:
    out: list[Finding] = []
    state = PLUGIN_HOST_STATE.read_text(encoding="utf-8", errors="replace") if PLUGIN_HOST_STATE.exists() else ""
    gateway = PLUGIN_HOST_GATEWAY.read_text(encoding="utf-8", errors="replace") if PLUGIN_HOST_GATEWAY.exists() else ""
    core = CORE_PLUGIN_DIAGNOSTICS.read_text(encoding="utf-8", errors="replace") if CORE_PLUGIN_DIAGNOSTICS.exists() else ""
    for field in ("active", "selection_state", "selection_reason"):
        if re.search(rf"pub\s+{field}\s*:", state) is None:
            out.append(Finding("ERROR", "gateway-diagnostics", PLUGIN_HOST_STATE, f"EngineGatewayRouteSnapshot must expose {field}"))
    for token in ("selection_state", "selection_reason", "shadowed_by"):
        if token not in gateway:
            out.append(Finding("ERROR", "gateway-diagnostics", PLUGIN_HOST_GATEWAY, f"route list construction must compute {token}"))
    for token in ("selection_state", "selection_reason"):
        if token not in core:
            out.append(Finding("ERROR", "gateway-diagnostics", CORE_PLUGIN_DIAGNOSTICS, f"startup diagnostics table must print {token}"))
    tests = REGISTRY_TESTS.read_text(encoding="utf-8", errors="replace") if REGISTRY_TESTS.exists() else ""
    if "active_and_shadowed_routes_are_diagnostic_visible" not in tests:
        out.append(Finding("ERROR", "gateway-diagnostics", REGISTRY_TESTS, "missing active/shadowed diagnostics test"))
    return out


def scan_null_providers() -> list[Finding]:
    out: list[Finding] = []
    text = NULL_PROVIDERS.read_text(encoding="utf-8", errors="replace") if NULL_PROVIDERS.exists() else ""
    required = {
        "NullRenderer": (
            "NULL_RENDER_SERVICE",
            "NULL_RENDER_ROUTE",
            "register_null_render_provider",
            "RENDER_BACKEND_SERVICE_SPEC",
            "spec.engine_gateway_id",
            "spec.backend_capability_id",
            "service_kind: spec.domain",
        ),
        "NullPhysics": (
            "NULL_PHYSICS_SERVICE",
            "NULL_PHYSICS_ROUTE",
            "register_null_physics_provider",
            "PHYSICS_BACKEND_SERVICE_SPEC",
            "spec.engine_gateway_id",
            "spec.backend_capability_id",
            "service_kind: spec.domain",
        ),
        "NullUI": (
            "NULL_UI_SERVICE",
            "NULL_UI_ROUTE",
            "register_null_ui_provider",
            "UI_BACKEND_SERVICE_SPEC",
            "spec.engine_gateway_id",
            "spec.backend_capability_id",
            "service_kind: spec.domain",
        ),
        "NullAI": (
            "NULL_AI_SERVICE",
            "register_null_ai_provider",
            "engine.ai.null",
            "ai.backend",
        ),
    }
    for name, tokens in required.items():
        for token in tokens:
            if token not in text:
                out.append(
                    Finding(
                        "ERROR",
                        "null-provider",
                        NULL_PROVIDERS,
                        f"{name} must consume its canonical provider contract and expose a visible null route",
                        token,
                    )
                )
    if "register_null_engine_gateway_provider_service_dynamic_best_effort" not in text:
        out.append(
            Finding(
                "ERROR",
                "null-provider",
                NULL_PROVIDERS,
                "null providers must register through null-provider gateway helper",
            )
        )
    return out

def scan_conformance_matrix() -> list[Finding]:
    matrix, findings = load_json(CONFORMANCE_MATRIX)
    if findings:
        return findings
    families = matrix.get("families")
    if not isinstance(families, list):
        return [Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, "families must be a list")]
    out: list[Finding] = []
    by_family: dict[str, dict] = {}
    for idx, family in enumerate(families):
        if not isinstance(family, dict):
            out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, f"family[{idx}] must be an object"))
            continue
        name = str(family.get("family", "")).strip()
        if not name:
            out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, f"family[{idx}] missing family id"))
            continue
        by_family[name] = family
        for field in ("gateway", "capability", "service", "methods", "test_cases"):
            if field not in family:
                out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, f"family '{name}' missing {field}"))
        missing_tests = sorted(REQUIRED_TEST_CASES.difference(set(family.get("test_cases") or [])))
        if missing_tests:
            out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, f"family '{name}' missing test cases: {', '.join(missing_tests)}"))
        if not str(family.get("gateway", "")).startswith("engine."):
            out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, f"family '{name}' gateway must be engine.*", str(family.get("gateway", ""))))
        methods = family.get("methods") or []
        if "shutdown_v1" not in methods and name not in {"input"}:
            out.append(Finding("WARN", "conformance-matrix", CONFORMANCE_MATRIX, f"family '{name}' has no shutdown_v1 method listed"))
    missing_families = sorted(P1_REQUIRED_FAMILIES.difference(by_family))
    for family in missing_families:
        out.append(Finding("ERROR", "conformance-matrix", CONFORMANCE_MATRIX, "P1 required provider family missing", family))
    return out


def scan_api_contract_ownership() -> list[Finding]:
    out: list[Finding] = []
    conformance, conformance_findings = load_json(CONFORMANCE_MATRIX)
    capability, capability_findings = load_json(CAPABILITY_MATRIX)
    if conformance_findings or capability_findings:
        return conformance_findings + capability_findings

    families = {
        str(row.get("family", "")).strip(): row
        for row in conformance.get("families", [])
        if isinstance(row, dict)
    }
    records = {
        str(row.get("engine_gateway", "")).strip(): row
        for row in capability.get("records", [])
        if isinstance(row, dict)
    }

    for family, (gateway, capability_id, service_id) in provider_family_authorities().items():
        if not gateway or not capability_id or not service_id:
            out.append(
                Finding(
                    "ERROR",
                    "api-contract-owner",
                    SERVICE_GATEWAYS,
                    f"failed to resolve Rust authority for provider family '{family}'",
                    f"gateway={gateway!r} capability={capability_id!r} service={service_id!r}",
                )
            )
            continue

        family_row = families.get(family)
        if family_row is None:
            out.append(
                Finding(
                    "ERROR",
                    "api-contract-owner",
                    CONFORMANCE_MATRIX,
                    "provider family missing from conformance matrix",
                    family,
                )
            )
        else:
            for field, expected in (
                ("gateway", gateway),
                ("capability", capability_id),
                ("service", service_id),
            ):
                actual = str(family_row.get(field, "")).strip()
                if actual != expected:
                    out.append(
                        Finding(
                            "ERROR",
                            "api-contract-owner",
                            CONFORMANCE_MATRIX,
                            f"{family}.{field} drifted from Rust API owner",
                            f"expected={expected} actual={actual}",
                        )
                    )

        capability_row = records.get(gateway)
        if capability_row is None:
            out.append(
                Finding(
                    "ERROR",
                    "api-contract-owner",
                    CAPABILITY_MATRIX,
                    "API-owned provider gateway missing from capability matrix",
                    gateway,
                )
            )
        else:
            for field, expected in (
                ("capability_id", capability_id),
                ("owner_service", service_id),
                ("contract", service_id),
            ):
                actual = str(capability_row.get(field, "")).strip()
                if actual != expected:
                    out.append(
                        Finding(
                            "ERROR",
                            "api-contract-owner",
                            CAPABILITY_MATRIX,
                            f"{family}.{field} drifted from Rust API owner",
                            f"expected={expected} actual={actual}",
                        )
                    )
    return out


def scan_asset_browser_contract_policy() -> list[Finding]:
    """Forbid Asset Browser from guessing semantics by extension/name/hash.

    Icon asset paths may still live in the large catalog runtime module. This
    check is intentionally focused on the presentation decision module where
    labels, icons and preview labels are selected for catalog rows.
    """

    if not ASSET_BROWSER_PRESENTATION.exists():
        return [Finding("ERROR", "asset-browser-contract", ASSET_BROWSER_PRESENTATION, "entry presentation module is missing")]

    text = ASSET_BROWSER_PRESENTATION.read_text(encoding="utf-8", errors="replace")
    out: list[Finding] = []
    if re.search(r"match\s+entry\.extension\b", text) or re.search(r"match\s+ext\b", text):
        out.append(Finding("ERROR", "asset-browser-contract", ASSET_BROWSER_PRESENTATION, "presentation layer must not branch on extension"))
    if "preview provider" in text:
        out.append(Finding("ERROR", "asset-browser-contract", ASSET_BROWSER_PRESENTATION, "preview provider must come from explicit browser contract, not presentation text"))
    for token in sorted(FORBIDDEN_ASSET_BROWSER_EXTENSION_TOKENS):
        pattern = rf'"\.?{re.escape(token)}"'
        if re.search(pattern, text, re.IGNORECASE):
            out.append(Finding("ERROR", "asset-browser-contract", ASSET_BROWSER_PRESENTATION, "forbidden hardcoded extension token in Asset Browser presentation", token))
    if "declared provider contract required" not in text:
        out.append(Finding("WARN", "asset-browser-contract", ASSET_BROWSER_PRESENTATION, "missing degraded-state copy for absent browser explanation"))
    return out


def run_checks() -> list[Finding]:
    findings: list[Finding] = []
    findings.extend(scan_capability_matrix())
    findings.extend(scan_diagnostics_visibility())
    findings.extend(scan_null_providers())
    findings.extend(scan_conformance_matrix())
    findings.extend(scan_api_contract_ownership())
    findings.extend(scan_asset_browser_contract_policy())
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p1_capability_conformance_scan.py")
    parser.add_argument("--summary-only", action="store_true")
    ns = parser.parse_args(argv)
    findings = run_checks()
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p1 capability/conformance scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
