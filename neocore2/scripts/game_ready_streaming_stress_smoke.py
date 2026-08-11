#!/usr/bin/env python3
"""Stress engine.world partition churn and enforce a process-memory plateau."""
from __future__ import annotations

import argparse
import ctypes
import os
import statistics
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

WM_CLOSE = 0x0010
SW_RESTORE = 9
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_VM_READ = 0x0010

class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]

class ProcessMemoryCounters(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--steps", type=int, default=512)
    parser.add_argument("--timeout", type=float, default=240.0)
    parser.add_argument("--plateau-growth-mib", type=float, default=96.0)
    return parser.parse_args()


def window_for_pid(pid: int, timeout: float = 30.0) -> int:
    user32 = ctypes.windll.user32
    callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)
    deadline = time.monotonic() + timeout
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


def click_start(hwnd: int) -> None:
    user32 = ctypes.windll.user32
    user32.ShowWindow(hwnd, SW_RESTORE)
    user32.SetForegroundWindow(hwnd)
    point = Point(240, 336)
    if not user32.ClientToScreen(hwnd, ctypes.byref(point)):
        raise RuntimeError("ClientToScreen failed")
    user32.SetCursorPos(point.x, point.y)
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)


def working_set_bytes(pid: int) -> int | None:
    kernel32 = ctypes.windll.kernel32
    psapi = ctypes.windll.psapi
    handle = kernel32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
    if not handle:
        return None
    try:
        counters = ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            return None
        return int(counters.WorkingSetSize)
    finally:
        kernel32.CloseHandle(handle)


def main() -> int:
    if os.name != "nt":
        print("Windows is required", file=sys.stderr)
        return 2
    args = parse_args()
    steps = max(8, min(args.steps, 4096))
    root = Path(__file__).resolve().parents[1]
    exe = root / "target" / "release" / "game-ready-fps.exe"
    smoke = root / "target" / "smoke"
    smoke.mkdir(parents=True, exist_ok=True)
    stdout_path = smoke / "game-ready-streaming-stress.stdout.log"
    stderr_path = smoke / "game-ready-streaming-stress.stderr.log"
    env = os.environ.copy()
    env["NEWENGINE_GAME_READY_VALIDATION_SCENARIO"] = "streaming"
    env["NEWENGINE_GAME_READY_STREAMING_STEPS"] = str(steps)
    env["NEWENGINE_RENDER_PROFILER_SAMPLES"] = "0"
    env["NEWENGINE_RENDER_TRACE_MS"] = "1000"
    env["NEWENGINE_RENDER_WARN_MS"] = "1000"
    success = "game-ready validation: world streaming stress complete"

    samples: list[tuple[float, int]] = []
    process: subprocess.Popen[str] | None = None
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(
            [str(exe), "--no-startup-window"],
            cwd=root,
            env=env,
            stdout=stdout,
            stderr=stderr,
            text=True,
        )
        print(f"PROCESS pid={process.pid} steps={steps}")
        try:
            deadline = time.monotonic() + 45.0
            while time.monotonic() < deadline:
                text = stderr_path.read_text(encoding="utf-8", errors="replace") if stderr_path.exists() else ""
                if "authored game .neui mounted ref='ui/engine/main_menu.neui@surface'" in text:
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"process exited {process.returncode} before main menu")
                time.sleep(0.2)
            else:
                raise RuntimeError("main menu timeout")
            click_start(window_for_pid(process.pid))

            deadline = time.monotonic() + args.timeout
            started = time.monotonic()
            while time.monotonic() < deadline:
                text = stderr_path.read_text(encoding="utf-8", errors="replace")
                ws = working_set_bytes(process.pid)
                if ws is not None:
                    samples.append((time.monotonic() - started, ws))
                if success in text:
                    break
                if process.poll() is not None:
                    raise RuntimeError(f"process exited {process.returncode} before streaming success")
                time.sleep(0.5)
            else:
                raise RuntimeError("streaming validation timeout")
            process.wait(timeout=20.0)
            if process.returncode != 0:
                raise RuntimeError(f"process exit={process.returncode}")
        except Exception as error:
            if process.poll() is None:
                for hwnd in [window_for_pid(process.pid, 1.0)]:
                    ctypes.windll.user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait(timeout=5)
            print(f"FAIL {error}", file=sys.stderr)
            return 1

    text = stderr_path.read_text(encoding="utf-8", errors="replace")
    if "[ERROR" in text or "[FATAL" in text:
        print("FAIL disallowed severity", file=sys.stderr)
        return 1
    progress = text.count("game-ready validation: world streaming progress")
    if progress < max(1, steps // 32):
        print(f"FAIL insufficient progress events: {progress}", file=sys.stderr)
        return 1

    warm = [(t, value) for t, value in samples if t >= 5.0]
    if len(warm) < 10:
        print(f"FAIL insufficient memory samples: {len(warm)}", file=sys.stderr)
        return 1
    third = max(1, len(warm) // 3)
    baseline = statistics.median(value for _, value in warm[:third])
    tail = statistics.median(value for _, value in warm[-third:])
    peak = max(value for _, value in warm)
    growth = tail - baseline
    allowed = int(args.plateau_growth_mib * 1024 * 1024)
    duration = warm[-1][0] - warm[0][0]
    slope_mib_min = (growth / 1024 / 1024) / max(duration / 60.0, 1e-6)
    print(
        "MEMORY_PLATEAU "
        f"samples={len(warm)} baseline_mib={baseline/1024/1024:.1f} "
        f"tail_mib={tail/1024/1024:.1f} peak_mib={peak/1024/1024:.1f} "
        f"growth_mib={growth/1024/1024:.1f} slope_mib_min={slope_mib_min:.1f} "
        f"allowed_growth_mib={args.plateau_growth_mib:.1f}"
    )
    if growth > allowed:
        print("FAIL working-set plateau growth exceeded", file=sys.stderr)
        return 1
    print("STREAMING_STRESS_SMOKE_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
