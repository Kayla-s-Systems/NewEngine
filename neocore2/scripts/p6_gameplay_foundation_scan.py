#!/usr/bin/env python3
"""P6 gameplay foundation source-level scanner."""
from __future__ import annotations

import json
import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

CRATES = ENGINE_ROOT / "crates"
SERVICE_API = CRATES / "newengine-service-api" / "src" / "lib.rs"
CAP_MATRIX = ENGINE_ROOT / "config" / "capabilities" / "engine_capability_matrix.v1.json"
CONFORMANCE_MATRIX = ENGINE_ROOT / "config" / "conformance" / "provider_conformance_matrix.v1.json"
GAMEPLAY_CONFIG = ENGINE_ROOT / "config" / "gameplay" / "gameplay_foundation.v1.json"
GAMEPLAY_RUNTIME = CRATES / "newengine-gameplay-runtime" / "src"
RUNTIME_HOST_CARGO = CRATES / "newengine-runtime-host" / "Cargo.toml"
GAME_READY_CARGO = CRATES / "newengine-game-ready-profile" / "Cargo.toml"
GAME_READY_PROFILE = CRATES / "newengine-game-ready-profile" / "src" / "lib.rs"
HEADLESS = CRATES / "newengine-runtime-host" / "src" / "headless_cli.rs"
PLATFORM_RUNTIME = CRATES / "newengine-runtime-host" / "src" / "platform_runtime" / "runtime_host.rs"
NULL_PROVIDERS = CRATES / "newengine-runtime-host" / "src" / "null_providers.rs"
TAKESOME_INVARIANTS = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "invariants.py"
TAKESOME_VALIDATION = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "validation.py"
TAKESOME_TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
TAKESOME_CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
SUITE_REGISTRY = REPO_ROOT / "tools" / "scripts" / "takesome" / "suite" / "registry.py"
AUDIT_DOC = REPO_ROOT / "docs" / "audits" / "P6_GAMEPLAY_FOUNDATION_20260531.md"

REQUIRED_API_CRATES = {
    "newengine-animation-api": ["ENGINE_ANIMATION_SERVICE_ID", "AnimationIntentDtoV1", "ANIMATION_BACKEND_CAPABILITY_ID"],
    "newengine-navigation-api": ["ENGINE_NAVIGATION_SERVICE_ID", "NavPlanPathRequestV1", "NAVIGATION_BACKEND_CAPABILITY_ID"],
    "newengine-ai-api": ["ENGINE_AI_SERVICE_ID", "AiFrameInputV1", "AiFrameOutputV1", "AiIntentDtoV1", "AI_BACKEND_CAPABILITY_ID"],
    "newengine-tags-api": ["ENGINE_TAGS_SERVICE_ID", "TagDescriptorV1", "TagSetSnapshotV1", "TAGS_REGISTRY_CAPABILITY_ID"],
    "newengine-tasks-api": ["ENGINE_TASKS_SERVICE_ID", "TaskDescriptorV1", "TaskRequestDtoV1", "TASKS_BACKEND_CAPABILITY_ID"],
}
REQUIRED_GATEWAYS = {
    "engine.animation": "animation.backend",
    "engine.navigation": "navigation.backend",
    "engine.ai": "ai.backend",
    "engine.tags": "tags.registry",
    "engine.tasks": "tasks.backend",
}
REQUIRED_METHODS = [
    "ai.frame_json_v1",
    "ai.validate_intents_json_v1",
    "animation.plan_json_v1",
    "navigation.plan_path_json_v1",
    "tags.describe_tags_json_v1",
    "tasks.plan_queue_json_v1",
]
FORBIDDEN_AI_SOURCE_PATTERNS = [
    re.compile(r"newengine_ecs::"),
    re.compile(r"&\s*mut\s+World\b"),
    re.compile(r"use\s+newengine_entity_api::EntityId\b"),
]

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
        return {}, [Finding("ERROR", "p6-json", rel(path), "required JSON file is missing")]
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        return {}, [Finding("ERROR", "p6-json", rel(path), f"invalid JSON: {exc}")]
    if not isinstance(value, dict):
        return {}, [Finding("ERROR", "p6-json", rel(path), "root must be a JSON object")]
    return value, []


