#!/usr/bin/env python3
"""Interactive physical XInput acceptance for the Game Ready profile.

This scenario never enables NEWENGINE_INPUT_TEST_INGRESS: only a real, non-virtual
controller can satisfy it.
"""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

SUCCESS = "game-ready validation: physical gamepad complete"
STEPS = [
    ("physical controller accepted", "Connect an Xbox/XInput controller."),
    ("movement PASS", "Move the LEFT stick beyond half travel."),
    ("camera look PASS", "Move the RIGHT stick beyond half travel."),
    ("aim PASS", "Hold LT."),
    ("fire PASS", "Hold RT."),
    ("inventory toggle semantic PASS control='LB' press=2", "Press LB twice (Inventory open/close)."),
    ("character menu toggle semantic PASS control='RB' press=2", "Press RB twice (Character Menu open/close)."),
    ("pause presentation PASS", "Press Start once to open Pause."),
    ("UI navigation PASS", "While Pause is open, press D-pad Down once to focus Resume."),
    ("UI accept PASS", "Press A to activate Resume."),
    ("pause resume PASS", "Resume returned to gameplay."),
]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--project", type=Path)
    ap.add_argument("--timeout", type=float, default=300.0)
    args = ap.parse_args()

    root = Path(__file__).resolve().parents[1]
    repo_root = root.parents[1]
    exe = root / "target" / "release" / "NewEngine.exe"
    project = args.project or (repo_root / "Projects" / "room" / "game.toml")
    if not exe.is_file():
        print(f"missing executable: {exe}", file=sys.stderr)
        return 2
    if not project.is_file():
        print(f"missing project: {project}", file=sys.stderr)
        return 2

    smoke = root / "target" / "smoke"
    smoke.mkdir(parents=True, exist_ok=True)
    out_path = smoke / "game-ready-hardware-gamepad.stdout.log"
    err_path = smoke / "game-ready-hardware-gamepad.stderr.log"
    env = os.environ.copy()
    env.pop("NEWENGINE_INPUT_TEST_INGRESS", None)
    env["NEWENGINE_GAME_READY_VALIDATION_SCENARIO"] = "hardware_controller"
    env["NEWENGINE_RENDER_PROFILER_SAMPLES"] = "0"
    env["NEWENGINE_RENDER_TRACE_MS"] = "1000"
    env["NEWENGINE_RENDER_WARN_MS"] = "1000"

    print("P3 PHYSICAL GAMEPAD ACCEPTANCE")
    print("Only non-virtual XInput hardware is accepted.")
    print(STEPS[0][1])
    announced = set()
    with out_path.open("w", encoding="utf-8") as stdout, err_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(
            [str(exe), "--project", str(project), "--launch", "game"],
            cwd=root,
            env=env,
            stdout=stdout,
            stderr=stderr,
            text=True,
        )
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            text = err_path.read_text(encoding="utf-8", errors="replace") if err_path.exists() else ""
            if SUCCESS in text:
                process.wait(timeout=30)
                break
            if process.poll() is not None:
                break
            for index, (token, instruction) in enumerate(STEPS):
                if token in text and index not in announced:
                    announced.add(index)
                    if index + 1 < len(STEPS):
                        print(f"PASS {instruction}")
                        print(f"NEXT {STEPS[index + 1][1]}")
            time.sleep(0.15)
        else:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait(timeout=5)

    text = err_path.read_text(encoding="utf-8", errors="replace")
    if SUCCESS not in text:
        tail = "\n".join(text.splitlines()[-120:])
        print("P3_HARDWARE_GAMEPAD_NOT_COMPLETE", file=sys.stderr)
        print(tail, file=sys.stderr)
        return 1

    required_runtime = [
        "inventory HUD toggled open=true source='player.inventory.toggle'",
        "inventory HUD toggled open=false source='player.inventory.toggle'",
        "character selector toggled open=true",
        "character selector toggled open=false",
        "pause presentation PASS",
        "pause resume PASS",
    ]
    missing = [token for token in required_runtime if token not in text]
    if missing:
        print(f"P3_HARDWARE_GAMEPAD_RUNTIME_CONSUMER_FAIL missing={missing}", file=sys.stderr)
        return 1
    if "virtual:game-ready-controller" in text or "virtual:pad" in text:
        print("P3_HARDWARE_GAMEPAD_FAIL virtual controller evidence detected", file=sys.stderr)
        return 1
    print("P3_HARDWARE_GAMEPAD_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
