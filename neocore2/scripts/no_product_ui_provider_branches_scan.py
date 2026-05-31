#!/usr/bin/env python3
"""Deny product-specific UI branches inside generic UI providers.

AureliaUI is an engine.ui provider. It may know framework component roles
(button, panel, scroll_bar, tree, viewport), but it must not grow hardcoded
Asset Browser / EditorScreen / GameScreen rendering or hit-test paths.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parent.parent
PROVIDER_ROOTS = [REPO_ROOT / "Plugins" / "AureliaUI" / "newengine-ui-provider-aurelia" / "src"]
SOURCE_SUFFIXES = {".rs"}
SKIP_DIRS = {"target", ".git"}

# Product-owned ids are allowed in app/runtime crates, not in provider framework code.
DENY = [
    (re.compile(r"asset[_-]browser", re.IGNORECASE), "Asset Browser must remain app/UI composition, not provider branch"),
    (re.compile(r"content[_ -]?browser", re.IGNORECASE), "Content Browser must remain app/UI composition, not provider branch"),
    (re.compile(r"app\.asset_browser"), "product owner id is forbidden in engine.ui provider"),
    (re.compile(r"bottom\.content_browser"), "editor dock panel id is forbidden in engine.ui provider"),
    (re.compile(r"EditorScreen|GameScreen|ScreenProfile"), "screen profile products are forbidden in engine.ui provider"),
    (re.compile(r"editor\s+shell", re.IGNORECASE), "editor shell behavior is forbidden in engine.ui provider"),
]

# These words can appear in docs/manifests/text capability metadata without
# adding provider branches. The scan intentionally targets product identifiers
# and screen-specific code, not generic terms such as `editor.yft` font assets.
ALLOW_FILES = {
    pathlib.Path("manifests.rs"),
}


def iter_provider_files() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for root in PROVIDER_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if not path.is_file():
                continue
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            if path.suffix not in SOURCE_SUFFIXES:
                continue
            rel = path.relative_to(root)
            if rel in ALLOW_FILES:
                continue
            files.append(path)
    return files


def main() -> int:
    violations: list[str] = []
    for path in iter_provider_files():
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        for line_no, line in enumerate(lines, start=1):
            for pattern, message in DENY:
                if pattern.search(line):
                    violations.append(f"{path.relative_to(REPO_ROOT)}:{line_no}: {message}: {line.strip()}")

    if violations:
        print("no-product-ui-provider-branches scan failed:")
        for violation in violations:
            print(f"  {violation}")
        print("\nMove product UI behavior to app/UI composition nodes and action handlers; provider code must stay generic.")
        return 1

    print("no-product-ui-provider-branches scan passed: engine.ui provider contains no product-specific UI branches.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
