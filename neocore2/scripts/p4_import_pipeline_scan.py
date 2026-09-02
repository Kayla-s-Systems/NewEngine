#!/usr/bin/env python3
"""P4 import/reimport/package pipeline source-level scanner."""
from __future__ import annotations

import json
import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

IMPORTERS = ENGINE_ROOT / "config" / "importers" / "importer_descriptors.v1.json"
PIPELINE_CONFIG = ENGINE_ROOT / "config" / "assets" / "import_pipeline.v1.json"
GENERATED_GRAPH = ENGINE_ROOT / "config" / "assets" / "import_pipeline.generated_graph.v1.json"
INVALIDATION_PLAN = ENGINE_ROOT / "config" / "assets" / "import_pipeline.invalidation_plan.v1.json"
CAP_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
PIPELINE_API = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "pipeline.rs"
ASSETS_API_LIB = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "lib.rs"
ASSETS_SERVICE_CLIENT = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "asset_service_client.rs"
ASSET_MANAGER_SERVICE = REPO_ROOT / "Plugins" / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "service.rs"
ASSET_MANAGER_IMPORT_HANDLERS = REPO_ROOT / "Plugins" / "AssetManager" / "newengine-AssetManager" / "src" / "module" / "service" / "handlers_import.rs"
ASSET_MANAGER_STORE = REPO_ROOT / "Plugins" / "AssetManager" / "newengine-AssetManager" / "src" / "asset_store" / "store.rs"
ASSET_MANAGER_SERVICE_DESCRIPTION = REPO_ROOT / "Plugins" / "AssetManager" / "newengine-AssetManager" / "assets" / "service_description.json"
IMPORT_PIPELINE_SCRIPT = REPO_ROOT / "tools" / "scripts" / "takesome" / "import_pipeline.py"
TAKESOME_CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
TAKESOME_TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
NEUI_PACKER_MANIFEST = REPO_ROOT / "tools" / "northstar" / "neui_packer" / "Cargo.toml"
NEUI_PACKER_TOOL = REPO_ROOT / "tools" / "northstar" / "neui_packer" / "tool.json"
NEUI_PACKER_MAIN = REPO_ROOT / "tools" / "northstar" / "neui_packer" / "src" / "main.rs"
LARGE_DEBT_LEDGER = ENGINE_ROOT / "config" / "invariants" / "p0_large_module_debt.v1.json"

BYPASS_ROOTS = [
    ENGINE_ROOT / "crates" / "newengine-render-api",
    ENGINE_ROOT / "crates" / "newengine-render-feature-api",
    ENGINE_ROOT / "crates" / "newengine-model-runtime",
    ENGINE_ROOT / "crates" / "newengine-material-runtime",
    REPO_ROOT / "Plugins" / "VulkanRenderer",
]
SOURCE_PARSE_BYPASS = re.compile(r"ends_with\(\s*\"\.(?:fbx|obj|gltf|glb|png|jpg|jpeg|dds|tga|psd|blend)\"|extension\(\).*\.(?:fbx|obj|gltf|glb|png|jpg|jpeg|dds|tga|psd|blend)", re.IGNORECASE)
ALLOW_LINE = re.compile(r"forbidden|scan|deny|diagnostic|importer descriptor", re.IGNORECASE)


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    message: str
    excerpt: str = ""

    def render(self) -> str:
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {self.path}: {self.message}{suffix}"


def rel(path: pathlib.Path) -> pathlib.Path:
    return path.relative_to(REPO_ROOT)


def read_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p4-import-pipeline", rel(path), "required JSON file is missing")]
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {}, [Finding("ERROR", "p4-import-pipeline", rel(path), f"invalid JSON: {exc}")]
    if not isinstance(value, dict):
        return {}, [Finding("ERROR", "p4-import-pipeline", rel(path), "root must be a JSON object")]
    return value, []


def scan_importer_descriptors() -> list[Finding]:
    data, findings = read_json(IMPORTERS)
    importers = data.get("importers", [])
    if not importers:
        findings.append(Finding("ERROR", "importer-descriptors", rel(IMPORTERS), "must declare at least one importer descriptor"))
    for index, desc in enumerate(importers):
        label = desc.get("importer_id") or f"#{index}"
        for key in ("importer_id", "source_extensions", "runtime_outputs", "owner_gateway", "cache_key_inputs"):
            if not desc.get(key):
                findings.append(Finding("ERROR", "importer-descriptors", rel(IMPORTERS), f"importer {label} missing {key}"))
        if desc.get("deterministic") is not True:
            findings.append(Finding("ERROR", "importer-descriptors", rel(IMPORTERS), f"importer {label} must declare deterministic=true"))
        inputs = set(desc.get("cache_key_inputs", []))
        required = {"source_path", "source_content_hash", "importer_id", "importer_version", "settings_hash", "target_platform"}
        missing = sorted(required - inputs)
        if missing:
            findings.append(Finding("ERROR", "importer-cache-key", rel(IMPORTERS), f"importer {label} cache key missing inputs {missing}"))
    return findings


