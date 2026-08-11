#!/usr/bin/env python3
"""Run an autonomous Game Ready validation scenario and verify its runtime log."""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scenario", choices=["controller", "hotplug"])
    parser.add_argument("--timeout", type=float, default=180.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[1]
    exe = root / "target" / "release" / "game-ready-fps.exe"
    if not exe.is_file():
        print(f"missing executable: {exe}", file=sys.stderr)
        return 2
    smoke = root / "target" / "smoke"
    smoke.mkdir(parents=True, exist_ok=True)
    stdout_path = smoke / f"game-ready-{args.scenario}.stdout.log"
    stderr_path = smoke / f"game-ready-{args.scenario}.stderr.log"
    env = os.environ.copy()
    env["NEWENGINE_GAME_READY_VALIDATION_SCENARIO"] = args.scenario
    env["NEWENGINE_INPUT_TEST_INGRESS"] = "1"
    env["NEWENGINE_RENDER_PROFILER_SAMPLES"] = "0"
    env["NEWENGINE_RENDER_TRACE_MS"] = "1000"
    env["NEWENGINE_RENDER_WARN_MS"] = "1000"
    success = {
        "controller": "game-ready validation: controller-only flow complete",
        "hotplug": "game-ready validation: device hot-plug complete",
    }[args.scenario]

    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(
            [str(exe), "--no-startup-window"],
            cwd=root,
            env=env,
            stdout=stdout,
            stderr=stderr,
            text=True,
        )
        print(f"PROCESS scenario={args.scenario} pid={process.pid}")
        deadline = time.monotonic() + args.timeout
        found = False
        while time.monotonic() < deadline:
            text = stderr_path.read_text(encoding="utf-8", errors="replace") if stderr_path.exists() else ""
            if success in text:
                found = True
                break
            if process.poll() is not None:
                break
            time.sleep(0.2)
        if not found:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait(timeout=5)
            tail = "\n".join(stderr_path.read_text(encoding="utf-8", errors="replace").splitlines()[-100:])
            print(f"FAIL missing success event {success}\n{tail}", file=sys.stderr)
            return 1
        process.wait(timeout=20)
        if process.returncode != 0:
            print(f"FAIL process exit={process.returncode}", file=sys.stderr)
            return 1

    text = stderr_path.read_text(encoding="utf-8", errors="replace")
    if "[ERROR" in text or "[FATAL" in text:
        print("FAIL disallowed severity", file=sys.stderr)
        return 1
    if args.scenario == "controller":
        required = [
            "from='main_menu' to='loading' trigger='game.start'",
            "from='loading' to='gameplay' trigger='runtime_ready'",
            "from='gameplay' to='pause' trigger='engine.ui.primary.toggle'",
            "from='pause' to='pause_settings' trigger='engine.settings.open'",
            "from='pause_settings' to='pause' trigger='ui.back'",
            "from='pause' to='gameplay' trigger='engine.ui.primary.toggle'",
        ]
        missing = [item for item in required if item not in text]
        if missing:
            print(f"FAIL controller transitions missing: {missing}", file=sys.stderr)
            return 1
    print(f"{args.scenario.upper()}_SMOKE_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
