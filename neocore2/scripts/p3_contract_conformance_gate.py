#!/usr/bin/env python3
from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path


def run(label: str, cmd: list[str], cwd: Path) -> None:
    started = time.monotonic()
    print(f"[P3][RUN] {label}: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True)
    elapsed = time.monotonic() - started
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode:
        raise SystemExit(f"[P3][FAIL] {label}: rc={result.returncode} elapsed={elapsed:.2f}s")
    print(f"[P3][PASS] {label}: elapsed={elapsed:.2f}s")


def main() -> int:
    parser = argparse.ArgumentParser(description="NorthStar P3 cross-component contract conformance gate")
    parser.add_argument(
        "--full-assets",
        action="store_true",
        help="also run the full owner-scoped native asset validation (slower)",
    )
    parser.add_argument(
        "--hardware-runtime",
        action="store_true",
        help="also run the real Win32/window-backed Vulkan active-route smoke",
    )
    ns = parser.parse_args()

    neocore = Path(__file__).resolve().parents[1]
    repo = neocore.parent.parent
    providers = repo / "PluginsSrc"

    run("kernel dependency gate", [sys.executable, "scripts/kernel_dependency_gate.py"], neocore)
    run("dependency direction gate", [sys.executable, "scripts/dependency_direction_gate.py"], neocore)
    run("P1 compatibility scanner", [sys.executable, "scripts/p1_capability_conformance_scan.py"], neocore)
    run(
        "typed contract core",
        [
            "cargo",
            "test",
            "-p",
            "newengine-contract-api",
            "-p",
            "newengine-contract-registry",
            "-p",
            "newengine-contract-conformance",
            "-p",
            "newengine-plugin-host",
            "--",
            "--nocapture",
        ],
        neocore,
    )
    run(
        "loaded null-provider route snapshots",
        [
            "cargo",
            "test",
            "-p",
            "newengine-null-providers-runtime",
            "loaded_null_provider_routes_conform_to_registered_abis",
            "--",
            "--nocapture",
        ],
        neocore,
    )

    run(
        "loaded headless-safe first-party DLL routes",
        [
            "cargo",
            "test",
            "-p",
            "newengine-runtime-host",
            "loaded_headless_safe_first_party_provider_routes_conform_to_registry",
            "--",
            "--nocapture",
        ],
        neocore,
    )

    provider_tests = [
        (
            "Vulkan RenderProvider",
            providers / "VulkanRenderer",
            "engine-render-vulkan",
            "vulkan_descriptor_conforms_to_render_provider_contract",
        ),
        (
            "Gravitas PhysicsProvider",
            providers / "JoltPhysics",
            "engine-physics-gravitas",
            "gravitas_descriptor_conforms_to_physics_provider_contract",
        ),
        (
            "Egui UiProvider",
            providers / "EguiUI",
            "engine-ui-egui",
            "egui_descriptor_conforms_to_ui_provider_contract",
        ),
        (
            "Aurelia UiProvider",
            providers / "AureliaUI",
            "engine-ui-aurelia",
            "aurelia_descriptor_conforms_to_ui_provider_contract",
        ),
    ]
    for label, cwd, package, test_name in provider_tests:
        run(
            label,
            ["cargo", "test", "-p", package, test_name, "--", "--nocapture"],
            cwd,
        )

    run("installed tool -> runtime contracts", [sys.executable, "scripts/p4_tool_runtime_conformance.py"], neocore)

    if ns.hardware_runtime:
        run(
            "window-backed Vulkan active route",
            [sys.executable, "scripts/p3_vulkan_window_route_smoke.py"],
            neocore,
        )

    if ns.full_assets:
        run(
            "full owner-scoped native asset validation",
            [sys.executable, "tools/maintenance/northstar_native_assets.py", "validate"],
            repo,
        )

    print("[P3] CONTRACT CONFORMANCE GATE PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
