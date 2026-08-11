#!/usr/bin/env python3
"""Run the complete Windows Game Ready regression gate and emit a JSON report."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--soak-duration", type=float, default=60.0)
    parser.add_argument(
        "--long-soak-hours",
        type=float,
        default=0.0,
        help="Override soak duration with a shipping endurance interval in hours.",
    )
    parser.add_argument("--streaming-steps", type=int, default=256)
    parser.add_argument("--skip-soak", action="store_true")
    parser.add_argument("--skip-package", action="store_true")
    parser.add_argument("--skip-save-load", action="store_true")
    parser.add_argument("--skip-input-validation", action="store_true")
    parser.add_argument("--skip-streaming", action="store_true")
    parser.add_argument(
        "--package-copy",
        action="store_true",
        help="Use physical copies instead of the default hardlink staging package.",
    )
    parser.add_argument("--continue-on-failure", action="store_true")
    return parser.parse_args()


def main() -> int:
    options = parse_args()
    root = Path(__file__).resolve().parents[1]
    scripts = root / "scripts"
    report_path = root / "target" / "smoke" / "game-ready-suite-report.json"
    report_path.parent.mkdir(parents=True, exist_ok=True)

    gates: list[tuple[str, list[str]]] = [
        ("pause_flow", [sys.executable, str(scripts / "game_ready_pause_flow_smoke.py")]),
        (
            "resolution_scaling",
            [sys.executable, str(scripts / "game_ready_resolution_smoke.py")],
        ),
    ]
    if not options.skip_save_load:
        gates.append(
            ("save_load_restart", [sys.executable, str(scripts / "game_ready_save_load_smoke.py")])
        )
    if not options.skip_input_validation:
        gates.extend(
            [
                (
                    "device_hotplug",
                    [
                        sys.executable,
                        str(scripts / "game_ready_validation_smoke.py"),
                        "hotplug",
                    ],
                ),
                (
                    "controller_only",
                    [
                        sys.executable,
                        str(scripts / "game_ready_validation_smoke.py"),
                        "controller",
                    ],
                ),
            ]
        )
    if not options.skip_streaming:
        gates.append(
            (
                "world_streaming_stress",
                [
                    sys.executable,
                    str(scripts / "game_ready_streaming_stress_smoke.py"),
                    "--steps",
                    str(options.streaming_steps),
                ],
            )
        )
    if not options.skip_soak:
        soak_duration = (
            options.long_soak_hours * 3600.0
            if options.long_soak_hours > 0.0
            else options.soak_duration
        )
        gates.append(
            (
                "gameplay_endurance_soak",
                [
                    sys.executable,
                    str(scripts / "game_ready_render_soak_smoke.py"),
                    "--duration",
                    str(soak_duration),
                ],
            )
        )
    if not options.skip_package:
        package_command = [sys.executable, str(scripts / "game_ready_package_smoke.py")]
        if options.package_copy:
            package_command.append("--copy")
        gates.append(("isolated_package", package_command))

    suite_started = time.monotonic()
    results: list[dict[str, object]] = []
    failed = False
    for name, command in gates:
        print(f"\n=== GAME READY GATE: {name} ===", flush=True)
        print(">", subprocess.list2cmdline(command), flush=True)
        started = time.monotonic()
        result = subprocess.run(command, cwd=root, text=True)
        elapsed = time.monotonic() - started
        gate_result = {
            "name": name,
            "command": command,
            "exit_code": result.returncode,
            "elapsed_seconds": round(elapsed, 3),
            "status": "passed" if result.returncode == 0 else "failed",
        }
        results.append(gate_result)
        print(
            f"=== {name}: {gate_result['status'].upper()} "
            f"({elapsed:.1f}s, exit={result.returncode}) ===",
            flush=True,
        )
        if result.returncode != 0:
            failed = True
            if not options.continue_on_failure:
                break

    report = {
        "schema": "northstar.game-ready.smoke-suite.v2",
        "status": "failed" if failed else "passed",
        "elapsed_seconds": round(time.monotonic() - suite_started, 3),
        "configuration": {
            "soak_duration_seconds": (
                options.long_soak_hours * 3600.0
                if options.long_soak_hours > 0.0
                else options.soak_duration
            ),
            "streaming_steps": options.streaming_steps,
            "package_mode": "copy" if options.package_copy else "hardlink",
        },
        "gates": results,
    }
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"\nREPORT {report_path}")
    if failed:
        print("GAME_READY_SUITE_FAILED")
        return 1
    print("GAME_READY_SUITE_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
