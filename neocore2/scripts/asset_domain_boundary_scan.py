#!/usr/bin/env python3
"""Enforce asset/world data ownership boundaries.

P2 rule:
  AssetManager owns bytes, VFS and package/source mounts.
  Domain gateways own semantics.
  Renderer receives render-ready packets and must not parse .ydd/.nemat/.ytd/.ytyp.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]

RENDER_ROOTS = [
    ROOT.parent.parent / "Plugins" / "VulkanRenderer",
    ROOT / "crates" / "newengine-engine-runtime" / "src" / "render_controller",
]

# AssetManager may call codec/container helpers in explicit package writer code,
# but it must not become the owner of model/material/render domain semantics.
ASSET_MANAGER_ROOT = ROOT.parent.parent / "Plugins" / "AssetManager" / "newengine-AssetManager" / "src"

SKIP_SUFFIXES = {".md", ".txt", ".log", ".png", ".jpg", ".jpeg", ".ico", ".ytd", ".nepak"}
RUNTIME_FORMAT_LITERALS = re.compile(r'"[^"]*\.(?:ydd|ytd|nemat|ytyp)(?:@|"|/)')
RENDER_PARSE_WORDS = re.compile(r"parse_(?:ytd|ydd|nemat|ytyp)|TextureDictionary::parse|DrawableDictionary|MaterialLibrary")
ASSET_MANAGER_SEMANTIC_IMPORTS = re.compile(r"use\s+newengine_(?:model|material|render|scene)[A-Za-z0-9_]*(?:::|\s|;)")
NEPAK_AS_LISTFILE = re.compile(r"LIST_FILE_CONTENT_KIND_NEPAK|content_kind\s*=\s*\"nepak\"|\.nepak@")

ALLOW_RENDER_STRINGS = (
    "runtime_rule",
    "diagnostic",
)
ALLOW_ASSET_MANAGER_PATH_PARTS = {
    "listfile_writer.rs",  # explicit package-writer byte mutation path; not semantic resolver.
}


def iter_files(root: pathlib.Path):
    if not root.exists():
        return
    for path in root.rglob("*"):
        if path.is_file() and path.suffix not in SKIP_SUFFIXES:
            yield path


def line_allowed(line: str, allowed_terms: tuple[str, ...]) -> bool:
    return any(term in line for term in allowed_terms)


def main() -> int:
    violations: list[str] = []

    for root in RENDER_ROOTS:
        for path in iter_files(root):
            rel = path.relative_to(ROOT.parent.parent)
            for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                if line.strip().startswith("//"):
                    continue
                if RUNTIME_FORMAT_LITERALS.search(line) and not line_allowed(line, ALLOW_RENDER_STRINGS):
                    violations.append(f"{rel}:{idx}: renderer/runtime render code must not branch on runtime asset file formats: {line.strip()}")
                if RENDER_PARSE_WORDS.search(line):
                    violations.append(f"{rel}:{idx}: renderer must consume packets, not parse asset domain payloads: {line.strip()}")

    for path in iter_files(ASSET_MANAGER_ROOT):
        rel = path.relative_to(ROOT.parent.parent)
        allow_file = path.name in ALLOW_ASSET_MANAGER_PATH_PARTS
        for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if line.strip().startswith("//"):
                continue
            if ASSET_MANAGER_SEMANTIC_IMPORTS.search(line) and not allow_file:
                violations.append(f"{rel}:{idx}: AssetManager must not import semantic domain/runtime crates: {line.strip()}")
            if NEPAK_AS_LISTFILE.search(line):
                violations.append(f"{rel}:{idx}: .nepak is a mounted VFS package, not a ListFile entry/selector: {line.strip()}")

    if violations:
        print("asset-domain-boundary scan failed:")
        for item in violations:
            print(f"  {item}")
        return 1
    print("asset-domain-boundary scan passed: bytes/semantics/render packet boundaries are clean.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
