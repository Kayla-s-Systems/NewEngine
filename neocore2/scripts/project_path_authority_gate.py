#!/usr/bin/env python3
"""Reject machine-specific absolute paths from engine/project configuration and source.

Project-relative authored paths, logical VFS refs, environment-derived paths and
platform installation fallbacks are valid. Paths tied to a developer home directory
are not: those must come from ProjectRuntimeContext, startup storage roots, an
explicit manifest field, or an environment/config boundary.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCAN_ROOTS = (
    ROOT / "apps",
    ROOT / "crates",
    ROOT / "config",
    ROOT / "scripts",
    ROOT / "config.json",
    ROOT / "runtime.toml",
)
SKIP_DIRS = {
    ".git",
    ".idea",
    ".springsuite",
    "cache",
    "target",
    "Temp",
    "third_party",
    "testdata",
    "fixtures",
}
TEXT_SUFFIXES = {".rs", ".toml", ".json", ".py", ".cmd", ".bat", ".md"}

# Intentionally narrow: platform roots such as C:\\Program Files are installation
# conventions, while developer-home paths make a checkout non-relocatable.
MACHINE_PATH = re.compile(r"(?i)(?:[a-z]:[\\/]+users[\\/]|/users/|/home/)")


def iter_files(root: Path):
    if root.is_file():
        yield root
        return
    if not root.exists():
        return
    for path in root.rglob("*"):
        if not path.is_file() or path.suffix.lower() not in TEXT_SUFFIXES:
            continue
        if any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts):
            continue
        yield path


def main() -> int:
    violations: list[tuple[Path, int, str]] = []
    seen: set[Path] = set()
    for root in SCAN_ROOTS:
        for path in iter_files(root):
            if path == Path(__file__).resolve():
                continue
            if path in seen:
                continue
            seen.add(path)
            try:
                lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
            except OSError as error:
                print(f"project-path-authority: cannot read {path}: {error}", file=sys.stderr)
                return 2
            for line_number, line in enumerate(lines, 1):
                if MACHINE_PATH.search(line):
                    violations.append((path, line_number, line.strip()))

    if violations:
        print("project-path-authority: FAIL")
        for path, line_number, line in violations:
            relative = path.relative_to(ROOT)
            print(f"  {relative}:{line_number}: {line}")
        print(
            "Machine-specific paths must be resolved from the selected project, "
            "startup storage roots, manifest/config, or environment authority."
        )
        return 1

    print(f"project-path-authority: PASS ({len(seen)} files scanned)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
