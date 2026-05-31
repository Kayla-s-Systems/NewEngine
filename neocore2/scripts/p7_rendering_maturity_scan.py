#!/usr/bin/env python3
"""P7 rendering maturity source-level scanner."""
from __future__ import annotations

import json
import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

RENDER_CONFIG = ENGINE_ROOT / "config" / "render" / "render_maturity.v1.json"
CAP_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
RENDER_API_SRC = ENGINE_ROOT / "crates" / "newengine-render-api" / "src"
RENDER_API_LIB = RENDER_API_SRC / "lib.rs"
RENDER_API_CAPS = RENDER_API_SRC / "capabilities.rs"
TAKESOME_CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
TAKESOME_TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
TAKESOME_INVARIANTS = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "invariants.py"
TAKESOME_VALIDATION = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "validation.py"
SUITE_REGISTRY = REPO_ROOT / "tools" / "scripts" / "takesome" / "suite" / "registry.py"
AUDIT_DOC = REPO_ROOT / "docs" / "audits" / "P7_RENDERING_MATURITY_20260531.md"

REQUIRED_FEATURE_CAPS = {
    "render.material_shader_graph",
    "render.shader_variant_registry",
    "render.lighting_stack",
    "render.shadow_system",
    "render.postfx_stack",
    "render.probes.reflection",
    "render.vfx.particles",
    "render.terrain",
    "render.foliage",
    "render.lod",
    "render.occlusion",
    "render.debug.overlays",
}
REQUIRED_CONFIG_TOKENS = (
    "material_shader_graph",
    "shader_variant_registry",
    "lighting_shadows_postfx_probes",
    "particles_vfx",
    "terrain_foliage_lod_occlusion",
    "debug_overlays",
    "forbidden_bypasses",
)
REQUIRED_API_FILES = {
    "feature_registry.rs": ["RenderFeatureCapabilityDescriptor", "render_feature_capability", "render_feature_gateway"],
    "material_graph.rs": ["MaterialShaderGraphDto", "MaterialGraphValidationReport", "MaterialGraphNodeKind"],
    "shader_variants.rs": ["ShaderVariantRegistryDto", "ShaderVariantRecordDto", "ShaderVariantKeyDto"],
    "maturity.rs": ["RenderDebugOverlayKind", "LightingStackDescriptorDto", "ShadowSystemDescriptorDto", "VfxSystemDescriptorDto", "TerrainFoliageLodDescriptorDto"],
}
BYPASS_ROOTS = [
    ENGINE_ROOT / "crates" / "newengine-render-api",
    ENGINE_ROOT / "crates" / "newengine-render-feature-api",
    ENGINE_ROOT / "crates" / "newengine-model-runtime",
    ENGINE_ROOT / "crates" / "newengine-material-runtime",
    REPO_ROOT / "Plugins" / "VulkanRenderer",
]
SOURCE_PARSE_BYPASS = re.compile(
    r"ends_with\(\s*\"\.(?:fbx|obj|gltf|glb|png|jpg|jpeg|dds|tga|psd|blend|nemat|ydd|ytd|ytyp)\"|"
    r"extension\(\).*\.(?:fbx|obj|gltf|glb|png|jpg|jpeg|dds|tga|psd|blend|nemat|ydd|ytd|ytyp)",
    re.IGNORECASE,
)
ALLOW_LINE = re.compile(r"forbidden|scanner|scan|diagnostic|policy|source_policy|runtime requires", re.IGNORECASE)


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
    try:
        return path.relative_to(REPO_ROOT)
    except ValueError:
        return path


def load_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p7-rendering", rel(path), "required JSON file is missing")]
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {}, [Finding("ERROR", "p7-rendering", rel(path), f"invalid JSON: {exc}")]
    if not isinstance(value, dict):
        return {}, [Finding("ERROR", "p7-rendering", rel(path), "root must be a JSON object")]
    return value, []


def scan_render_config() -> list[Finding]:
    data, findings = load_json(RENDER_CONFIG)
    if findings:
        return findings
    text = json.dumps(data, sort_keys=True)
    for token in REQUIRED_CONFIG_TOKENS:
        if token not in text:
            findings.append(Finding("ERROR", "render-maturity-config", rel(RENDER_CONFIG), f"missing token {token}"))
    caps = {str(item.get("capability_id", "")) for item in data.get("feature_capabilities", []) if isinstance(item, dict)}
    missing = sorted(REQUIRED_FEATURE_CAPS - caps)
    if missing:
        findings.append(Finding("ERROR", "render-maturity-config", rel(RENDER_CONFIG), f"missing feature capabilities {missing}"))
    variants = data.get("shader_variant_registry", {}).get("variants", [])
    if not variants:
        findings.append(Finding("ERROR", "shader-variant-registry", rel(RENDER_CONFIG), "registry must contain at least one baseline variant"))
    if data.get("shader_variant_registry", {}).get("deterministic") is not True:
        findings.append(Finding("ERROR", "shader-variant-registry", rel(RENDER_CONFIG), "registry must declare deterministic=true"))
    return findings


