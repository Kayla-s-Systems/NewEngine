#!/usr/bin/env python3
"""P2 schema/property/resource foundation gate for North Star Engine.

This is a source-level harness. It verifies that schema/property/transaction
contracts exist and that editor/assets/scripting consumers are wired to them
instead of depending on local hardcoded Inspector branches.
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

SCHEMA_API = ENGINE_ROOT / "crates" / "newengine-schema-api" / "src" / "lib.rs"
SERVICE_API = ENGINE_ROOT / "crates" / "newengine-service-api" / "src" / "lib.rs"
CAPABILITY_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
SCHEMA_REGISTRY = ENGINE_ROOT / "config" / "schema" / "schema_registry.v1.json"
ASSET_DOCUMENT_API = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "asset_document.rs"
ASSET_DOCUMENT_SERVICE = ENGINE_ROOT / "crates" / "newengine-assets" / "src" / "asset_document_service.rs"
SCREEN_PROFILE = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "screen_profile.rs"
SCREEN_PROFILE_PARTS = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "screen_profile_parts"
SCRIPTING_API = ENGINE_ROOT / "crates" / "newengine-scripting-api" / "src" / "lib.rs"
WORKSPACE_TOML = ENGINE_ROOT / "Cargo.toml"

REQUIRED_SCHEMA_METHODS = {
    "schema.describe_type_v1",
    "schema.describe_properties_v1",
    "schema.validate_patch_v1",
    "schema.default_value_v1",
    "schema.binding_manifest_v1",
    "schema.transaction_plan_v1",
}
REQUIRED_SCHEMA_DTOS = {
    "SchemaTypeDescriptorV1",
    "SchemaPropertyDescriptorV1",
    "SchemaPatchDtoV1",
    "SchemaPatchValidationRequestV1",
    "SchemaPatchValidationResponseV1",
    "SchemaDefaultValueRequestV1",
    "SchemaTransactionDtoV1",
    "SchemaBindingManifestV1",
}
REQUIRED_REGISTRY_DOMAINS = {"assets", "components", "settings"}


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


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def read_screen_profile_surface() -> str:
    text = read(SCREEN_PROFILE)
    if SCREEN_PROFILE_PARTS.exists():
        for part in sorted(SCREEN_PROFILE_PARTS.glob("*.rs")):
            text += "\n" + read(part)
    return text


def load_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p2-schema", path, "required JSON file is missing")]
    try:
        return json.loads(path.read_text(encoding="utf-8")), []
    except json.JSONDecodeError as exc:
        return {}, [Finding("ERROR", "p2-schema", path, f"invalid JSON: {exc}")]


def scan_schema_api() -> list[Finding]:
    out: list[Finding] = []
    text = read(SCHEMA_API)
    cargo = read(WORKSPACE_TOML)
    if not SCHEMA_API.exists():
        return [Finding("ERROR", "schema-api", SCHEMA_API, "newengine-schema-api crate is missing")]
    if '"crates/newengine-schema-api"' not in cargo:
        out.append(Finding("ERROR", "schema-api", WORKSPACE_TOML, "workspace does not include newengine-schema-api"))
    for token in ("ENGINE_SCHEMA_SERVICE_ID", "SCHEMA_BACKEND_CAPABILITY_ID", "SCHEMA_RUNTIME_CONTRACT"):
        if token not in text:
            out.append(Finding("ERROR", "schema-api", SCHEMA_API, f"missing {token}"))
    for method in REQUIRED_SCHEMA_METHODS:
        if method not in text:
            out.append(Finding("ERROR", "schema-api", SCHEMA_API, "missing schema service method", method))
    for dto in REQUIRED_SCHEMA_DTOS:
        if re.search(rf"pub\s+struct\s+{dto}\b", text) is None:
            out.append(Finding("ERROR", "schema-api", SCHEMA_API, "missing schema DTO", dto))
    service = read(SERVICE_API)
    for token in ("Schema", '"schema" => Some(Self::Schema)', 'Self::Schema => "engine.schema"'):
        if token not in service:
            out.append(Finding("ERROR", "schema-api", SERVICE_API, "EngineServiceKind must expose engine.schema", token))
    matrix, findings = load_json(CAPABILITY_MATRIX)
    out.extend(findings)
    if not findings:
        records = matrix.get("records") or []
        if not any(record.get("engine_gateway") == "engine.schema" and record.get("capability_id") == "schema.registry" for record in records if isinstance(record, dict)):
            out.append(Finding("ERROR", "schema-api", CAPABILITY_MATRIX, "capability matrix must describe engine.schema / schema.registry"))
    return out


def scan_schema_registry() -> list[Finding]:
    data, findings = load_json(SCHEMA_REGISTRY)
    if findings:
        return findings
    out: list[Finding] = []
    records = data.get("records")
    if not isinstance(records, list):
        return [Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, "records must be a list")]
    domains: set[str] = set()
    seen: set[str] = set()
    for idx, record in enumerate(records):
        if not isinstance(record, dict):
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] must be an object"))
            continue
        type_id = str(record.get("type_id", "")).strip()
        domain = str(record.get("domain", "")).strip()
        domains.add(domain)
        if not type_id:
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] missing type_id"))
        if type_id in seen:
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, "duplicate type_id", type_id))
        seen.add(type_id)
        if not str(record.get("owner_gateway", "")).startswith("engine."):
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] owner_gateway must be engine.*", type_id))
        if record.get("patch_validation") != "schema.validate_patch_v1":
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] must route patch validation through schema.validate_patch_v1", type_id))
        if record.get("transaction_dto") != "newengine.schema.transaction.v1":
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] must declare schema transaction DTO", type_id))
        props = record.get("properties") or []
        if not isinstance(props, list) or not props:
            out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}] must declare properties", type_id))
        for prop_idx, prop in enumerate(props if isinstance(props, list) else []):
            if not isinstance(prop, dict):
                out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"record[{idx}].properties[{prop_idx}] must be object", type_id))
                continue
            for field in ("property_id", "value_kind", "editable", "json_pointer"):
                if field not in prop:
                    out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, f"property missing {field}", f"{type_id}[{prop_idx}]"))
    missing_domains = sorted(REQUIRED_REGISTRY_DOMAINS - domains)
    for domain in missing_domains:
        out.append(Finding("ERROR", "schema-registry", SCHEMA_REGISTRY, "registry missing required domain", domain))
    return out


def scan_consumers() -> list[Finding]:
    out: list[Finding] = []
    asset_api = read(ASSET_DOCUMENT_API)
    for token in ("SchemaPropertyDescriptorV1", "SchemaTypeDescriptorV1", "SchemaPatchDtoV1", "SchemaTransactionDtoV1", "schema_property", "schema_type", "schema_patch", "transaction"):
        if token not in asset_api:
            out.append(Finding("ERROR", "asset-properties", ASSET_DOCUMENT_API, "AssetDocument DTO must bridge to schema DTOs", token))
    asset_service = read(ASSET_DOCUMENT_SERVICE)
    for token in ("asset_document_schema_type", "schema_patch_for_document", "schema_transaction_for_document", "schema.patch.target_mismatch", "schema.transaction.missing"):
        if token not in asset_service:
            out.append(Finding("ERROR", "asset-properties", ASSET_DOCUMENT_SERVICE, "asset inspect/edit service must produce and validate schema/transaction DTOs", token))
    screen = read_screen_profile_surface()
    for token in ("ENGINE_SCHEMA_SERVICE_ID", "right_edit_window.asset.schema", "schema-property", "asset_document_field_detail"):
        if token not in screen:
            out.append(Finding("ERROR", "inspector-schema", SCREEN_PROFILE, "Right Edit Window must render from schema-backed properties", token))
    scripting = read(SCRIPTING_API)
    for token in ("SchemaBindingManifestV1", "SCRIPTING_SERVICE_METHOD_BINDING_MANIFEST_JSON_V1", "ScriptingBindingGenerationRequest", "ScriptingBindingGenerationResponse"):
        if token not in scripting:
            out.append(Finding("ERROR", "scripting-schema", SCRIPTING_API, "scripting bindings must be generated from schema manifest", token))
    return out


def run_checks() -> list[Finding]:
    findings: list[Finding] = []
    findings.extend(scan_schema_api())
    findings.extend(scan_schema_registry())
    findings.extend(scan_consumers())
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p2_schema_property_scan.py")
    parser.add_argument("--summary-only", action="store_true")
    ns = parser.parse_args(argv)
    findings = run_checks()
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p2 schema/property scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
