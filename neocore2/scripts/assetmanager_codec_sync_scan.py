#!/usr/bin/env python3
"""Validate that building AssetManager also syncs its private codec workers.

AssetManager is only the VFS/bytes/dispatch host. NEF8/ListFile and NEPAK
identity live in provider-owned codec workers. A selected AssetManager build that
installs only engine.assets.starvault-*.dll but leaves plugins/codecs stale can boot with an
empty codec registry and then fail strict data-driven startup at .ymap/.ytyp
resolution time.
"""
from __future__ import annotations

import json
import pathlib
import re

SCRIPT = pathlib.Path(__file__).resolve()
REPO_ROOT = SCRIPT.parents[3]
MANIFEST = REPO_ROOT / "Plugins" / "build_manifest.json"
BUILD_CMD = REPO_ROOT / "Plugins" / "build_all_plugins.cmd"
BUILD_SCRIPT = REPO_ROOT / "tools" / "scripts" / "takesome" / "plugin_build" / "main.py"
REQUIRED_CODEC_WORKERS = {"newengine-codec-listfile", "newengine-codec-nepak"}


def fail(message: str) -> int:
    print(f"assetmanager-codec-sync scan failed: {message}")
    return 1


def main() -> int:
    if not MANIFEST.is_file():
        return fail(f"missing {MANIFEST}")
    if not BUILD_CMD.is_file():
        return fail(f"missing {BUILD_CMD}")
    if not BUILD_SCRIPT.is_file():
        return fail(f"missing {BUILD_SCRIPT}")

    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    plugins = {str(item) for item in manifest.get("plugins", [])}
    workers = {str(item) for item in manifest.get("codecWorkers", [])}

    # The plugin list is discovery-driven. AssetManager no longer has to be
    # hardcoded in build_manifest.json, but it must remain discoverable from the
    # standard plugin workspace layout.
    asset_manager_workspace = REPO_ROOT / "Plugins" / "AssetManager" / "newengine-AssetManager" / "Cargo.toml"
    if "AssetManager" not in plugins and not asset_manager_workspace.is_file():
        return fail("AssetManager must be discoverable as Plugins/AssetManager/newengine-AssetManager/Cargo.toml or explicitly listed")

    missing_workers = sorted(REQUIRED_CODEC_WORKERS - workers)
    if missing_workers:
        return fail(f"Plugins/build_manifest.json missing codecWorkers: {missing_workers}")

    text = BUILD_CMD.read_text(encoding="utf-8", errors="replace") + "\n" + BUILD_SCRIPT.read_text(encoding="utf-8", errors="replace")
    required_fragments = [
        r"AssetManager selected: syncing required codec workers",
        r"name\.lower\(\) == \"assetmanager\"",
        r"manifest\(root\)\.get\(\"codecWorkers\"",
        r"plugin_out / \"codecs\"",
    ]
    for fragment in required_fragments:
        if not re.search(fragment, text, flags=re.IGNORECASE):
            return fail(f"Plugins/build_all_plugins.cmd missing required fragment: {fragment}")

    print("assetmanager-codec-sync scan passed: selected AssetManager builds also sync codec workers.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
