#!/usr/bin/env python3
"""End-to-end Game Ready world save/restart/load regression smoke on Windows."""
from __future__ import annotations

import ctypes
import json
import os
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

WM_CLOSE = 0x0010
SW_RESTORE = 9
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004

class Rect(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long), ("right", ctypes.c_long), ("bottom", ctypes.c_long)]

class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


def window_for_pid(user32: object, pid: int, timeout: float = 20.0) -> int:
    deadline = time.monotonic() + timeout
    callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)
    while time.monotonic() < deadline:
        found: list[int] = []
        @callback_type
        def callback(hwnd: int, _lparam: int) -> bool:
            owner = ctypes.c_ulong()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
            if owner.value == pid and user32.IsWindowVisible(hwnd):
                length = user32.GetWindowTextLengthW(hwnd)
                title = ctypes.create_unicode_buffer(length + 1)
                user32.GetWindowTextW(hwnd, title, length + 1)
                if "North Star Game Ready FPS" in title.value:
                    found.append(hwnd)
                    return False
            return True
        user32.EnumWindows(callback, 0)
        if found:
            return found[0]
        time.sleep(0.2)
    raise RuntimeError("Game Ready window not found")


def click_start(user32: object, hwnd: int) -> None:
    user32.ShowWindow(hwnd, SW_RESTORE)
    user32.SetForegroundWindow(hwnd)
    point = Point(240, 336)
    if not user32.ClientToScreen(hwnd, ctypes.byref(point)):
        raise RuntimeError("ClientToScreen failed")
    user32.SetCursorPos(point.x, point.y)
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)


def run_phase(root: Path, exe: Path, save_path: Path, scenario: str) -> Path:
    smoke_dir = root / "target" / "smoke"
    smoke_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = smoke_dir / f"game-ready-save-load-{scenario}.stdout.log"
    stderr_path = smoke_dir / f"game-ready-save-load-{scenario}.stderr.log"
    env = os.environ.copy()
    env["NEWENGINE_GAME_READY_VALIDATION_SCENARIO"] = scenario
    env["NEWENGINE_GAME_READY_VALIDATION_SAVE_PATH"] = str(save_path.resolve())
    env["NEWENGINE_RENDER_PROFILER_SAMPLES"] = "0"
    env["NEWENGINE_RENDER_TRACE_MS"] = "1000"
    env["NEWENGINE_RENDER_WARN_MS"] = "1000"

    success = f"game-ready validation: {scenario} snapshot complete"
    with stdout_path.open("w", encoding="utf-8") as stdout_file, stderr_path.open("w", encoding="utf-8") as stderr_file:
        process = subprocess.Popen(
            [str(exe), "--no-startup-window"],
            cwd=root,
            env=env,
            stdout=stdout_file,
            stderr=stderr_file,
            text=True,
        )
        print(f"{scenario.upper()} pid={process.pid}")
        user32 = ctypes.windll.user32
        try:
            deadline = time.monotonic() + 45.0
            while time.monotonic() < deadline:
                text = stderr_path.read_text(encoding="utf-8", errors="replace") if stderr_path.exists() else ""
                if "authored game .neui mounted ref='ui/frontend/main_menu.neui@surface'" in text:
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"{scenario} process exited {process.returncode} before main menu")
                time.sleep(0.2)
            else:
                raise RuntimeError(f"{scenario} main menu timeout")

            hwnd = window_for_pid(user32, process.pid)
            click_start(user32, hwnd)
            deadline = time.monotonic() + 150.0
            while time.monotonic() < deadline:
                text = stderr_path.read_text(encoding="utf-8", errors="replace")
                if success in text:
                    print(f"PASS {scenario} validation event")
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"{scenario} process exited {process.returncode} before success event")
                time.sleep(0.2)
            else:
                raise RuntimeError(f"{scenario} validation timeout")

            process.wait(timeout=20.0)
            if process.returncode != 0:
                raise RuntimeError(f"{scenario} process exit={process.returncode}")
        finally:
            if process.poll() is None:
                for hwnd in [window_for_pid(user32, process.pid, 1.0)]:
                    user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
                try:
                    process.wait(timeout=5.0)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5.0)
    return stderr_path


def main() -> int:
    if os.name != "nt":
        print("Windows is required", file=sys.stderr)
        return 2
    root = Path(__file__).resolve().parents[1]
    exe = root / "target" / "release" / "game-ready-fps.exe"
    if not exe.is_file():
        print(f"missing executable: {exe}", file=sys.stderr)
        return 2
    save_path = root / "target" / "smoke" / "game-ready-world-save.json"
    save_path.unlink(missing_ok=True)

    try:
        save_log = run_phase(root, exe, save_path, "save")
        if not save_path.is_file():
            raise RuntimeError("save file was not created")
        payload = json.loads(save_path.read_text(encoding="utf-8"))
        if payload.get("schema") != "newengine.world.snapshot.v1":
            raise RuntimeError(f"unexpected save schema: {payload.get('schema')!r}")
        state = payload.get("state") or {}
        scene = payload.get("scene_payload")
        if not isinstance(scene, dict):
            raise RuntimeError("save is missing scene_payload")
        print(
            "SAVE_FILE "
            f"bytes={save_path.stat().st_size} world={state.get('world_instance_id')} "
            f"entities={state.get('entity_count')} cells={len(state.get('active_cells') or [])}"
        )
        load_log = run_phase(root, exe, save_path, "load")
    except Exception as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1

    for log_path in (save_log, load_log):
        text = log_path.read_text(encoding="utf-8", errors="replace")
        if "[ERROR" in text or "[FATAL" in text:
            print(f"FAIL disallowed severity in {log_path}", file=sys.stderr)
            return 1
    print("SAVE_LOAD_SMOKE_OK")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
