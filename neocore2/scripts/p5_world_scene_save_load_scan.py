#!/usr/bin/env python3
"""P5 world/scene/save-load/prefab parity source-level scanner."""
from __future__ import annotations

import json
import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

WORLD_API = ENGINE_ROOT / "crates" / "newengine-world-api" / "src" / "lib.rs"
WORLD_RUNTIME = ENGINE_ROOT / "crates" / "newengine-world-runtime" / "src" / "lib.rs"
WORLD_RUNTIME_APPLY = ENGINE_ROOT / "crates" / "newengine-world-runtime" / "src" / "apply_stage.rs"
WORLD_RUNTIME_STREAMING = ENGINE_ROOT / "crates" / "newengine-world-runtime" / "src" / "streaming_cells.rs"
SCENE_IO_CONSTS = ENGINE_ROOT / "crates" / "newengine-scene-io" / "src" / "consts.rs"
SCENE_IO_CLIENT = ENGINE_ROOT / "crates" / "newengine-scene-io" / "src" / "scene_io_client.rs"
SCENE_RUNTIME = ENGINE_ROOT / "crates" / "newengine-scene-runtime" / "src" / "lib.rs"
SCENE_RUNTIME_INSTANTIATION = ENGINE_ROOT / "crates" / "newengine-scene-runtime" / "src" / "instantiation.rs"
SCENE_COMPONENTS = ENGINE_ROOT / "crates" / "newengine-scene" / "src" / "components.rs"
SCENE_ASSET = ENGINE_ROOT / "crates" / "newengine-scene" / "src" / "scene_asset.rs"
SCENE_LIB = ENGINE_ROOT / "crates" / "newengine-scene" / "src" / "lib.rs"
CAP_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
P5_CONFIG = ENGINE_ROOT / "config" / "world" / "world_scene_save_load_pipeline.v1.json"
TAKESOME_CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
TAKESOME_TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
TAKESOME_INVARIANTS = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "invariants.py"
TAKESOME_VALIDATION = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "validation.py"
SUITE_REGISTRY = REPO_ROOT / "tools" / "scripts" / "takesome" / "suite" / "registry.py"

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


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def read_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p5-world-scene", rel(path), "required JSON file is missing")]
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {}, [Finding("ERROR", "p5-world-scene", rel(path), f"invalid JSON: {exc}")]
    if not isinstance(value, dict):
        return {}, [Finding("ERROR", "p5-world-scene", rel(path), "root must be a JSON object")]
    return value, []


def require_tokens(path: pathlib.Path, check: str, tokens: list[str]) -> list[Finding]:
    findings: list[Finding] = []
    text = read(path)
    if not path.exists():
        return [Finding("ERROR", check, rel(path), "required source file is missing")]
    for token in tokens:
        if token not in text:
            findings.append(Finding("ERROR", check, rel(path), f"missing token {token}"))
    return findings


def scan_world_api() -> list[Finding]:
    return require_tokens(WORLD_API, "world-api-contract", [
        "WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1",
        "WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1",
        "WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1",
        "WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1",
        "WorldStreamingCellsRequest",
        "WorldStreamingCellsResponse",
        "WorldApplyStageRequest",
        "WorldApplyStageResponse",
        "WorldSaveSnapshotRequest",
        "WorldLoadSnapshotRequest",
        "EntityHandle",
    ])


def scan_world_runtime() -> list[Finding]:
    findings = require_tokens(WORLD_RUNTIME, "world-runtime-route", [
        "WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1",
        "WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1",
        "WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1",
        "WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1",
    ])
    findings += require_tokens(WORLD_RUNTIME_STREAMING, "world-streaming-cells", [
        "streaming_cells_json_v1",
        "WorldStreamingCellsResponse",
        "WorldStreamingPlanDto",
    ])
    findings += require_tokens(WORLD_RUNTIME_APPLY, "world-apply-stage", [
        "apply_stage_json_v1",
        "save_snapshot_json_v1",
        "load_snapshot_json_v1",
        "scene.spawn_instance",
        "scene.to_asset",
        "scene.load_asset",
        "WorldApplyStageResponse",
    ])
    text = read(WORLD_RUNTIME) + read(WORLD_RUNTIME_STREAMING) + read(WORLD_RUNTIME_APPLY)
    if "newengine_ecs::World" in text or "newengine_ecs::EntityId" in text:
        findings.append(Finding("ERROR", "world-runtime-boundary", rel(WORLD_RUNTIME), "engine.world runtime service must not expose native ECS imports in service surface"))
    return findings


