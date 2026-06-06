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
SERVICE_API = ENGINE_ROOT / "crates" / "newengine-service-api" / "src" / "lib.rs"
PLUGIN_HOST_STATE = ENGINE_ROOT / "crates" / "newengine-plugin-host" / "src" / "host_context" / "state.rs"
PLUGIN_HOST_GATEWAY = ENGINE_ROOT / "crates" / "newengine-plugin-host" / "src" / "host_context" / "gateway.rs"
CORE_PLUGIN_DIAGNOSTICS = ENGINE_ROOT / "crates" / "newengine-core" / "src" / "engine" / "plugins.rs"
NULL_PROVIDERS = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "null_providers.rs"
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


def rust_engine_gateways() -> set[str]:
    text = SERVICE_API.read_text(encoding="utf-8", errors="replace") if SERVICE_API.exists() else ""
    # Parse the match arms in EngineServiceKind::engine_gateway_id. This keeps
    # the P1 matrix tied to the engine's declared gateway vocabulary without
    # requiring a Rust build in workspace tools.
    return set(re.findall(r'Self::[A-Za-z0-9_]+\s*=>\s*"(engine\.[^"]+)"', text))


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
        "NullRenderer": ("NULL_RENDER_SERVICE", "register_null_render_provider", "engine.render.null", "render.backend"),
        "NullPhysics": ("NULL_PHYSICS_SERVICE", "register_null_physics_provider", "engine.physics.null", "physics.backend"),
        "NullUI": ("NULL_UI_SERVICE", "register_null_ui_provider", "engine.ui.null", "ui.backend"),
        "NullAI": ("NULL_AI_SERVICE", "register_null_ai_provider", "engine.ai.null", "ai.backend"),
    }
    for name, tokens in required.items():
        for token in tokens:
            if token not in text:
                out.append(Finding("ERROR", "null-provider", NULL_PROVIDERS, f"{name} must be a visible registered NullProvider route", token))
    if "register_null_engine_gateway_provider_service_dynamic_best_effort" not in text:
        out.append(Finding("ERROR", "null-provider", NULL_PROVIDERS, "null providers must register through null-provider gateway helper"))
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
