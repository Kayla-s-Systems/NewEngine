#!/usr/bin/env python3
"""Deny runtime work that bypasses engine.jobs identity.

The playable runtime may only create long-running work through engine.jobs.
Standalone tools/importers are intentionally skipped because they do not live in
runtime. Platform-bootstrap exceptions must carry an explicit marker and should
emit a console warning when stopped/joined.
"""
from __future__ import annotations

import pathlib
import re
import sys
from dataclasses import dataclass

SCRIPT = pathlib.Path(__file__).resolve()
NEOCORE_ROOT = SCRIPT.parents[1]
REPO_ROOT = SCRIPT.parents[3] if (SCRIPT.parents[3] / "Plugins").exists() else NEOCORE_ROOT

THREAD_PATTERNS = [
    re.compile(r"\bstd::thread::spawn\s*\("),
    re.compile(r"\bthread::spawn\s*\("),
    re.compile(r"\bstd::thread::Builder::new\s*\("),
    re.compile(r"\bthread::Builder::new\s*\("),
    re.compile(r"\brayon::scope\s*\("),
    re.compile(r"\brayon::join\s*\("),
    re.compile(r"\btokio::spawn\s*\("),
    re.compile(r"\basync_std::task::spawn\s*\("),
]
PROCESS_PATTERNS = [
    re.compile(r"\bCommand::new\s*\("),
]
QUEUE_PATTERNS = [
    re.compile(r"\bmpsc::channel\s*\("),
    re.compile(r"\bmpsc::sync_channel\s*\("),
    re.compile(r"\bcrossbeam_channel::unbounded\s*\("),
    re.compile(r"\bcrossbeam_channel::bounded\s*\("),
]

ALLOW_MARKER = "no-hidden-thread-scan:"
SKIP_DIRS = {
    ".git",
    "target",
    "docs",
    "tools",
    "Importers",
    "third_party",
    "build",
}
SKIP_FILES = {
    "Cargo.lock",
}

# Standalone plugin build workspaces contain their own Cargo.lock files; scan the
# source, not lockfile dependency metadata.
SKIP_SUFFIXES = {
    ".md",
    ".toml",
    ".json",
    ".lock",
}

@dataclass(frozen=True)
class Finding:
    severity: str
    kind: str
    rel: pathlib.Path
    line_no: int
    line: str


def iter_source_files() -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    roots = [REPO_ROOT / "NewEngine" / "neocore2", REPO_ROOT / "Plugins"]
    for root in roots:
        if not root.exists():
            continue
        for path in root.rglob("*.rs"):
            if path.name in SKIP_FILES:
                continue
            if any(part in SKIP_DIRS for part in path.relative_to(REPO_ROOT).parts):
                continue
            if path.suffix in SKIP_SUFFIXES:
                continue
            out.append(path)
    return out


def has_allow_marker(lines: list[str], index: int) -> bool:
    lo = max(0, index - 5)
    hi = min(len(lines), index + 4)
    return any(ALLOW_MARKER in lines[i] for i in range(lo, hi))


def classify(path: pathlib.Path, line: str, lines: list[str], index: int) -> Finding | None:
    rel = path.relative_to(REPO_ROOT)
    stripped = line.strip()

    if any(pattern.search(line) for pattern in THREAD_PATTERNS):
        if has_allow_marker(lines, index):
            return None
        return Finding("ERROR", "hidden-thread", rel, index + 1, stripped)

    if any(pattern.search(line) for pattern in PROCESS_PATTERNS):
        if has_allow_marker(lines, index):
            return None
        return Finding("WARN", "hidden-process", rel, index + 1, stripped)

    if any(pattern.search(line) for pattern in QUEUE_PATTERNS):
        # Queues are allowed for engine-owned event/job buses and local bounded
        # bootstrap handoff only when annotated. Unannotated runtime queues can
        # become dark work pipelines, so they are warnings now and can be
        # promoted to errors once all local queues are audited.
        if has_allow_marker(lines, index):
            return None
        if "newengine-core/src/events.rs" in str(rel) or "newengine-core/src/bus.rs" in str(rel):
            return None
        return Finding("WARN", "local-queue", rel, index + 1, stripped)

    return None


def main() -> int:
    findings: list[Finding] = []
    for path in iter_source_files():
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(lines):
            finding = classify(path, line, lines, i)
            if finding:
                findings.append(finding)

    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]

    if findings:
        print("no-hidden-thread scan findings:")
        for item in findings:
            print(f"  [{item.severity}] {item.kind}: {item.rel}:{item.line_no}: {item.line}")

    if errors:
        print("\nno-hidden-thread scan failed:")
        print("  Runtime/plugin threads must go through engine.jobs or a documented platform-bootstrap exception. Hidden processes are reported as warnings until tool-runner migration.")
        print("  Standalone tools/importers are skipped by this scanner.")
        return 1

    if warnings:
        print("\nno-hidden-thread scan passed with warnings: local queues remain audited but not fatal.")
        return 0

    print("no-hidden-thread scan passed: runtime executors are engine.jobs-visible or explicitly annotated bootstrap exceptions.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
