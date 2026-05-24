#!/usr/bin/env python3
"""Deny high-risk compatibility debt in runtime source.

This is intentionally strict for public/runtime code and intentionally narrow for
terms that can appear legitimately in math/shader/debug code. Migration notes and
third-party sources are excluded; engine code must not grow a second public API by
keeping old adapters alive.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SKIP_DIRS = {"target", ".git", "docs", "archive", "research", "third_party", "assets"}
SKIP_SUFFIXES = {".md", ".txt", ".log"}
SOURCE_SUFFIXES = {".rs", ".toml", ".json", ".yml", ".yaml", ".py", ".cmd", ".ps1"}
ALLOW_ENV_PREFIXES = (
    pathlib.Path("build-support"),
    pathlib.Path("scripts"),
    pathlib.Path("crates/newengine-core/src/startup"),
    pathlib.Path("crates/newengine-core/src/crash.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/platform_runtime/config.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/platform_runtime/discovery.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/platform_runtime/early_log.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/platform_runtime/shutdown_watchdog.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/app_launcher.rs"),
    pathlib.Path("crates/newengine-runtime-host/src/asset_bootstrap.rs"),
    pathlib.Path("crates/newengine-plugin-host/src/paths.rs"),
    pathlib.Path("crates/newengine-plugin-host/src/manager/lifecycle.rs"),
    pathlib.Path("crates/newengine-plugin-host/src/manager/discovery"),
    pathlib.Path("crates/newengine-engine-runtime/src/env_config.rs"),
    pathlib.Path("crates/newengine-core/src/storage_root.rs"),
    pathlib.Path("crates/newengine-core/src/engine/plugins.rs"),
    pathlib.Path("crates/newengine-core/src/startup_window/args.rs"),
)
DENY = [
    (re.compile(r"#\[deprecated"), "deprecated attribute is forbidden"),
    (re.compile(r"#\[allow\(deprecated\)\]"), "allow(deprecated) is forbidden"),
    (re.compile(r"deprecated compatibility adapter", re.IGNORECASE), "deprecated compatibility adapter is forbidden"),
    (re.compile(r"\.neytd@"), ".neytd authored/runtime selector is forbidden"),
    (re.compile(r"newengine-codec-neytd"), "retired .neytd codec worker is forbidden"),
    (re.compile(r"NETD/top-level-compat"), "NETD top-level compatibility input is forbidden"),
    (re.compile(r"top-level-compat"), "top-level compatibility input is forbidden"),
    (re.compile(r"load_loading_background_from_neytd"), "Aurelia loading texture must load canonical .ytd, not .neytd vocabulary"),
    (re.compile(r"asset\.codec\.pak"), "public .pak codec alias is forbidden"),
    (re.compile(r"newengine\.container\.pak"), "public .pak container alias is forbidden"),
    (re.compile(r"type\s*=\s*\"pak\""), "public VFS/package type='pak' alias is forbidden"),
    (re.compile(r"SCRIPTING_SERVICE_METHOD_(FRAME|LOAD_MODULE|MODULE_MANIFEST|DISPATCH_EVENT)_JSON_V1"), "scripting JSON compatibility method is forbidden"),
    (re.compile(r"\bScript(FrameInput|FrameOutput|ModuleDescriptor|ModuleManifest|ModuleLoadRequest|ModuleLoadResponse|DispatchEventRequest)\b"), "scripting JSON compatibility DTO is forbidden"),
    (re.compile(r"uses deprecated service_id"), "provider route metadata must reject service_id instead of accepting it for migration"),
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
        if path.suffix not in SOURCE_SUFFIXES:
            continue
        out.append(path)
    return out


def is_allowed_env_var_read(rel: pathlib.Path) -> bool:
    return any(rel == prefix or rel.is_relative_to(prefix) for prefix in ALLOW_ENV_PREFIXES)


def main() -> int:
    violations: list[str] = []
    for path in iter_source_files():
        rel = path.relative_to(ROOT)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line_no, line in enumerate(lines, start=1):
            for pattern, message in DENY:
                if pattern.search(line):
                    violations.append(f"{rel}:{line_no}: {message}: {line.strip()}")
            if ("std::env::var(" in line or "std::env::var_os(" in line) and not is_allowed_env_var_read(rel):
                violations.append(f"{rel}:{line_no}: std::env reads are allowed only in bootstrap/config/build/tool-launcher layers: {line.strip()}")

    if violations:
        print("no-legacy scan failed:")
        for item in violations:
            print(f"  {item}")
        return 1

    print("no-legacy scan passed: deprecated adapters, runtime aliases and hidden env reads are clean.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
