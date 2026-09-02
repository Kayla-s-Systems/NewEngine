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
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
SKIP_DIRS = {"target", ".git", "docs", "archive", "research", "third_party", "assets", "cache"}
SKIP_SUFFIXES = {".md", ".txt", ".log"}
SOURCE_SUFFIXES = {".rs", ".toml", ".json", ".yml", ".yaml", ".py", ".cmd", ".ps1"}
DENY = [
    (re.compile(r"#\[deprecated"), "deprecated attribute is forbidden"),
    (re.compile(r"#\[allow\(deprecated\)\]"), "allow(deprecated) is forbidden"),
    (re.compile(r"deprecated compatibility adapter", re.IGNORECASE), "deprecated compatibility adapter is forbidden"),
    (re.compile(r"\bPluginModuleV2\b"), "PluginModuleV2 is forbidden; use canonical PluginModule"),
    (re.compile(r"\bPluginModuleV3\b"), "PluginModuleV3 is forbidden; use canonical PluginModule"),
    (re.compile(r"\bcreate_module_v2\b"), "create_module_v2 is forbidden; export create_module only"),
    (re.compile(r"\bcreate_module_v3\b"), "create_module_v3 is forbidden; export create_module only"),
    (re.compile(r"\bcreate_v2\b"), "create_v2 root callback is forbidden; use create"),
    (re.compile(r"\bcreate_v3\b"), "create_v3 root callback is forbidden; use create"),
    (re.compile(r"\.neytd@"), ".neytd authored/runtime selector is forbidden"),
    (re.compile(r"newengine-codec-neytd"), "retired .neytd codec worker is forbidden"),
    (re.compile(r"NETD/top-level-compat"), "NETD top-level compatibility input is forbidden"),
    (re.compile(r"top-level-compat"), "top-level compatibility input is forbidden"),
    (re.compile(r"load_loading_background_from_neytd"), "Loading UI texture must load canonical .ytd, not .neytd vocabulary"),
    (re.compile(r"asset\.codec\.pak"), "public .pak codec alias is forbidden"),
    (re.compile(r"newengine\.container\.pak"), "public .pak container alias is forbidden"),
    (re.compile(r"type\s*=\s*\"pak\""), "public VFS/package type='pak' alias is forbidden"),
    (re.compile(r"SCRIPTING_SERVICE_METHOD_(FRAME|LOAD_MODULE|MODULE_MANIFEST|DISPATCH_EVENT)_JSON_V1"), "scripting JSON compatibility method is forbidden"),
    (re.compile(r"\bScript(FrameInput|FrameOutput|ModuleDescriptor|ModuleManifest|ModuleLoadRequest|ModuleLoadResponse|DispatchEventRequest)\b"), "scripting JSON compatibility DTO is forbidden"),
    (re.compile(r"uses deprecated service_id"), "provider route metadata must reject service_id instead of accepting it for migration"),

    # Retired provider route aliases. Concrete implementations must publish
    # route identity through descriptor metadata (`engine.render.vulkan` etc.)
    # and install artifacts through the canonical implementation label.
    (re.compile(r'"newengine\.renderer(?:\.|"|$)'), "retired renderer provider route alias is forbidden; use engine.render.<provider>"),
    (re.compile(r'"newengine\.physics(?:\.|"|$)'), "retired physics provider route alias is forbidden; use engine.physics.<provider>"),
    (re.compile(r'"newengine\.platform(?:\.|"|$)'), "retired platform provider route alias is forbidden; use engine.platform.<provider>"),
    (re.compile(r'"newengine\.logging(?:\.|"|$)'), "retired logging provider route alias is forbidden; use engine.logging.<provider>"),
    (re.compile(r'newengine\.assets_catalog_ui'), "Asset Browser is an app/UI composition, not a backend API/domain"),
    (re.compile(r'assetManager-[0-9].*\.dll'), "retired AssetManager DLL name is forbidden; use provider install name starVault-assetManager-{version}"),
    (re.compile(r'engine\.ui\.aurelia-[0-9].*\.dll'), "route id must not be used as DLL stem; use aurelia-ui-{version}"),
    (re.compile(r'engine\.render\.vulkan-[0-9].*\.dll'), "route id must not be used as DLL stem; use vulkan-renderer-{version}"),
    (re.compile(r'unwrap_or_else\s*\([^\n]*(?:Null|Fallback|fallback)'), "hidden fallback construction is forbidden; use profile policy or real NullProvider route"),
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



def is_versioned_api_constant_path(rel: pathlib.Path) -> bool:
    parts = rel.parts
    if len(parts) >= 2 and parts[0] == "crates":
        crate = parts[1]
        return crate.endswith("-api") or crate.endswith("-contracts")
    return False


def is_allowed_v2_line(line: str, rel: pathlib.Path) -> bool:
    stripped = line.strip()
    upper = stripped.upper()
    # Versioned schema/wire names are first-class vocabulary. Versioned method/symbol
    # constants are legal only at an owning API/contracts boundary, not in runtime code.
    if stripped.startswith(("pub const ", "const ", "pub static ", "static ")):
        return is_versioned_api_constant_path(rel)
    return (
        "WIRE_V2" in upper
        or "SCHEMA_V2" in upper
        or "VERSION_" in upper
        or "CONTENT_SCHEMA" in upper
        or "FORMAT_VERSION" in upper
    )


METHOD_V2_LITERAL_RE = re.compile(r'["\'][A-Za-z0-9_.:-]+_v2["\']', re.IGNORECASE)
AD_HOC_METHOD_CONTEXT_RE = re.compile(
    r"call_service|invoke|router|\.blob\s*\(|\.post_json\s*\(|method\s*[=:]",
    re.IGNORECASE,
)


def has_forbidden_method_v2(line: str, rel: pathlib.Path) -> bool:
    if "_v2" not in line.lower() or is_allowed_v2_line(line, rel):
        return False
    # Type/function/API symbol names such as PluginDescriptorV2, compile_v2 and
    # newengine_plugin_descriptor_v2 are legitimate versioned APIs. Only an inline
    # service-method string literal in a call/router context is migration debt; such
    # identities must come from an owning API/contract constant instead.
    return bool(METHOD_V2_LITERAL_RE.search(line) and AD_HOC_METHOD_CONTEXT_RE.search(line))


def iter_tracked_repository_paths() -> list[pathlib.Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise RuntimeError(f"git ls-files failed while scanning repository artifacts: {detail}")

    paths: list[pathlib.Path] = []
    for raw in result.stdout.split(b"\0"):
        if not raw:
            continue
        rel = pathlib.PurePosixPath(raw.decode("utf-8", errors="surrogateescape"))
        paths.append(ROOT.joinpath(*rel.parts))
    return paths


def iter_forbidden_repository_artifacts() -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    skip = {"target", ".git", "docs", "archive", "research", "third_party", "cache"}
    for path in iter_tracked_repository_paths():
        rel = path.relative_to(ROOT)
        if any(part in skip for part in rel.parts):
            continue
        if "__pycache__" in rel.parts:
            out.append(path)
            continue
        if path.suffix == ".pyc" or path.name.endswith(".bak") or ".bak-" in path.name:
            out.append(path)
    return out


def main() -> int:
    violations: list[str] = []
    for artifact in iter_forbidden_repository_artifacts():
        rel = artifact.relative_to(ROOT)
        violations.append(
            f"{rel}: repository backup/cache artifact is forbidden; use git history or ignored build cache"
        )

    for path in iter_source_files():
        rel = path.relative_to(ROOT)
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line_no, line in enumerate(lines, start=1):
            for pattern, message in DENY:
                if pattern.search(line):
                    violations.append(f"{rel}:{line_no}: {message}: {line.strip()}")
            if has_forbidden_method_v2(line, rel):
                violations.append(f"{rel}:{line_no}: _v2 service method identifiers are forbidden outside schema/wire constants: {line.strip()}")

    if violations:
        print("no-legacy scan failed:")
        for item in violations:
            print(f"  {item}")
        return 1

    print("no-legacy scan passed: deprecated adapters, runtime aliases and repository backup/cache artifacts are clean.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
