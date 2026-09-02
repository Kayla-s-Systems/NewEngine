#!/usr/bin/env python3
"""P0 gate: process environment is launcher/bootstrap input, never live runtime state.

Runtime may consume values copied into an Engine-owned HostContext snapshot. Direct
`std::env` variable reads/writes are allowed only in explicitly named bootstrap ingress
functions. New runtime accesses fail this gate so multi-instance behavior stays hermetic.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PROCESS_ENV = re.compile(
    r"\bstd::env::(?:var|var_os|vars|vars_os|set_var|remove_var)\b"
)
FUNCTION = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)\b"
)

STRICT_RUNTIME_ROOTS = (
    ROOT / "crates/newengine-engine-runtime/src",
    ROOT / "crates/newengine-audio-runtime/src",
    ROOT / "crates/newengine-authored-world-runtime/src",
    ROOT / "crates/newengine-asset-bootstrap-runtime/src",
    ROOT / "crates/newengine-asset-hot-reload-runtime/src",
    ROOT / "crates/newengine-asset-inspector-runtime/src",
    ROOT / "crates/newengine-core/src/engine",
    ROOT / "crates/newengine-core/src/startup",
    ROOT / "crates/newengine-core/src/startup_window",
)

ALLOWED_INGRESS_FUNCTIONS: dict[str, set[str]] = {
    "crates/newengine-runtime-host/src/app_launcher/logging.rs": {
        "prepare_early_log_session",
        "bind_early_log_to_run",
        "early_log_path_candidates",
        "cache_root_from_env_or_neocore2",
    },
    "crates/newengine-windowed-host-runtime/src/platform_runtime/early_log.rs": {
        "candidate_paths",
    },
    "crates/newengine-game-ready-profile/src/game_ready_fps.rs": {
        "apply_game_ready_fps_env_policy",
    },
    "crates/newengine-game-ready-profile/src/lib.rs": {
        "launch_game_ready_profile_with",
    },
    "crates/newengine-project-runtime/src/lib.rs": {
        "set_default_env",
        "apply_project_ui_env",
        "game_manifest_request_from_process",
        "project_request_from_process",
        "project_launch_request_from_process",
    },
    "crates/newengine-project-runtime/src/launch_env.rs": {
        "set_default_env",
        "apply_project_ui_env",
    },
    "crates/newengine-project-runtime/src/project_browser.rs": {
        "default_projects_root",
    },
}

CONTROLLED_ROOTS = (
    ROOT / "crates/newengine-runtime-host/src/app_launcher",
    ROOT / "crates/newengine-windowed-host-runtime/src",
    ROOT / "crates/newengine-game-ready-profile/src",
    ROOT / "crates/newengine-project-runtime/src",
    ROOT / "crates/newengine-plugin-host/src",
)


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def production_lines(path: Path):
    """Yield (line_no, text, enclosing_fn) before a conventional cfg(test) tail module."""
    current_fn: str | None = None
    fn_base_depth: int | None = None
    fn_opened = False
    depth = 0
    pending_test_cfg = False

    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith("#[cfg(test)]"):
            pending_test_cfg = True
            continue
        if pending_test_cfg and (
            stripped.startswith("mod tests") or stripped.startswith("mod test")
        ):
            break
        pending_test_cfg = False

        match = FUNCTION.match(line)
        if match:
            current_fn = match.group(1)
            fn_base_depth = depth
            fn_opened = False

        yield line_no, line, current_fn

        opens = line.count("{")
        depth += opens - line.count("}")
        if current_fn is not None and not fn_opened and opens:
            fn_opened = True
        if (
            current_fn is not None
            and fn_opened
            and fn_base_depth is not None
            and depth <= fn_base_depth
        ):
            current_fn = None
            fn_base_depth = None
            fn_opened = False



def scan_file(path: Path, allowed_functions: set[str], errors: list[str]) -> None:
    for line_no, line, current_fn in production_lines(path):
        if not PROCESS_ENV.search(line):
            continue
        if current_fn in allowed_functions:
            continue
        errors.append(
            f"{rel(path)}:{line_no}: process environment access in runtime scope"
            + (f" fn={current_fn}" if current_fn else "")
            + f": {line.strip()}"
        )


def scan_tree(root: Path, errors: list[str], *, controlled: bool) -> None:
    if not root.is_dir():
        return
    for path in sorted(root.rglob("*.rs")):
        if any(part in {"target", "Intermediate"} for part in path.parts):
            continue
        if controlled and rel(path) == "crates/newengine-runtime-host/src/app_launcher/bootstrap.rs":
            # The canonical launcher boundary has a dedicated stronger verifier below.
            continue
        allowed = (
            ALLOWED_INGRESS_FUNCTIONS.get(rel(path), set()) if controlled else set()
        )
        scan_file(path, allowed, errors)


def verify_launcher_boundary(errors: list[str]) -> None:
    path = ROOT / "crates/newengine-runtime-host/src/app_launcher/bootstrap.rs"
    source = path.read_text(encoding="utf-8")

    ctor = source.find("create_host_context_with_environment_snapshot(")
    snapshot = source.find("std::env::vars_os()", ctor)
    build = source.find("self.build_engine(&startup, host_context.clone())")

    if ctor < 0:
        errors.append(
            f"{rel(path)}: launcher must construct HostContext from an explicit snapshot"
        )
    if snapshot < 0:
        errors.append(f"{rel(path)}: one process-environment snapshot is missing")
    if source.count("std::env::vars_os()") != 1:
        errors.append(
            f"{rel(path)}: launcher must snapshot process environment exactly once"
        )
    if "host_context.refresh_environment_from_process()" in source:
        errors.append(f"{rel(path)}: implicit process refresh is forbidden")
    if "host_context.replace_environment_snapshot(std::env::vars_os())" in source:
        errors.append(
            f"{rel(path)}: launcher must not re-snapshot process environment"
         )
    if min(ctor, snapshot, build) >= 0 and not (ctor <= snapshot < build):
        errors.append(
            f"{rel(path)}: expected snapshot during explicit HostContext construction "
            "before Engine construction"
        )

    process_env_hits = list(PROCESS_ENV.finditer(source))
    if len(process_env_hits) != 1:
        errors.append(
            f"{rel(path)}: process environment must be touched only by the single "
            "HostContext snapshot"
        )
    elif not source[process_env_hits[0].start():].startswith("std::env::vars_os("):
        errors.append(
            f"{rel(path)}: the sole process-environment access must be std::env::vars_os()"
         )


def verify_host_fallback(errors: list[str]) -> None:
    path = ROOT / "crates/newengine-plugin-host/src/host_context/state/global.rs"
    source = path.read_text(encoding="utf-8")
    ctx_start = source.find("pub(crate) fn ctx()")
    services_start = source.find("pub fn services_generation()", ctx_start)
    if ctx_start < 0 or services_start < 0:
        errors.append(f"{rel(path)}: unable to locate HostContext implicit fallback")
        return
    ctx_body = source[ctx_start:services_start]
    if "make_default_ctx()" in ctx_body or "std::env::" in ctx_body:
        errors.append(
            f"{rel(path)}: implicit current_host_context fallback must never snapshot process environment"
        )


def main() -> int:
    errors: list[str] = []

    for root in STRICT_RUNTIME_ROOTS:
        scan_tree(root, errors, controlled=False)
    for root in CONTROLLED_ROOTS:
        scan_tree(root, errors, controlled=True)

    scan_file(ROOT / "crates/newengine-core/src/storage_root.rs", set(), errors)

    verify_launcher_boundary(errors)
    verify_host_fallback(errors)

    if errors:
        print("[environment-isolation-gate] FAILED")
        for error in errors:
            print(f"  - {error}")
        return 1

    print("[environment-isolation-gate] OK")
    print("  process env -> launcher snapshot once -> HostContext -> runtime")
    return 0


if __name__ == "__main__":
    sys.exit(main())
