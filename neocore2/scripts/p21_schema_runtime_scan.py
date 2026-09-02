#!/usr/bin/env python3
"""P2.1 live engine.schema provider gate for North Star Engine.

Verifies that schema is a core-owned replaceable baseline provider, not an
external must-have plugin and not a hidden hardcoded Inspector branch.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

WORKSPACE_TOML = ENGINE_ROOT / "Cargo.toml"
CARGO_LOCK = ENGINE_ROOT / "Cargo.lock"
RUNTIME_CRATE = ENGINE_ROOT / "crates" / "newengine-schema-runtime"
RUNTIME_LIB = RUNTIME_CRATE / "src" / "lib.rs"
RUNTIME_HOST_TOML = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "Cargo.toml"
RUNTIME_HOST_PLATFORM = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "runtime_host.rs"
RUNTIME_HOST_HEADLESS = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "headless_cli.rs"
GAME_READY_TOML = ENGINE_ROOT / "crates" / "newengine-game-ready-profile" / "Cargo.toml"
GAME_READY_PROFILE = ENGINE_ROOT / "crates" / "newengine-game-ready-profile" / "src" / "lib.rs"
CAPABILITY_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
SCHEMA_API = ENGINE_ROOT / "crates" / "newengine-schema-api" / "src" / "lib.rs"

REQUIRED_METHODS = {
    "schema.describe_type_v1",
    "schema.describe_properties_v1",
    "schema.validate_patch_v1",
    "schema.default_value_v1",
    "schema.binding_manifest_v1",
    "schema.transaction_plan_v1",
}

REQUIRED_RUNTIME_TOKENS = {
    "register_schema_gateway_best_effort",
    "schema_gateway_service",
    "SchemaRegistryState",
    "EngineGatewayProviderDecl",
    "EngineServiceKind::Schema",
    "ENGINE_SCHEMA_SERVICE_ID",
    "SCHEMA_SERVICE_ID",
    "SCHEMA_BACKEND_CAPABILITY_ID",
    "SCHEMA_RUNTIME_CONTRACT",
    "PROVIDER_ROUTE: &str = \"engine.schema.registry\"",
    "core-owned-baseline-replaceable-provider",
    "replaceable: true",
    "describe_type",
    "describe_properties",
    "validate_patch",
    "default_value",
    "binding_manifest_from_value",
    "transaction_plan",
    "normalized_patch",
    "undo_operations",
}


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    message: str
    excerpt: str = ""

    def render(self) -> str:
        try:
            rel = self.path.relative_to(REPO_ROOT)
        except Exception:
            rel = self.path
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {rel}: {self.message}{suffix}"


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def load_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p21-schema-runtime", path, "required JSON file is missing")]
    try:
        return json.loads(path.read_text(encoding="utf-8")), []
    except json.JSONDecodeError as exc:
        return {}, [Finding("ERROR", "p21-schema-runtime", path, f"invalid JSON: {exc}")]


def scan_workspace() -> list[Finding]:
    out: list[Finding] = []
    workspace = read(WORKSPACE_TOML)
    if '"crates/newengine-schema-runtime"' not in workspace:
        out.append(Finding("ERROR", "schema-runtime-workspace", WORKSPACE_TOML, "workspace must include newengine-schema-runtime"))
    lock = read(CARGO_LOCK)
    if 'name = "newengine-schema-runtime"' not in lock:
        out.append(Finding("ERROR", "schema-runtime-workspace", CARGO_LOCK, "Cargo.lock must include newengine-schema-runtime package"))
    for path in (RUNTIME_HOST_TOML, GAME_READY_TOML):
        if "newengine-schema-runtime" not in read(path):
            out.append(Finding("ERROR", "schema-runtime-dependency", path, "runtime/profile must depend on core schema runtime crate"))
    return out


def scan_runtime_crate() -> list[Finding]:
    out: list[Finding] = []
    if not RUNTIME_CRATE.exists():
        return [Finding("ERROR", "schema-runtime-crate", RUNTIME_CRATE, "newengine-schema-runtime crate is missing")]
    text = "\n".join(read(path) for path in sorted((RUNTIME_CRATE / "src").glob("*.rs")))
    for token in REQUIRED_RUNTIME_TOKENS:
        if token not in text:
            out.append(Finding("ERROR", "schema-runtime-crate", RUNTIME_CRATE, "missing runtime provider token", token))
    required_method_tokens = {
        "schema.describe_type_v1": "schema_method::DESCRIBE_TYPE_V1",
        "schema.describe_properties_v1": "schema_method::DESCRIBE_PROPERTIES_V1",
        "schema.validate_patch_v1": "schema_method::VALIDATE_PATCH_V1",
        "schema.default_value_v1": "schema_method::DEFAULT_VALUE_V1",
        "schema.binding_manifest_v1": "schema_method::BINDING_MANIFEST_V1",
        "schema.transaction_plan_v1": "schema_method::TRANSACTION_PLAN_V1",
    }
    for method, token in required_method_tokens.items():
        if token not in text:
            out.append(Finding("ERROR", "schema-runtime-method", RUNTIME_CRATE, "runtime provider must register schema method", method))
    if re.search(r"priority:\s*0", text) is None:
        out.append(Finding("ERROR", "schema-runtime-route", RUNTIME_CRATE, "baseline provider priority must be neutral so external higher-origin providers can replace it"))
    if "register_engine_gateway_provider_service_best_effort" not in text:
        out.append(Finding("ERROR", "schema-runtime-route", RUNTIME_CRATE, "provider must register through gateway/capability route helper, not a hidden singleton"))
    forbidden = [
        "register_null_engine_gateway_provider_service",
        "must_have_plugin",
        "panic!(\"schema",
        "expect(\"schema",
    ]
    for token in forbidden:
        if token in text:
            out.append(Finding("ERROR", "schema-runtime-hidden-or-required", RUNTIME_CRATE, "schema runtime must not be hidden/null/must-have", token))
    return out


def scan_registration() -> list[Finding]:
    out: list[Finding] = []
    for path in (RUNTIME_HOST_PLATFORM, RUNTIME_HOST_HEADLESS, GAME_READY_PROFILE):
        text = read(path)
        if "newengine_schema_runtime::register_schema_gateway_best_effort();" not in text:
            out.append(Finding("ERROR", "schema-runtime-registration", path, "engine/profile startup must register core-owned schema baseline route"))
    return out


def scan_matrices() -> list[Finding]:
    out: list[Finding] = []
    cap, findings = load_json(CAPABILITY_MATRIX)
    out.extend(findings)
    if not findings:
        records = cap.get("records") or []
        schema_records = [r for r in records if isinstance(r, dict) and r.get("engine_gateway") == "engine.schema"]
        if not schema_records:
            out.append(Finding("ERROR", "schema-runtime-capability", CAPABILITY_MATRIX, "capability matrix must include engine.schema"))
        else:
            record = schema_records[0]
            if record.get("capability_id") != "schema.registry":
                out.append(Finding("ERROR", "schema-runtime-capability", CAPABILITY_MATRIX, "engine.schema capability must be schema.registry"))
            if record.get("owner_service") != "schema.api":
                out.append(Finding("ERROR", "schema-runtime-capability", CAPABILITY_MATRIX, "baseline owner service must remain schema.api"))
            if record.get("fallback_policy") != "explicit_profile_only":
                out.append(Finding("ERROR", "schema-runtime-capability", CAPABILITY_MATRIX, "schema fallback must be explicit profile policy only"))
    conf, findings = load_json(CONFORMANCE_MATRIX)
    out.extend(findings)
    if not findings:
        records = conf.get("families") or conf.get("records") or []
        schema_records = [r for r in records if isinstance(r, dict) and r.get("family") == "schema"]
        if not schema_records:
            out.append(Finding("ERROR", "schema-runtime-conformance", CONFORMANCE_MATRIX, "conformance matrix must include schema provider family"))
        else:
            record = schema_records[0]
            methods = set(record.get("methods") or [])
            missing_methods = sorted(REQUIRED_METHODS - methods)
            for method in missing_methods:
                out.append(Finding("ERROR", "schema-runtime-conformance", CONFORMANCE_MATRIX, "schema conformance missing method", method))
            cases = set(record.get("test_cases") or [])
            for case in ("core_owned_baseline_route_is_replaceable", "validate_patch_returns_normalized_patch_and_undo", "default_value_served_from_registry"):
                if case not in cases:
                    out.append(Finding("ERROR", "schema-runtime-conformance", CONFORMANCE_MATRIX, "schema conformance missing execution case", case))
    return out


def scan_api_contract() -> list[Finding]:
    out: list[Finding] = []
    text = read(SCHEMA_API)
    for token in ("SchemaPatchValidationResponseV1", "normalized_patch", "undo_operations", "SchemaDefaultValueResponseV1", "SchemaBindingManifestV1", "SchemaTransactionResultV1"):
        if token not in text:
            out.append(Finding("ERROR", "schema-runtime-api", SCHEMA_API, "schema API must expose live-provider response contract", token))
    return out


def run_checks() -> list[Finding]:
    findings: list[Finding] = []
    findings.extend(scan_workspace())
    findings.extend(scan_runtime_crate())
    findings.extend(scan_registration())
    findings.extend(scan_matrices())
    findings.extend(scan_api_contract())
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p21_schema_runtime_scan.py")
    parser.add_argument("--summary-only", action="store_true")
    ns = parser.parse_args(argv)
    findings = run_checks()
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p2.1 schema runtime scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