def require_tokens(path: pathlib.Path, check: str, tokens: list[str]) -> list[Finding]:
    if not path.exists():
        return [Finding("ERROR", check, rel(path), "required file is missing")]
    text = read(path)
    return [Finding("ERROR", check, rel(path), f"missing token {token}") for token in tokens if token not in text]


def scan_api_crates() -> list[Finding]:
    findings: list[Finding] = []
    for crate, tokens in REQUIRED_API_CRATES.items():
        crate_root = CRATES / crate
        findings += require_tokens(crate_root / "Cargo.toml", "p6-api-crate", [f'name = "{crate}"'])
        findings += require_tokens(crate_root / "src" / "lib.rs", "p6-api-contract", tokens)
    findings += require_tokens(SERVICE_API, "p6-service-kind", [
        "EngineServiceKind::Animation", "EngineServiceKind::Navigation", "EngineServiceKind::Ai", "EngineServiceKind::Tags", "EngineServiceKind::Tasks",
        '"engine.animation"', '"engine.navigation"', '"engine.ai"', '"engine.tags"', '"engine.tasks"',
    ])
    return findings


def scan_matrices() -> list[Finding]:
    findings: list[Finding] = []
    cap, f = read_json(CAP_MATRIX); findings += f
    conf, f = read_json(CONFORMANCE_MATRIX); findings += f
    cap_text = json.dumps(cap, sort_keys=True)
    conf_text = json.dumps(conf, sort_keys=True)
    for gateway, capability in REQUIRED_GATEWAYS.items():
        if gateway not in cap_text:
            findings.append(Finding("ERROR", "p6-capability", rel(CAP_MATRIX), f"missing gateway {gateway}"))
        if capability not in cap_text:
            findings.append(Finding("ERROR", "p6-capability", rel(CAP_MATRIX), f"missing capability {capability}"))
    for family in ("animation", "navigation", "ai", "tags", "tasks"):
        if f'"family": "{family}"' not in conf_text:
            findings.append(Finding("ERROR", "p6-conformance", rel(CONFORMANCE_MATRIX), f"missing provider family {family}"))
    for token in ("ai_emits_intents_not_mutations", "ai_provider_never_receives_world", "navigation_returns_path_dtos", "tasks_are_declarative", "tags_are_data_driven"):
        if token not in conf_text:
            findings.append(Finding("ERROR", "p6-conformance", rel(CONFORMANCE_MATRIX), f"missing conformance token {token}"))
    return findings


def scan_gameplay_runtime() -> list[Finding]:
    findings: list[Finding] = []
    findings += require_tokens(CRATES / "newengine-gameplay-runtime" / "Cargo.toml", "p6-runtime-crate", [
        'name = "newengine-gameplay-runtime"', "newengine-ai-api", "newengine-animation-api", "newengine-navigation-api", "newengine-tags-api", "newengine-tasks-api",
    ])
    findings += require_tokens(GAMEPLAY_RUNTIME / "lib.rs", "p6-runtime-lib", [
        "register_gameplay_foundation_gateways_best_effort", "TAGS_PROVIDER_ROUTE", "TASKS_PROVIDER_ROUTE", "ANIMATION_PROVIDER_ROUTE", "NAVIGATION_PROVIDER_ROUTE", "AI_PROVIDER_ROUTE",
    ])
    findings += require_tokens(GAMEPLAY_RUNTIME / "service.rs", "p6-runtime-services", [
        "register_tags_gateway", "register_tasks_gateway", "register_animation_gateway", "register_navigation_gateway", "register_ai_gateway", "register_engine_gateway_provider_service", "AiFrameInputV1", "AiFrameOutputV1",
    ])
    findings += require_tokens(GAMEPLAY_RUNTIME / "state.rs", "p6-runtime-state", [
        "ai_frame", "AiIntentDtoV1", "AiIntentKind", "runtime apply stage owns mutation", "describe_tags", "plan_queue", "plan_path",
    ])
    findings += require_tokens(RUNTIME_HOST_CARGO, "p6-runtime-host-dep", ["newengine-gameplay-runtime"])
    findings += require_tokens(GAME_READY_CARGO, "p6-profile-dep", ["newengine-gameplay-runtime"])
    for path in (GAME_READY_PROFILE, HEADLESS, PLATFORM_RUNTIME):
        findings += require_tokens(path, "p6-route-registration", ["register_gameplay_foundation_gateways_best_effort"])
    return findings


