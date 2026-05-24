#!/usr/bin/env python3
"""Deny provider service ids in semantic route metadata."""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGETS = [
    ROOT / "crates" / "newengine-model-domain-api" / "src" / "asset_graph.rs",
    ROOT / "crates" / "newengine-model-domain-api" / "src" / "asset_graph",
    ROOT / "crates" / "newengine-assets-api" / "src" / "list_file.rs",
]
PROVIDER_IDS = [
    "model.api",
    "physics.api",
    "materials.api",
    "ai.api",
    "asset_manager.api",
    "render.api",
]
DENY_FIELD_NAMES = [
    re.compile(r"\bhandler_service\b.*AssetGraphNode"),
    re.compile(r"\bservice\s*:\s*\"(?:" + "|".join(re.escape(x) for x in PROVIDER_IDS) + r")\""),
]


def iter_files() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for target in TARGETS:
        if target.is_file():
            files.append(target)
        elif target.is_dir():
            files.extend(sorted(target.rglob("*.rs")))
    return files


def main() -> int:
    violations: list[str] = []
    for path in iter_files():
        rel = path.relative_to(ROOT)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line_no, line in enumerate(lines, start=1):
            for provider_id in PROVIDER_IDS:
                if f'"{provider_id}"' in line:
                    violations.append(
                        f"{rel}:{line_no}: provider service id `{provider_id}` appears in semantic route metadata"
                    )
            for pattern in DENY_FIELD_NAMES:
                if pattern.search(line):
                    violations.append(f"{rel}:{line_no}: gateway metadata shape violation: {line.strip()}")

    if violations:
        print("gateway-contract scan failed:")
        for item in violations:
            print(f"  {item}")
        print("\nSemantic route metadata must be gateway + method + semantic_owner, not provider service id.")
        return 1

    print("gateway-contract scan passed: semantic route metadata does not expose provider service ids.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