def scan_pipeline_config() -> list[Finding]:
    data, findings = read_json(PIPELINE_CONFIG)
    text = json.dumps(data, sort_keys=True)
    for token in ("source_to_runtime_graph", "dependency_invalidation", "cache_key_policy", "assets.package_writer.nepak", "asset.package_write_nepak_json_v1", "descriptor_driven", "northstar.importer.yfd_font.v1", "invalidation_index"):
        if token not in text:
            findings.append(Finding("ERROR", "pipeline-config", rel(PIPELINE_CONFIG), f"missing pipeline token {token}"))
    return findings



def scan_generated_runtime_graph() -> list[Finding]:
    data, findings = read_json(GENERATED_GRAPH)
    text = json.dumps(data, sort_keys=True)
    for token in (
        "northstar.assets.runtime_graph.generated.v1",
        "northstar.importer.neui.v1",
        "northstar.importer.yfd_font.v1",
        "font_dictionary",
        "cache_keys",
        "invalidation_index",
        "worker_results",
    ):
        if token not in text:
            findings.append(Finding("ERROR", "runtime-asset-graph", rel(GENERATED_GRAPH), f"generated graph missing token {token}"))
    if not data.get("sources") or not data.get("runtime_assets"):
        findings.append(Finding("ERROR", "runtime-asset-graph", rel(GENERATED_GRAPH), "generated graph must contain sources and runtime_assets"))
    return findings


def scan_invalidation_plan_shape() -> list[Finding]:
    data, findings = read_json(INVALIDATION_PLAN)
    text = json.dumps(data, sort_keys=True)
    for token in ("northstar.assets.invalidation_plan.v1", "invalidated_cache_keys", "affected_runtime_assets"):
        if token not in text:
            findings.append(Finding("ERROR", "dependency-invalidation", rel(INVALIDATION_PLAN), f"invalidation plan missing token {token}"))
    return findings

def scan_assets_api() -> list[Finding]:
    findings: list[Finding] = []
    text = PIPELINE_API.read_text(encoding="utf-8", errors="replace") if PIPELINE_API.exists() else ""
    for token in (
        "ImporterDescriptorV1",
        "AssetRuntimeGraphV1",
        "AssetInvalidationPlanV1",
        "AssetCacheKeyV1",
        "NepakPackageWriteRequestV1",
        "NEPAK_PACKAGE_WRITER_CAPABILITY_ID",
        "ASSET_PACKAGE_WRITE_NEPAK_JSON_V1",
    ):
        if token not in text:
            findings.append(Finding("ERROR", "pipeline-dto", rel(PIPELINE_API), f"missing DTO/constant {token}"))
    return findings



def scan_package_writer_execution() -> list[Finding]:
    findings: list[Finding] = []
    required = {
        ASSETS_API_LIB: ["PACKAGE_WRITE_NEPAK_JSON_V1", "package_write_nepak_json_v1"],
        ASSETS_SERVICE_CLIENT: ["m_package_write_nepak_json_v1", "package_write_nepak_json_v1"],
        ASSET_MANAGER_SERVICE: ["method::PACKAGE_WRITE_NEPAK_JSON_V1", "handle_package_write_nepak_json_v1"],
        ASSET_MANAGER_IMPORT_HANDLERS: ["handle_package_write_nepak_json_v1", "NepakPackageWriteRequestV1"],
        ASSET_MANAGER_STORE: ["package_write_nepak_json", "write_raw_path_allow_create", "NEPAK_PACKAGE_WRITER_CAPABILITY_ID"],
        ASSET_MANAGER_SERVICE_DESCRIPTION: ["asset.package_write_nepak_json_v1"],
    }
    for path, tokens in required.items():
        text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
        if not path.exists():
            findings.append(Finding("ERROR", "package-writer-execution", rel(path), "required package writer execution file is missing"))
            continue
        for token in tokens:
            if token not in text:
                findings.append(Finding("ERROR", "package-writer-execution", rel(path), f"missing execution token {token}"))
    return findings


