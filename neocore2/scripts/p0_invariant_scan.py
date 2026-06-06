#!/usr/bin/env python3
"""P0 architecture invariant scanner for North Star Engine.

This scanner is intentionally broader than the narrow legacy/provider scans. It
checks the foundation pass that must stay green before adding schema/reflection,
Inspector, scripting or editor mutation layers.
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

TEXT_SUFFIXES = {".rs", ".py", ".toml", ".json", ".md", ".txt", ".bat", ".cmd", ".ps1", ".yml", ".yaml"}
CODE_SUFFIXES = {".rs", ".py", ".toml", ".json", ".yml", ".yaml", ".bat", ".cmd", ".ps1"}
SKIP_DIRS = {".git", ".takesome", ".northstar", "target", "node_modules", "third_party", "__pycache__", "logs", "cache", "dist", "out", "bin", "obj", "artifacts"}

PROVIDER_IDS = (
    "render.api",
    "physics.api",
    "ui.api",
    "asset_manager.api",
    "time.api",
    "materials.api",
    "model.api",
    "ecs.api",
    "entity.api",
    "input.api",
    "ai.api",
)

API_ID_CONSTANT_FILES = (
    "newengine-render-api/src/constants.rs",
    "newengine-physics-api/src/lib.rs",
    "newengine-ui-api/src/draw_protocol.rs",
    "newengine-assets-api/src/lib.rs",
    "newengine-time-api/src/lib.rs",
    "newengine-materials/src/service.rs",
    "newengine-model-domain-api/src/lib.rs",
    "newengine-ecs-api/src/lib.rs",
    "newengine-entity-api/src/lib.rs",
    "newengine-ai-api/src/lib.rs",
)

ALLOW_PROVIDER_ID_PARTS = (
    pathlib.Path("tools/scripts/northstar_bridge"),  # diagnostic tooling scans provider/gateway ids as data
    pathlib.Path("NewEngine/neocore2/scripts"),
    pathlib.Path("NewEngine/neocore2/config/capabilities"),
    pathlib.Path("NewEngine/neocore2/config/conformance"),
    pathlib.Path("NewEngine/neocore2/crates/newengine-plugin-host/src/service_gateway/registry/tests.rs"),
    pathlib.Path("NewEngine/neocore2/crates/newengine-service-api/src/lib.rs"),
)

ALLOW_PROVIDER_ID_DOC_DIRS = (
    pathlib.Path("docs/architecture"),
    pathlib.Path("docs/audits"),
)

PUBLIC_NEYTD_ALLOW_PATHS = (
    pathlib.Path("NewEngine/neocore2/scripts/no_legacy_scan.py"),
    pathlib.Path("NewEngine/neocore2/scripts/p0_invariant_scan.py"),
)

PUBLIC_NEYTD_ALLOW_LINE = re.compile(r"reject|retired|forbidden|not public|legacy/cache|migration|deny", re.IGNORECASE)

HIDDEN_FALLBACK_PATTERNS = (
    (re.compile(r"unwrap_or_else\s*\([^\n]*(?:Null|Fallback)"), "hidden fallback construction"),
    (re.compile(r"InternalNull[A-Za-z0-9_]*"), "internal null provider construction"),
    (re.compile(r"local\s+scene\s+clock\s+projection|local_projection", re.IGNORECASE), "local projection fallback instead of engine gateway authority"),
    (re.compile(r"missing\s+route[^\n]+local", re.IGNORECASE), "missing route uses local implementation"),
)

FALLBACK_FORBID_WORDS = re.compile(r"forbidden|not allowed|no hidden|without hidden|requires engine\.jobs", re.IGNORECASE)

SERVICE_BOUNDARY_CRATE_RE = re.compile(r"newengine-[a-z0-9-]+-(?:api|contracts)$|newengine-(?:assets|materials|model-domain|physics|render|ui|world-environment|ecs|entity|time)-api$")
SERVICE_BOUNDARY_PATTERNS = (
    (re.compile(r"&\s*mut\s+World\b"), "&mut World crosses service/API boundary"),
    (re.compile(r"newengine_ecs::World\b"), "concrete ECS World appears in service/API boundary"),
    (re.compile(r"newengine_ecs::EntityId\b"), "native ECS EntityId appears in service/API boundary"),
)

ENTITY_ID_IMPORT_RE = re.compile(r"use\s+newengine_entity_api::EntityId\b")
ENTITY_ID_TOKEN_RE = re.compile(r"\bEntityId\b")

ENTITY_ID_ALLOW_API_REL = pathlib.Path("NewEngine/neocore2/crates/newengine-entity-api/src/lib.rs")
ENTITY_ID_ALLOWED_NAMES = {"EntityId", "ENTITY_SERVICE_ID"}

LARGE_FILE_LIMIT = 550
LARGE_DEBT_LEDGER = ENGINE_ROOT / "config" / "invariants" / "p0_large_module_debt.v1.json"
LARGE_FILE_ALLOW_SUFFIXES = (
    "registry/tests.rs",
)


def load_large_debt_ledger() -> dict[str, dict]:
    if not LARGE_DEBT_LEDGER.exists():
        return {}
    try:
        data = json.loads(LARGE_DEBT_LEDGER.read_text(encoding="utf-8"))
    except Exception:
        return {}
    return {str(entry.get("path", "")): entry for entry in data.get("entries", []) if entry.get("path")}


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    line: int
    message: str
    excerpt: str = ""

    def render(self) -> str:
        loc = f"{self.path}:{self.line}" if self.line else str(self.path)
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {loc}: {self.message}{suffix}"


def rel(path: pathlib.Path) -> pathlib.Path:
    return path.relative_to(REPO_ROOT)


def iter_text_files() -> Iterable[pathlib.Path]:
    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        yield path


def iter_code_files() -> Iterable[pathlib.Path]:
    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.suffix.lower() not in CODE_SUFFIXES:
            continue
        yield path


def is_relative_to(path: pathlib.Path, parent: pathlib.Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def is_provider_id_allowed(rel_path: pathlib.Path) -> bool:
    normalized = rel_path.as_posix()
    if any(normalized.endswith(suffix) for suffix in API_ID_CONSTANT_FILES):
        return True
    if any(rel_path == p or is_relative_to(rel_path, p) for p in ALLOW_PROVIDER_ID_PARTS):
        return True
    if any(is_relative_to(rel_path, p) for p in ALLOW_PROVIDER_ID_DOC_DIRS):
        return True
    if "/assets/service_description.json" in normalized:
        return True
    if "/docs/" in normalized and ("provider" in normalized.lower() or "gateway" in normalized.lower()):
        return True
    return False


def is_provider_impl_path(rel_path: pathlib.Path) -> bool:
    parts = rel_path.parts
    return bool(parts and parts[0] in {"Plugins", "Importers"})


def scan_public_neytd() -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_text_files():
        rp = rel(path)
        text = path.read_text(encoding="utf-8", errors="replace")
        for idx, line in enumerate(text.splitlines(), start=1):
            if not re.search(r"\.neytd|NEYTD|neytd", line):
                continue
            if rp in PUBLIC_NEYTD_ALLOW_PATHS:
                continue
            if PUBLIC_NEYTD_ALLOW_LINE.search(line):
                continue
            findings.append(Finding("ERROR", "no-public-neytd", rp, idx, ".neytd/NEYTD must not appear in live public/runtime surface", line))
    return findings


def scan_direct_provider_ids() -> list[Finding]:
    findings: list[Finding] = []
    provider_literal = re.compile(r'"(?:' + "|".join(re.escape(x) for x in PROVIDER_IDS) + r')"')
    direct_call = re.compile(r"call_service_v1\s*\(\s*\"(?:" + "|".join(re.escape(x) for x in PROVIDER_IDS) + r")\"")
    for path in iter_code_files():
        rp = rel(path)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for idx, line in enumerate(lines, start=1):
            if direct_call.search(line):
                findings.append(Finding("ERROR", "direct-provider-id", rp, idx, "consumer calls provider service id directly; use engine.* gateway", line))
                continue
            if provider_literal.search(line) and not is_provider_id_allowed(rp) and not is_provider_impl_path(rp):
                findings.append(Finding("ERROR", "direct-provider-id", rp, idx, "provider service id literal outside API constants/provider descriptors/tests", line))
    return findings


def is_comment_or_doc_line(line: str) -> bool:
    stripped = line.strip()
    return stripped.startswith("//") or stripped.startswith("#") or stripped.startswith("*") or stripped.startswith("//!") or stripped.startswith("///")


def scan_hidden_fallbacks() -> list[Finding]:
    findings: list[Finding] = []
    for path in iter_code_files():
        rp = rel(path)
        if rp == pathlib.Path("NewEngine/neocore2/scripts/p0_invariant_scan.py"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for idx, line in enumerate(text.splitlines(), start=1):
            if is_comment_or_doc_line(line) or FALLBACK_FORBID_WORDS.search(line):
                continue
            for pattern, message in HIDDEN_FALLBACK_PATTERNS:
                if pattern.search(line):
                    findings.append(Finding("ERROR", "no-hidden-fallback", rp, idx, message, line))
    return findings


def is_service_boundary_file(path: pathlib.Path) -> bool:
    try:
        rel_to_crates = path.relative_to(ENGINE_ROOT / "crates")
    except ValueError:
        return False
    crate_name = rel_to_crates.parts[0]
    return bool(SERVICE_BOUNDARY_CRATE_RE.match(crate_name))


def scan_service_boundaries(strict: bool) -> list[Finding]:
    findings: list[Finding] = []
    for path in (ENGINE_ROOT / "crates").glob("*/src/**/*.rs"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if not is_service_boundary_file(path):
            continue
        rp = rel(path)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for idx, line in enumerate(lines, start=1):
            if is_comment_or_doc_line(line):
                continue
            if rp == ENTITY_ID_ALLOW_API_REL:
                # The entity API is the one crate allowed to define the low-level key and wrap it.
                continue
            for pattern, message in SERVICE_BOUNDARY_PATTERNS:
                if pattern.search(line):
                    findings.append(Finding("ERROR" if strict else "WARN", "service-boundary", rp, idx, message, line))
            if ENTITY_ID_IMPORT_RE.search(line):
                findings.append(Finding("ERROR" if strict else "WARN", "service-boundary", rp, idx, "EntityId import in API/contract crate; use EntityHandle or domain key", line))
    return findings


def scan_large_files(strict: bool, fail_tracked: bool) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    ledger = load_large_debt_ledger()
    tracked_count = 0
    for suffix in ("*.rs", "*.py"):
        for path in REPO_ROOT.rglob(suffix):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            rp = rel(path)
            if any(rp.as_posix().endswith(s) for s in LARGE_FILE_ALLOW_SUFFIXES):
                continue
            try:
                loc = sum(1 for _ in path.open(encoding="utf-8", errors="replace"))
            except OSError:
                continue
            if loc > LARGE_FILE_LIMIT:
                key = rp.as_posix()
                if key in ledger:
                    tracked_count += 1
                    if fail_tracked:
                        findings.append(Finding("ERROR", "large-module", rp, 0, f"{loc} LOC > {LARGE_FILE_LIMIT}; tracked debt owner_wave={ledger[key].get('owner_wave', '<unknown>')}"))
                    continue
                sev = "ERROR" if strict else "WARN"
                findings.append(Finding(sev, "large-module", rp, 0, f"{loc} LOC > {LARGE_FILE_LIMIT}; untracked split debt; add ownership split or explicit ledger entry"))
    return findings, tracked_count


def run_checks(strict_large_files: bool, strict_boundaries: bool, fail_tracked_large_debt: bool) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    findings.extend(scan_public_neytd())
    findings.extend(scan_direct_provider_ids())
    findings.extend(scan_hidden_fallbacks())
    findings.extend(scan_service_boundaries(strict_boundaries))
    large_findings, tracked_large_debt = scan_large_files(strict_large_files, fail_tracked_large_debt)
    findings.extend(large_findings)
    return findings, tracked_large_debt


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p0_invariant_scan.py")
    parser.add_argument("--strict-large-files", action="store_true", help="Treat >550 LOC first-party files as blocking errors. Default: report as WARN while split work is staged.")
    parser.add_argument("--strict-boundaries", action="store_true", help="Treat service-boundary candidate leaks as blocking errors. Default: report non-provider-facing ECS component/extraction APIs as WARN.")
    parser.add_argument("--fail-tracked-large-debt", action="store_true", help="Make every ledger-tracked >550 LOC module fatal. Default: fail only new/untracked large modules.")
    parser.add_argument("--summary-only", action="store_true")
    ns = parser.parse_args(argv)

    findings, tracked_large_debt = run_checks(
        strict_large_files=bool(ns.strict_large_files),
        strict_boundaries=bool(ns.strict_boundaries),
        fail_tracked_large_debt=bool(ns.fail_tracked_large_debt),
    )
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p0 invariant scan: errors={len(errors)} warnings={len(warnings)} tracked_large_debt={tracked_large_debt} strict_large_files={bool(ns.strict_large_files)} strict_boundaries={bool(ns.strict_boundaries)} fail_tracked_large_debt={bool(ns.fail_tracked_large_debt)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
