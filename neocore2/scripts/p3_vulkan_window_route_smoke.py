#!/usr/bin/env python3
"""P3 hardware/runtime conformance smoke for the active Vulkan RenderProvider route.

Launches the real window-backed Game Ready runtime, observes a real Win32 window,
and validates the structured plugin-host route selection event. No fake native
handles or alternate descriptor parser are used.
"""
from __future__ import annotations

import argparse
import ctypes
import json
import os
import subprocess
import sys
import time
from ctypes import wintypes
from pathlib import Path

WM_CLOSE = 0x0010
EXPECTED = {
    "gateway_id": "engine.render",
    "provider_owner_id": "engine.render.vulkan",
    "provider_route_id": "engine.render.vulkan",
    "provider_service_id": "render.api",
    "backend_capability_id": "render.backend",
    "provider_abi": "newengine.render-provider/v2",
    "origin": "first-party-plugin",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--timeout", type=float, default=60.0)
    parser.add_argument("--keep-logs", action="store_true")
    parser.add_argument(
        "--project",
        type=Path,
        help="Game Ready game.toml to launch when using the generic NewEngine executable",
    )
    return parser.parse_args()


def standard_run_shards(log_dir: Path) -> list[Path]:
    return [
        path
        for path in log_dir.glob("current.ulog.*.ndjson")
        if not path.name.endswith(".bootstrap.ndjson") and ".orphan." not in path.name
    ]


def read_records(path: Path) -> tuple[list[dict[str, object]], int]:
    records: list[dict[str, object]] = []
    bad = 0
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except FileNotFoundError:
        return records, bad
    for line in lines:
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            bad += 1
            continue
        if isinstance(value, dict):
            records.append(value)
    return records, bad


def main() -> int:
    args = parse_args()
    if os.name != "nt":
        print("P3 Vulkan window route smoke requires Windows/Win32.", file=sys.stderr)
        return 2

    root = Path(__file__).resolve().parents[1]
    repo = root.parent.parent
    standalone = root / "target" / "release" / "game-ready-fps.exe"
    release_engine = root / "target" / "release" / "NewEngine.exe"
    debug_engine = root / "target" / "debug" / "NewEngine.exe"
    project = (args.project.resolve() if args.project is not None else repo / "Projects" / "GameReadyFPS" / "game.toml")
    if standalone.is_file():
        command = [str(standalone), "--no-startup-window"]
    else:
        engine = release_engine if release_engine.is_file() else debug_engine
        if not engine.is_file():
            print(
                f"missing runtime executable: checked {release_engine} and {debug_engine}",
                file=sys.stderr,
            )
            return 2
        if not project.is_file():
            print(f"missing Game Ready manifest: {project}", file=sys.stderr)
            return 2
        command = [
            str(engine),
            "--project",
            str(project),
            "--launch",
            "game",
            "--no-startup-window",
        ]

    log_dir = root / "cache" / "logs"
    smoke_dir = root / "target" / "smoke"
    log_dir.mkdir(parents=True, exist_ok=True)
    smoke_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = smoke_dir / "p3-vulkan-window-route.stdout.log"
    stderr_path = smoke_dir / "p3-vulkan-window-route.stderr.log"
    before = {path.resolve() for path in standard_run_shards(log_dir)}

    user32 = ctypes.windll.user32
    process: subprocess.Popen[str] | None = None
    selected: dict[str, object] | None = None
    selected_shard: Path | None = None
    visible_window = False
    public_play = False
    result = 0

    def windows_for_pid(pid: int) -> list[tuple[int, bool, int]]:
        found: list[tuple[int, bool, int]] = []
        callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, wintypes.HWND, wintypes.LPARAM)

        @callback_type
        def callback(hwnd: int, _lparam: int) -> bool:
            owner = ctypes.c_ulong()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
            if owner.value == pid and user32.IsWindow(hwnd):
                rect = wintypes.RECT()
                user32.GetClientRect(hwnd, ctypes.byref(rect))
                width = max(0, rect.right - rect.left)
                height = max(0, rect.bottom - rect.top)
                found.append((width * height, bool(user32.IsWindowVisible(hwnd)), int(hwnd)))
            return True

        user32.EnumWindows(callback, 0)
        return sorted(found, key=lambda item: (item[1], item[0]), reverse=True)

    env = os.environ.copy()
    for key in (
        "NEWENGINE_PLUGIN_DIR",
        "NEWENGINE_PLUGINS_DIR",
        "NEWENGINE_PLATFORM_RUNTIME_DIR",
        "NEWENGINE_PLATFORM_EARLY_LOG",
        "NEWENGINE_WINIT_EARLY_LOG",
    ):
        env.pop(key, None)
    env["NEWENGINE_RENDER_PROFILER_SAMPLES"] = "0"
    env["NEWENGINE_RENDER_TRACE_MS"] = "1000"
    env["NEWENGINE_RENDER_WARN_MS"] = "1000"

    try:
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr:
            process = subprocess.Popen(
                command,
                cwd=root,
                env=env,
                stdout=stdout,
                stderr=stderr,
                text=True,
            )
            print(f"[P3][VULKAN] process pid={process.pid}", flush=True)
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise RuntimeError(
                        f"runtime exited rc={process.returncode} before Vulkan route confirmation"
                    )

                windows = windows_for_pid(process.pid)
                if any(is_visible and area > 100_000 for area, is_visible, _ in windows):
                    visible_window = True

                shards = [
                    path
                    for path in standard_run_shards(log_dir)
                    if path.resolve() not in before
                ]
                for shard in sorted(shards, key=lambda path: path.stat().st_mtime_ns, reverse=True):
                    records, _bad = read_records(shard)
                    for record in records:
                        message = str(record.get("message", ""))
                        if (
                            "game-ready runtime: public play mode activated after scene launch gate release"
                            in message
                        ):
                            public_play = True
                            selected_shard = selected_shard or shard
                        if record.get("event_id") != "engine.gateway.route.selected":
                            continue
                        fields = record.get("fields")
                        if not isinstance(fields, dict) or fields.get("gateway_id") != "engine.render":
                            continue
                        selected = fields
                        selected_shard = shard
                    if selected is not None and public_play:
                        break

                if visible_window and selected is not None and public_play:
                    break
                time.sleep(0.1)

            if not visible_window:
                raise RuntimeError("no visible non-zero Win32 runtime window observed")
            if selected is None:
                raise RuntimeError("no engine.render route selection event observed")
            if not public_play:
                raise RuntimeError("runtime did not reach public Play after scene launch gate release")

            mismatches = {
                key: (selected.get(key), expected)
                for key, expected in EXPECTED.items()
                if selected.get(key) != expected
            }
            if mismatches:
                raise RuntimeError(f"Vulkan active route mismatch: {mismatches}")

            print(
                "[P3][VULKAN] route PASS "
                + " ".join(f"{key}={selected.get(key)!r}" for key in EXPECTED),
                flush=True,
            )
    except Exception as error:
        print(f"[P3][VULKAN][FAIL] {error}", file=sys.stderr)
        result = 1
    finally:
        if process is not None and process.poll() is None:
            for _area, _visible, hwnd in windows_for_pid(process.pid):
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
        if process is not None:
            print(f"[P3][VULKAN] process exit_code={process.returncode}", flush=True)
            if process.returncode not in (0, None):
                result = 1

    if selected_shard is not None:
        records, bad_json = read_records(selected_shard)
        severe = [
            record
            for record in records
            if str(record.get("level", "")).upper() in {"ERROR", "FATAL"}
        ]

        def relevant_to_vulkan_contract(record: dict[str, object]) -> bool:
            if str(record.get("level", "")).upper() == "FATAL":
                return True
            haystack = json.dumps(record, ensure_ascii=False).lower()
            return any(
                token in haystack
                for token in (
                    "engine.render.vulkan",
                    "render.api",
                    "engine.render",
                    "render.backend",
                    "engine.platform",
                    "device lost",
                    "device_loss",
                    "vulkan",
                )
            )

        relevant_errors = [record for record in severe if relevant_to_vulkan_contract(record)]
        unrelated_errors = [record for record in severe if record not in relevant_errors]
        print(
            f"[P3][VULKAN] ulog={selected_shard} rows={len(records)} "
            f"bad_json={bad_json} relevant_errors={len(relevant_errors)} "
            f"unrelated_errors={len(unrelated_errors)}",
            flush=True,
        )
        for record in unrelated_errors:
            print(
                "[P3][VULKAN][UNRELATED] "
                f"event_id={record.get('event_id')} message={record.get('message')!r}",
                flush=True,
            )
        if bad_json or relevant_errors:
            result = 1

    if result == 0:
        print("P3_VULKAN_WINDOW_ROUTE_SMOKE_OK")
    elif args.keep_logs:
        print(f"stdout={stdout_path}")
        print(f"stderr={stderr_path}")
    return result


if __name__ == "__main__":
    raise SystemExit(main())