def scan_import_workers_and_neui_packer() -> list[Finding]:
    findings: list[Finding] = []
    required = {
        IMPORT_PIPELINE_SCRIPT: ["ImportWorkerResult", "deterministic_cache_key", "build_runtime_graph", "plan_invalidation", "combined_source_hash", "run_neui_packer", "import_pipeline_command"],
        TAKESOME_CLI: ["import-ui-assets", "import_pipeline_command", "changed-source", "write-invalidation-plan"],
        TAKESOME_TOOLS_RUN: ["import-ui-assets", "import_pipeline_command"],
        NEUI_PACKER_MANIFEST: ["northstar-neui-packer", "blake3", "flate2"],
        NEUI_PACKER_TOOL: ["northstar.neui_packer", "safe_for_build", "validation_args"],
        NEUI_PACKER_MAIN: ["NEF8", "DeflateEncoder", "blake3", "NeUiDictionary", "NeUiThemeLibrary"],
    }
    for path, tokens in required.items():
        text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
        if not path.exists():
            findings.append(Finding("ERROR", "import-worker", rel(path), "required import worker/packer file is missing"))
            continue
        for token in tokens:
            if token not in text:
                findings.append(Finding("ERROR", "import-worker", rel(path), f"missing worker/packer token {token}"))
    return findings


def scan_p0_debt_burn() -> list[Finding]:
    data, findings = read_json(LARGE_DEBT_LEDGER)
    text = json.dumps(data, sort_keys=True)
    if "screen_profile.rs" in text and "resolved_in_this_pass" not in text:
        findings.append(Finding("ERROR", "p0-debt-burn", rel(LARGE_DEBT_LEDGER), "screen_profile.rs remains tracked without resolution record"))
    for path in (
        ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "screen_profile.rs",
        ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "ui_gateway_frame.rs",
    ):
        if not path.exists():
            findings.append(Finding("ERROR", "p0-debt-burn", rel(path), "split owner file is missing"))
            continue
        loc = sum(1 for _ in path.open("r", encoding="utf-8", errors="replace"))
        if loc > 550:
            findings.append(Finding("ERROR", "p0-debt-burn", rel(path), f"split owner file still exceeds 550 LOC: {loc}"))
    return findings


def scan_conformance_matrix() -> list[Finding]:
    data, findings = read_json(CONFORMANCE_MATRIX)
    text = json.dumps(data, sort_keys=True)
    for token in ("asset.package_write_nepak_json_v1", "package_writer_explicit_capability_required", "package_write_is_deterministic"):
        if token not in text:
            findings.append(Finding("ERROR", "package-writer-conformance", rel(CONFORMANCE_MATRIX), f"missing conformance token {token}"))
    return findings


def scan_capability_matrix() -> list[Finding]:
    data, findings = read_json(CAP_MATRIX)
    records = data.get("records", [])
    if not any(r.get("capability_id") == "assets.package_writer.nepak" for r in records):
        findings.append(Finding("ERROR", "package-writer-capability", rel(CAP_MATRIX), "missing explicit .nepak package writer capability"))
    return findings


def scan_source_parse_bypasses() -> list[Finding]:
    findings: list[Finding] = []
    for root in BYPASS_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if "third_party" in path.parts:
                continue
            for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1):
                if SOURCE_PARSE_BYPASS.search(line) and not ALLOW_LINE.search(line):
                    findings.append(Finding("ERROR", "source-parse-bypass", rel(path), f"renderer/model/material layer must not parse source extensions directly at line {idx}", line))
    return findings


def main() -> int:
    findings: list[Finding] = []
    findings.extend(scan_importer_descriptors())
    findings.extend(scan_pipeline_config())
    findings.extend(scan_assets_api())
    findings.extend(scan_generated_runtime_graph())
    findings.extend(scan_invalidation_plan_shape())
    findings.extend(scan_package_writer_execution())
    findings.extend(scan_import_workers_and_neui_packer())
    findings.extend(scan_p0_debt_burn())
    findings.extend(scan_conformance_matrix())
    findings.extend(scan_capability_matrix())
    findings.extend(scan_source_parse_bypasses())
    for finding in findings:
        print(finding.render())
    errors = [f for f in findings if f.severity == "ERROR"]
    print(f"p4 import pipeline scan: errors={len(errors)} warnings=0")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
