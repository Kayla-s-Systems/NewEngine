#!/usr/bin/env python3
"""Shared source-scope policy for North Star architecture gates.

Architecture gates must inspect authoritative source/configuration, not generated output,
cache trees, scratch workspaces, diagnostic repository copies, or migration backups.
Keep traversal exclusions centralized here so every broad scanner sees the same topology.
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import Iterable

# Names are matched case-insensitively because the primary development host is Windows.
# Temp and Intermediate are explicitly non-authoritative workspaces: smoke/diagnostic tools
# may place complete repository copies there and those copies must never become architecture
# evidence.
ARCHITECTURE_EXCLUDED_DIR_NAMES = frozenset(
    name.casefold()
    for name in {
        ".git",
        ".takesome",
        ".northstar",
        ".springsuite",
        "target",
        "node_modules",
        "third_party",
        "__pycache__",
        "logs",
        "cache",
        "dist",
        "out",
        "bin",
        "obj",
        "artifacts",
        "Intermediate",
        "Temp",
    }
)


def is_architecture_excluded_dir_name(name: str) -> bool:
    return name.casefold() in ARCHITECTURE_EXCLUDED_DIR_NAMES


def is_architecture_source_path(path: Path, *, root: Path | None = None) -> bool:
    """Return True only when *path* is inside authoritative architecture source scope.

    When root is supplied, only path components below that root participate in the policy;
    this avoids an unrelated parent directory named e.g. ``Temp`` changing scan semantics.
    """
    candidate = path
    if root is not None:
        try:
            candidate = path.relative_to(root)
        except ValueError:
            return False
    return not any(is_architecture_excluded_dir_name(part) for part in candidate.parts[:-1])


def iter_architecture_files(root: Path, suffixes: set[str] | None = None) -> Iterable[Path]:
    # suffixes=None scans every file; otherwise matching is case-insensitive.
    normalized_suffixes = (
        {suffix.casefold() for suffix in suffixes} if suffixes is not None else None
    )
    for dirpath, dirnames, filenames in os.walk(root, topdown=True):
        dirnames[:] = [
            name for name in dirnames if not is_architecture_excluded_dir_name(name)
        ]
        base = Path(dirpath)
        for name in filenames:
            path = base / name
            if normalized_suffixes is None or path.suffix.casefold() in normalized_suffixes:
                yield path


def assert_policy_contract() -> None:
    """Fail fast if diagnostic/generated roots accidentally become source again."""
    for required in ("Temp", "Intermediate", "target", "cache"):
        if not is_architecture_excluded_dir_name(required):
            raise RuntimeError(f"architecture scan policy lost required exclusion: {required}")