def scan_scene_instantiation() -> list[Finding]:
    findings: list[Finding] = []
    findings += require_tokens(SCENE_IO_CONSTS, "scene-io-methods", [
        "INSTANTIATE_PREFAB_JSON_V1",
        "INSTANTIATE_ARCHETYPE_JSON_V1",
    ])
    findings += require_tokens(SCENE_IO_CLIENT, "scene-io-client", [
        "instantiate_prefab_json_v1",
        "instantiate_archetype_json_v1",
    ])
    findings += require_tokens(SCENE_RUNTIME, "scene-instantiation-route", [
        "INSTANTIATE_PREFAB_JSON_V1",
        "INSTANTIATE_ARCHETYPE_JSON_V1",
    ])
    findings += require_tokens(SCENE_RUNTIME_INSTANTIATION, "scene-instantiation-plan", [
        "instantiate_prefab_json_v1",
        "instantiate_archetype_json_v1",
        "newengine.scene.instantiation_plan.v1",
        "mutates_scene",
        "apply_gateway",
        "WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1",
        "deterministic_instance_guid",
    ])
    text = read(SCENE_RUNTIME_INSTANTIATION)
    if re.search(r"fn instantiate_(?:prefab|archetype)_json_v1[\s\S]*scene\.load_asset", text):
        findings.append(Finding("ERROR", "scene-instantiation-mutates", rel(SCENE_RUNTIME), "scene instantiation planner must not call scene.load_asset; world apply stage owns mutation"))
    return findings


def scan_scene_asset_definition_refs() -> list[Finding]:
    findings: list[Finding] = []
    findings += require_tokens(SCENE_COMPONENTS, "scene-definition-ref", ["pub struct DefinitionRef", "Authored definition/archetype reference"])
    findings += require_tokens(SCENE_ASSET, "scene-asset-prefab-parity", [
        "DefinitionRef",
        "definition_ref",
        "world.get::<DefinitionRef>",
        "world.insert(id, DefinitionRef",
    ])
    findings += require_tokens(SCENE_LIB, "scene-public-contract", ["DefinitionRef", "SceneEntityAsset", "TransformAsset"])
    return findings


def scan_config_and_matrices() -> list[Finding]:
    findings: list[Finding] = []
    data, f = read_json(P5_CONFIG)
    findings += f
    text = json.dumps(data, sort_keys=True)
    for token in (
        "world.streaming_cells_json_v1",
        "world.apply_stage_json_v1",
        "world.save_snapshot_json_v1",
        "world.load_snapshot_json_v1",
        "scene.instantiate_prefab_json_v1",
        "scene.instantiate_archetype_json_v1",
        "native_entity_id_allowed_in_service_boundary",
    ):
        if token not in text:
            findings.append(Finding("ERROR", "p5-config", rel(P5_CONFIG), f"missing config token {token}"))
    cap, f = read_json(CAP_MATRIX)
    findings += f
    cap_text = json.dumps(cap, sort_keys=True)
    for token in ("scene.instantiation.backend", "world.streaming.cells", "world.apply_stage", "world.save_load.snapshots"):
        if token not in cap_text:
            findings.append(Finding("ERROR", "p5-capability", rel(CAP_MATRIX), f"missing capability {token}"))
    conf, f = read_json(CONFORMANCE_MATRIX)
    findings += f
    conf_text = json.dumps(conf, sort_keys=True)
    for token in ("scene.instantiate_prefab_json_v1", "world.apply_stage_json_v1", "save_load_snapshot_roundtrip"):
        if token not in conf_text:
            findings.append(Finding("ERROR", "p5-conformance", rel(CONFORMANCE_MATRIX), f"missing conformance token {token}"))
    return findings


def scan_tooling() -> list[Finding]:
    findings: list[Finding] = []
    for path, tokens in {
        TAKESOME_INVARIANTS: ["run_p5_world_scene_save_load_scan"],
        TAKESOME_VALIDATION: ["run_p5_world_scene_save_load_scan"],
        TAKESOME_TOOLS_RUN: ["world-scene", "run_p5_world_scene_save_load_scan"],
        TAKESOME_CLI: ["world-scene"],
        SUITE_REGISTRY: ["diag.world.scene", "run_p5_world_scene_save_load_scan"],
    }.items():
        findings += require_tokens(path, "p5-tooling", tokens)
    return findings


def scan_boundary_leaks() -> list[Finding]:
    findings: list[Finding] = []
    for path in (WORLD_API, SCENE_IO_CONSTS, SCENE_IO_CLIENT):
        text = read(path)
        for token in ("newengine_ecs", "EntityId", "&mut World"):
            if token in text:
                findings.append(Finding("ERROR", "p5-service-boundary", rel(path), f"service API boundary leaks {token}"))
    return findings


def main() -> int:
    findings: list[Finding] = []
    findings += scan_world_api()
    findings += scan_world_runtime()
    findings += scan_scene_instantiation()
    findings += scan_scene_asset_definition_refs()
    findings += scan_config_and_matrices()
    findings += scan_tooling()
    findings += scan_boundary_leaks()
    for finding in findings:
        print(finding.render())
    errors = sum(1 for f in findings if f.severity == "ERROR")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    print(f"p5 world/scene/save-load scan: errors={errors} warnings={warnings}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