def scan_capability_matrix() -> list[Finding]:
    data, findings = load_json(CAP_MATRIX)
    if findings:
        return findings
    caps = {str(item.get("capability_id", "")) for item in data.get("records", []) if isinstance(item, dict)}
    missing = sorted(REQUIRED_FEATURE_CAPS - caps)
    if missing:
        findings.append(Finding("ERROR", "capability-matrix", rel(CAP_MATRIX), f"missing P7 render capabilities {missing}"))
    for item in data.get("records", []):
        if not isinstance(item, dict):
            continue
        cap = str(item.get("capability_id", ""))
        if cap in REQUIRED_FEATURE_CAPS:
            diagnostics = item.get("diagnostics") or {}
            for key in ("must_show_active_route", "must_show_shadowed_routes", "must_show_capability_id"):
                if diagnostics.get(key) is not True:
                    findings.append(Finding("ERROR", "capability-matrix", rel(CAP_MATRIX), f"{cap} diagnostics.{key} must be true"))
            if item.get("fallback_policy") != "explicit_profile_only":
                findings.append(Finding("ERROR", "capability-matrix", rel(CAP_MATRIX), f"{cap} fallback_policy must be explicit_profile_only"))
    return findings


def scan_conformance_matrix() -> list[Finding]:
    data, findings = load_json(CONFORMANCE_MATRIX)
    if findings:
        return findings
    families = {str(item.get("family", "")): item for item in data.get("families", []) if isinstance(item, dict)}
    family = families.get("render.maturity")
    if not family:
        return [Finding("ERROR", "conformance-matrix", rel(CONFORMANCE_MATRIX), "missing render.maturity family")]
    required_tests = {
        "material_graph_is_descriptor_driven",
        "shader_variants_are_deterministic",
        "render_debug_overlays_are_dto_only",
        "renderer_does_not_parse_source_assets",
    }
    tests = set(family.get("test_cases") or [])
    missing_tests = sorted(required_tests - tests)
    if missing_tests:
        findings.append(Finding("ERROR", "conformance-matrix", rel(CONFORMANCE_MATRIX), f"render.maturity missing tests {missing_tests}"))
    return findings


def scan_render_api() -> list[Finding]:
    findings: list[Finding] = []
    lib = RENDER_API_LIB.read_text(encoding="utf-8", errors="replace") if RENDER_API_LIB.exists() else ""
    for module_name in ("feature_registry", "material_graph", "shader_variants", "maturity"):
        if f"mod {module_name};" not in lib or f"pub use {module_name}::*;" not in lib:
            findings.append(Finding("ERROR", "render-api", rel(RENDER_API_LIB), f"render API must expose {module_name}"))
    for filename, tokens in REQUIRED_API_FILES.items():
        path = RENDER_API_SRC / filename
        if not path.exists():
            findings.append(Finding("ERROR", "render-api", rel(path), "required P7 API file missing"))
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for token in tokens:
            if token not in text:
                findings.append(Finding("ERROR", "render-api", rel(path), f"missing token {token}"))
    caps_text = RENDER_API_CAPS.read_text(encoding="utf-8", errors="replace") if RENDER_API_CAPS.exists() else ""
    for token in ("MaterialShaderGraph", "ShaderVariantRegistry", "LightingStack", "ReflectionProbes", "ParticlesVfx", "TerrainRendering", "FoliageRendering", "LodSystem", "RenderDebugOverlays"):
        if token not in caps_text:
            findings.append(Finding("ERROR", "render-feature-enum", rel(RENDER_API_CAPS), f"RenderFeature missing {token}"))
    return findings


def scan_source_bypasses() -> list[Finding]:
    findings: list[Finding] = []
    for root in BYPASS_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            text = path.read_text(encoding="utf-8", errors="replace")
            for number, line in enumerate(text.splitlines(), start=1):
                if SOURCE_PARSE_BYPASS.search(line) and not ALLOW_LINE.search(line):
                    findings.append(Finding("ERROR", "source-parse-bypass", rel(path), f"renderer/model/material boundary parses source/runtime extension at line {number}", line.strip()))
    return findings


def scan_tooling() -> list[Finding]:
    findings: list[Finding] = []
    required = {
        TAKESOME_INVARIANTS: ["run_p7_rendering_maturity_scan", "p7_rendering_maturity_scan.py"],
        TAKESOME_VALIDATION: ["run_p7_rendering_maturity_scan", "render_code"],
        TAKESOME_TOOLS_RUN: ["rendering", "Run P7 rendering maturity scan"],
        TAKESOME_CLI: ["rendering"],
        SUITE_REGISTRY: ["diag.rendering", "run_p7_rendering_maturity_scan"],
        AUDIT_DOC: ["P7", "render.material_shader_graph", "render.debug.overlays"],
    }
    for path, tokens in required.items():
        text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
        if not path.exists():
            findings.append(Finding("ERROR", "p7-tooling", rel(path), "required tooling/audit file missing"))
            continue
        for token in tokens:
            if token not in text:
                findings.append(Finding("ERROR", "p7-tooling", rel(path), f"missing token {token}"))
    return findings


def main() -> int:
    findings: list[Finding] = []
    findings.extend(scan_render_config())
    findings.extend(scan_capability_matrix())
    findings.extend(scan_conformance_matrix())
    findings.extend(scan_render_api())
    findings.extend(scan_source_bypasses())
    findings.extend(scan_tooling())

    for finding in findings:
        print(finding.render())
    errors = sum(1 for finding in findings if finding.severity == "ERROR")
    warnings = sum(1 for finding in findings if finding.severity == "WARN")
    print(f"p7 rendering maturity scan: errors={errors} warnings={warnings}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
