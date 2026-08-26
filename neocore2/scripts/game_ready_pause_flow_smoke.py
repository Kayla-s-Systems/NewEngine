#!/usr/bin/env python3
"""Windows regression smoke for the authored Game Ready frontend/pause flow.

The test drives the real Winit window and verifies state transitions from runtime
logs instead of calling presentation-flow internals directly.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import subprocess
import sys
import time
from collections import Counter
from ctypes import wintypes
from pathlib import Path

DESIGN_WIDTH = 1600
DESIGN_HEIGHT = 900
WM_CLOSE = 0x0010
SW_RESTORE = 9
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
KEYEVENTF_KEYUP = 0x0002
VK_ESCAPE = 0x1B
VK_E = 0x45
VK_RETURN = 0x0D


class Rect(ctypes.Structure):
    _fields_ = [
        ("left", ctypes.c_long),
        ("top", ctypes.c_long),
        ("right", ctypes.c_long),
        ("bottom", ctypes.c_long),
    ]


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--exe",
        type=Path,
        help="Game Ready executable. Defaults to target/release/game-ready-fps.exe.",
    )
    parser.add_argument(
        "--runtime-root",
        type=Path,
        help="Runtime working directory. Defaults to the NewEngine workspace root.",
    )
    parser.add_argument(
        "--startup-timeout",
        type=float,
        default=45.0,
        help="Seconds allowed for the authored main menu to mount.",
    )
    parser.add_argument(
        "--gameplay-timeout",
        type=float,
        default=120.0,
        help="Seconds allowed for loading to hand off to gameplay.",
    )
    parser.add_argument(
        "--keep-logs",
        action="store_true",
        help="Keep smoke stdout/stderr paths printed even on success.",
    )
    return parser.parse_args()


def transition(source: str, target: str, trigger: str) -> str:
    return (
        "screen profile: presentation flow transition flow='game.frontend' "
        f"from='{source}' to='{target}' trigger='{trigger}'"
    )


def parse_ulog(path: Path) -> tuple[list[dict[str, object]], int]:
    records: list[dict[str, object]] = []
    bad_json = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            bad_json += 1
            continue
        if isinstance(value, dict):
            records.append(value)
    return records, bad_json


def standard_run_shards(log_dir: Path) -> list[Path]:
    return [
        path
        for path in log_dir.glob("current.ulog.*.ndjson")
        if not path.name.endswith(".bootstrap.ndjson")
        and ".orphan." not in path.name
        and path.name != "current.ulog.ndjson"
    ]


def main() -> int:
    if os.name != "nt":
        print("This smoke test requires Windows/Winit input.", file=sys.stderr)
        return 2

    args = parse_args()
    repo_root = Path(__file__).resolve().parents[1]
    runtime_root = (args.runtime_root or repo_root).resolve()
    exe = (args.exe or runtime_root / "target" / "release" / "game-ready-fps.exe").resolve()
    if not exe.is_file():
        print(f"Game Ready executable not found: {exe}", file=sys.stderr)
        return 2

    smoke_dir = runtime_root / "target" / "smoke"
    smoke_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = smoke_dir / "game-ready-fps-pause-flow.stdout.log"
    stderr_path = smoke_dir / "game-ready-fps-pause-flow.stderr.log"
    log_dir = runtime_root / "cache" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    before_shards = {path.resolve() for path in standard_run_shards(log_dir)}

    user32 = ctypes.windll.user32
    process: subprocess.Popen[str] | None = None

    def stderr_text() -> str:
        try:
            size = stderr_path.stat().st_size
            if size > 64 * 1024 * 1024:
                raise RuntimeError(
                    f"runtime stderr exceeded 64 MiB safety limit: {stderr_path} ({size} bytes)"
                )
            return stderr_path.read_text(encoding="utf-8", errors="replace")
        except FileNotFoundError:
            return ""

    def wait_for(needle: str, timeout: float, label: str, minimum_count: int = 1) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if process is not None and process.poll() is not None:
                raise RuntimeError(
                    f"process exited {process.returncode} while waiting for {label}"
                )
            if stderr_text().count(needle) >= minimum_count:
                print(f"PASS {label}")
                return
            time.sleep(0.2)
        tail = "\n".join(stderr_text().splitlines()[-80:])
        raise RuntimeError(f"timeout waiting for {label}: {needle}\n--- stderr tail ---\n{tail}")

    def windows_for_pid(pid: int) -> list[tuple[int, bool, int, str, str, Rect]]:
        found: list[tuple[int, bool, int, str, str, Rect]] = []
        callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)

        @callback_type
        def callback(hwnd: int, _lparam: int) -> bool:
            owner_pid = ctypes.c_ulong()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner_pid))
            if owner_pid.value != pid or not user32.IsWindow(hwnd):
                return True
            title_length = user32.GetWindowTextLengthW(hwnd)
            title = ctypes.create_unicode_buffer(title_length + 1)
            user32.GetWindowTextW(hwnd, title, title_length + 1)
            class_name = ctypes.create_unicode_buffer(256)
            user32.GetClassNameW(hwnd, class_name, 256)
            client = Rect()
            user32.GetClientRect(hwnd, ctypes.byref(client))
            area = max(0, client.right - client.left) * max(0, client.bottom - client.top)
            found.append(
                (
                    area,
                    bool(user32.IsWindowVisible(hwnd)),
                    int(hwnd),
                    title.value,
                    class_name.value,
                    client,
                )
            )
            return True

        user32.EnumWindows(callback, 0)
        return sorted(found, key=lambda value: (value[1], value[0]), reverse=True)

    def game_window() -> tuple[int, Rect]:
        assert process is not None
        candidates = [
            window
            for window in windows_for_pid(process.pid)
            if window[1] and window[0] > 100_000
        ]
        if not candidates:
            raise RuntimeError("no visible Game Ready window")
        _, _, hwnd, title, class_name, client = candidates[0]
        print(
            f"WINDOW hwnd=0x{hwnd:X} title={title!r} class={class_name!r} "
            f"client={client.right - client.left}x{client.bottom - client.top}"
        )
        return hwnd, client

    def activate(hwnd: int) -> None:
        user32.ShowWindow(hwnd, SW_RESTORE)
        user32.SetForegroundWindow(hwnd)
        user32.SetFocus(hwnd)
        time.sleep(0.2)

    def click_design(hwnd: int, x: int, y: int) -> None:
        client = Rect()
        user32.GetClientRect(hwnd, ctypes.byref(client))
        width = max(1, client.right - client.left)
        height = max(1, client.bottom - client.top)
        client_x = round(x * width / DESIGN_WIDTH)
        client_y = round(y * height / DESIGN_HEIGHT)
        point = Point(client_x, client_y)
        user32.ClientToScreen(hwnd, ctypes.byref(point))
        activate(hwnd)
        user32.SetCursorPos(point.x, point.y)
        time.sleep(0.15)
        user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
        time.sleep(0.07)
        user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
        print(f"INPUT click design=({x},{y}) client=({client_x},{client_y})")

    def press_key(hwnd: int, virtual_key: int) -> None:
        activate(hwnd)
        user32.keybd_event(virtual_key, 0, 0, 0)
        time.sleep(0.07)
        user32.keybd_event(virtual_key, 0, KEYEVENTF_KEYUP, 0)
        print(f"INPUT key=0x{virtual_key:02X}")

    env = os.environ.copy()
    env["RUST_BACKTRACE"] = "1"
    for key in (
        "NEWENGINE_PLUGIN_DIR",
        "NEWENGINE_PLUGINS_DIR",
        "NEWENGINE_PLATFORM_RUNTIME_DIR",
        "NEWENGINE_PLATFORM_EARLY_LOG",
        "NEWENGINE_WINIT_EARLY_LOG",
    ):
        env.pop(key, None)

    try:
        with stdout_path.open("w", encoding="utf-8") as stdout_file, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr_file:
            process = subprocess.Popen(
                [str(exe), "--no-startup-window"],
                cwd=runtime_root,
                env=env,
                stdout=stdout_file,
                stderr=stderr_file,
                text=True,
            )
            print(f"PROCESS pid={process.pid}")

            wait_for(
                "authored game .neui mounted ref='ui/frontend/main_menu.neui@surface'",
                args.startup_timeout,
                "main menu mounted",
            )
            hwnd, _ = game_window()

            click_design(hwnd, 240, 336)
            try:
                wait_for(
                    transition("main_menu", "loading", "game.start"),
                    8.0,
                    "main menu -> loading",
                )
            except RuntimeError:
                press_key(hwnd, VK_E)
                try:
                    wait_for(
                        transition("main_menu", "loading", "game.start"),
                        8.0,
                        "main menu -> loading via E",
                    )
                except RuntimeError:
                    press_key(hwnd, VK_RETURN)
                    wait_for(
                        transition("main_menu", "loading", "game.start"),
                        8.0,
                        "main menu -> loading via Enter",
                    )

            wait_for(
                transition("loading", "gameplay", "runtime_ready"),
                args.gameplay_timeout,
                "loading -> gameplay",
            )
            wait_for(
                "authored game .neui mounted ref='ui/game/game_hud.neui@surface'",
                30.0,
                "game HUD mounted",
            )

            press_key(hwnd, VK_ESCAPE)
            gameplay_to_pause = transition(
                "gameplay", "pause", "engine.ui.primary.toggle"
            )
            wait_for(gameplay_to_pause, 10.0, "gameplay -> pause")
            wait_for(
                "authored game .neui mounted ref='ui/engine/pause_menu.neui@surface'",
                20.0,
                "pause menu mounted",
            )

            click_design(hwnd, 260, 419)
            wait_for(
                transition("pause", "gameplay", "game.resume"),
                10.0,
                "pause -> gameplay via Resume",
            )

            press_key(hwnd, VK_ESCAPE)
            wait_for(gameplay_to_pause, 10.0, "second gameplay -> pause", minimum_count=2)

            click_design(hwnd, 260, 497)
            wait_for(
                transition("pause", "pause_settings", "engine.settings.open"),
                10.0,
                "pause -> pause settings",
            )
            wait_for(
                "authored game .neui mounted ref='ui/engine/settings.neui@surface'",
                20.0,
                "settings mounted from pause",
            )

            press_key(hwnd, VK_ESCAPE)
            wait_for(
                transition("pause_settings", "pause", "ui.back"),
                10.0,
                "pause settings -> pause via Back",
            )
    except Exception as error:
        print(f"FAIL {error}", file=sys.stderr)
        return_code = 1
    else:
        return_code = 0
    finally:
        if process is not None and process.poll() is None:
            for window in windows_for_pid(process.pid):
                user32.PostMessageW(window[2], WM_CLOSE, 0, 0)
            try:
                process.wait(timeout=12.0)
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=5.0)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5.0)
        if process is not None:
            print(f"PROCESS exit_code={process.returncode}")
            if process.returncode != 0:
                return_code = 1

    new_shards = [
        path
        for path in standard_run_shards(log_dir)
        if path.resolve() not in before_shards
    ]
    if not new_shards:
        print("FAIL no new standard ULOG shard", file=sys.stderr)
        return_code = 1
    else:
        shard = max(new_shards, key=lambda path: path.stat().st_mtime_ns)
        records, bad_json = parse_ulog(shard)
        levels = Counter(str(record.get("level", "")).upper() for record in records)
        event_ids = Counter(str(record.get("event_id", "")) for record in records)
        print(
            f"ULOG path={shard} rows={len(records)} bad_json={bad_json} "
            f"levels={dict(levels)}"
        )
        if bad_json:
            return_code = 1
        if any(levels.get(level, 0) for level in ("DEBUG", "TRACE", "ERROR", "FATAL")):
            print("FAIL standard ULOG shard contains disallowed levels", file=sys.stderr)
            return_code = 1
        if event_ids.get("engine.services.call_slow", 0):
            print("FAIL direct structured DEBUG event bypassed filter", file=sys.stderr)
            return_code = 1

    if args.keep_logs or return_code:
        print(f"STDOUT {stdout_path}")
        print(f"STDERR {stderr_path}")

    if return_code == 0:
        print("PAUSE_FLOW_SMOKE_OK")
    return return_code


if __name__ == "__main__":
    raise SystemExit(main())
