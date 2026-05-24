#!/usr/bin/env python3
"""Deny high-risk compatibility debt in runtime source.

This scan intentionally covers only enforceable P0 rules that the current source
must keep at zero. Larger API renames are handled by their own cleanup pass.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SKIP_DIRS = {"target", ".git", "docs", "archive", "research"}
SKIP_SUFFIXES = {".md", ".txt", ".log"}
DENY = [
    (re.compile(r"#\[deprecated"), "deprecated attribute is forbidden"),
    (re.compile(r"#\[allow\(deprecated\)\]"), "allow(deprecated) is forbidden"),
    (re.compile(r"deprecated compatibility adapter", re.IGNORECASE), "deprecated compatibility adapter is forbidden"),
    (re.compile(r"\.neytd@"), ".neytd authored/runtime selector is forbidden"),
    (re.compile(r"asset\.codec\.pak"), "public .pak codec alias is forbidden"),
    (re.compile(r"newengine\.container\.pak"), "public .pak container alias is forbidden"),
    (re.compile(r"type\s*=\s*\"pak\""), "public VFS/package type='pak' alias is forbidden"),
]


def iter_source_files() -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    for path in ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.name == "no_legacy_scan.py":
            continue
        if path.suffix in SKIP_SUFFIXES:
            continue
        if path.suffix not in {".rs", ".toml", ".json", ".yml", ".yaml", ".py", ".cmd", ".ps1"}:
            continue
        out.append(path)
    return out


def main() -> int:
    violations: list[str] = []
    for path in iter_source_files():
        rel = path.relative_to(ROOT)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line_no, line in enumerate(lines, start=1):
            for pattern, message in DENY:
                if pattern.search(line):
                    violations.append(f"{rel}:{line_no}: {message}: {line.strip()}")

    if violations:
        print("no-legacy scan failed:")
        for item in violations:
            print(f"  {item}")
        return 1

    print("no-legacy scan passed: enforceable deprecated/package alias rules are clean.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