def scan_gameplay_config() -> list[Finding]:
    data, findings = read_json(GAMEPLAY_CONFIG)
    text = json.dumps(data, sort_keys=True)
    for token in ("npc.police", "state.alert", "move_to", "play_animation", "humanoid.locomotion", "emit intents only"):
        if token not in text:
            findings.append(Finding("ERROR", "p6-gameplay-config", rel(GAMEPLAY_CONFIG), f"missing token {token}"))
    if len(data.get("tags", [])) < 5:
        findings.append(Finding("ERROR", "p6-gameplay-config", rel(GAMEPLAY_CONFIG), "expected at least 5 gameplay tag descriptors"))
    if len(data.get("tasks", [])) < 5:
        findings.append(Finding("ERROR", "p6-gameplay-config", rel(GAMEPLAY_CONFIG), "expected at least 5 gameplay task descriptors"))
    return findings


def scan_ai_boundary() -> list[Finding]:
    findings: list[Finding] = []
    source_paths = list((CRATES / "newengine-ai-api" / "src").glob("**/*.rs")) + list(GAMEPLAY_RUNTIME.glob("**/*.rs"))
    for path in source_paths:
        for idx, line in enumerate(read(path).splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("//") or stripped.startswith("//!") or stripped.startswith("///"):
                continue
            for pattern in FORBIDDEN_AI_SOURCE_PATTERNS:
                if pattern.search(line):
                    findings.append(Finding("ERROR", "ai-no-direct-mutation-boundary", rel(path), f"forbidden AI/provider boundary token at line {idx}", line))
    null_text = read(NULL_PROVIDERS)
    if "engine.ai.null" not in null_text or "empty-intents" not in null_text:
        findings.append(Finding("ERROR", "ai-null-provider", rel(NULL_PROVIDERS), "NullAI must remain a visible provider route that returns empty intents"))
    return findings


def scan_tooling() -> list[Finding]:
    findings: list[Finding] = []
    for path, tokens in {
        TAKESOME_INVARIANTS: ["run_p6_gameplay_foundation_scan"],
        TAKESOME_VALIDATION: ["run_p6_gameplay_foundation_scan"],
        TAKESOME_TOOLS_RUN: ["gameplay", "run_p6_gameplay_foundation_scan"],
        TAKESOME_CLI: ["gameplay"],
        SUITE_REGISTRY: ["diag.gameplay", "run_p6_gameplay_foundation_scan"],
    }.items():
        findings += require_tokens(path, "p6-tooling", tokens)
    findings += require_tokens(AUDIT_DOC, "p6-audit-doc", ["P6", "engine.ai", "AI emits intents", "reference archives"])
    return findings


def run_checks() -> list[Finding]:
    findings: list[Finding] = []
    findings += scan_api_crates()
    findings += scan_matrices()
    findings += scan_gameplay_runtime()
    findings += scan_gameplay_config()
    findings += scan_ai_boundary()
    findings += scan_tooling()
    return findings


def main(argv: list[str]) -> int:
    findings = run_checks()
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    for finding in findings:
        print(finding.render())
    print(f"p6 gameplay foundation scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
