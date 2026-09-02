#!/usr/bin/env python3
"""Fail when first-party NewEngine paths regress to parent-directory traversal.

Filesystem authority is explicit:
- ROOT-DIR: NorthStar repository/deployment root.
- PROJECT-DIR: active project root after game.toml selection.

Cargo app dependencies use [workspace.dependencies] because Cargo does not expand
runtime environment variables in dependency path declarations.
"""
from __future__ import annotations

from pathlib import Path
import json
import sys

from architecture_scan_policy import assert_policy_contract, iter_architecture_files

WORKSPACE = Path(__file__).resolve().parents[1]
ENGINE = WORKSPACE.parent
FORWARD_PARENT_PAIR = ".." + "/" + ".."
BACKWARD_PARENT_PAIR = ".." + "\\" + ".."

errors: list[str] = []
scanned = 0

assert_policy_contract()
for path in sorted(iter_architecture_files(ENGINE)):
    relative = path.relative_to(ENGINE)
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        continue
    scanned += 1
    if FORWARD_PARENT_PAIR in text or BACKWARD_PARENT_PAIR in text:
        errors.append(f"parent traversal is forbidden: {relative.as_posix()}")

project_api = WORKSPACE / "crates" / "newengine-project-api" / "src" / "lib.rs"
project_api_text = project_api.read_text(encoding="utf-8")
for required in (
    'pub const ROOT_DIR_ENV: &str = "ROOT-DIR";',
    'pub const PROJECT_DIR_ENV: &str = "PROJECT-DIR";',
):
    if required not in project_api_text:
        errors.append(f"path authority constant missing: {required}")

runtime_host = WORKSPACE / "crates" / "newengine-runtime-host" / "src" / "app_launcher" / "bootstrap.rs"
runtime_authority = runtime_host.parent / "bootstrap" / "authority.rs"
runtime_environment = runtime_host.parent / "bootstrap" / "environment.rs"
runtime_text = "\n".join(
    path.read_text(encoding="utf-8")
    for path in (runtime_host, runtime_authority, runtime_environment)
)
for required in (
    "install_root_dir_authority(&host_context)?",
    "publish_expanded_storage_roots(host, &startup);",
    "PROJECT_DIR_ENV",
    "Component::ParentDir",
):
    if required not in runtime_text:
        errors.append(f"runtime path authority invariant missing: {required}")

launcher = WORKSPACE / "apps" / "NewEngine" / "src" / "main.rs"
launcher_text = launcher.read_text(encoding="utf-8")
for required in (
    "install_process_root_dir_authority(&runtime_config_path)?",
    "env::set_var(PROJECT_DIR_ENV, &project.project_root);",
):
    if required not in launcher_text:
        errors.append(f"launcher path authority invariant missing: {required}")

for config_name in ("config.json", "config.p8-fine.json"):
    config_path = WORKSPACE / config_name
    config = json.loads(config_path.read_text(encoding="utf-8"))
    engine_cfg = config.get("engine", {})
    for key in ("modules_dir", "cache_files", "config"):
        value = str(engine_cfg.get(key, ""))
        if not value.startswith("ROOT-DIR/"):
            errors.append(f"{config_name}: engine.{key} must start with ROOT-DIR/, got {value!r}")
    layers = config.get("plugins", {}).get("engine.assets.starvault", {}).get("layers", [])
    for index, layer in enumerate(layers):
        for key in ("path", "dir"):
            value = layer.get(key) if isinstance(layer, dict) else None
            if isinstance(value, str) and value and ("Shared/Content" in value) and not value.startswith("ROOT-DIR/"):
                errors.append(
                    f"{config_name}: StarVault layers[{index}].{key} shared path must use ROOT-DIR, got {value!r}"
                )

for app in ("NewEngine", "AssetInspector", "AureliaUiTest", "RendererDemo"):
    manifest = (WORKSPACE / "apps" / app / "Cargo.toml").read_text(encoding="utf-8")
    if "path = \"" in manifest and (FORWARD_PARENT_PAIR in manifest or BACKWARD_PARENT_PAIR in manifest):
        errors.append(f"apps/{app}/Cargo.toml contains parent-traversing dependency path")

if errors:
    print("[path-authority] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print(
    f"[path-authority] PASS scanned={scanned} root=ROOT-DIR project=PROJECT-DIR parent_traversal=0"
)
