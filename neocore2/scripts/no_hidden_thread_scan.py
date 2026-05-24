#!/usr/bin/env python3
"""Deny runtime work that bypasses engine.jobs identity.

Allowed executors must either be the engine.jobs worker implementation or carry a
nearby `no-hidden-thread-scan:` annotation explaining the engine.jobs-compatible
telemetry bridge that makes the work visible.
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
DENY = [
    re.compile(r"\bstd::thread::spawn\s*\("),
    re.compile(r"\bthread::spawn\s*\("),
    re.compile(r"\bstd::thread::Builder::new\s*\("),
    re.compile(r"\bthread::Builder::new\s*\("),
    re.compile(r"\brayon::scope\s*\("),
    re.compile(r"\brayon::join\s*\("),
    re.compile(r"\btokio::spawn\s*\("),
]
ALLOW_MARKER = "no-hidden-thread-scan:"
SKIP_DIRS = {"target", ".git"}


def iter_rs_files() -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    for path in ROOT.rglob("*.rs"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        out.append(path)
    return out


def has_allow_marker(lines: list[str], index: int) -> bool:
    lo = max(0, index - 4)
    hi = min(len(lines), index + 3)
    return any(ALLOW_MARKER in lines[i] for i in range(lo, hi))


def main() -> int:
    violations: list[str] = []
    for path in iter_rs_files():
        rel = path.relative_to(ROOT)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for i, line in enumerate(lines):
            if not any(pattern.search(line) for pattern in DENY):
                continue
            if has_allow_marker(lines, i):
                continue
            violations.append(f"{rel}:{i + 1}: hidden executor without engine.jobs telemetry marker: {line.strip()}")

    if violations:
        print("no-hidden-thread scan failed:")
        for item in violations:
            print(f"  {item}")
        print("\nRoute long-running work through JobSystem/ToolJobRunner or add explicit engine.jobs-compatible telemetry before the executor.")
        return 1

    print("no-hidden-thread scan passed: all direct executors are engine.jobs-visible or explicitly annotated.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
