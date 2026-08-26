#!/usr/bin/env python3
"""Build and smoke-test an isolated Game Ready FPS package layout on Windows.

The staging directory is outside the NorthStar source tree. Runtime files are
hard-linked by default to avoid duplicating the large asset tree during local
verification; use --copy for a physically independent staging copy.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(tempfile.gettempdir())
        / f"northstar-game-ready-package-smoke-{os.getpid()}",
        help=(
            "Package staging directory outside the repository. The default is "
            "process-unique so loaded hard-linked DLLs from an earlier smoke cannot "
            "block cleanup."
        ),
    )
    parser.add_argument(
        "--copy",
        action="store_true",
        help="Copy file bytes instead of creating same-volume hard links.",
    )
    parser.add_argument(
        "--keep-existing",
        action="store_true",
        help="Do not recreate an existing package before the smoke test.",
    )
    parser.add_argument(
        "--skip-smoke",
        action="store_true",
        help="Only build the package layout and manifest.",
    )
    return parser.parse_args()


def terminate_packaged_processes(output: Path) -> None:
    executable = str((output / "game-ready-fps.exe").resolve()).replace("'", "''")
    runtime_root = str(output.resolve()).replace("'", "''")
    command = (
        "$target = '" + executable + "'; "
        "$runtimeRoot = '" + runtime_root + "'; "
        "Get-CimInstance Win32_Process | "
        "Where-Object { "
        "$_.ExecutablePath -eq $target -or "
        "($_.CommandLine -like '*game_ready_pause_flow_smoke.py*' -and "
        "$_.CommandLine -like ('*--runtime-root*' + $runtimeRoot + '*')) "
        "} | ForEach-Object { "
        "Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue "
        "}"
    )
    subprocess.run(
        ["powershell", "-NoProfile", "-Command", command],
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    time.sleep(0.5)


def remove_tree(path: Path) -> None:
    if not path.exists():
        return

    def on_error(function: Any, failed_path: str, _error: Any) -> None:
        os.chmod(failed_path, stat.S_IWRITE | stat.S_IREAD | stat.S_IEXEC)
        function(failed_path)

    last_error: OSError | None = None
    for _ in range(5):
        try:
            shutil.rmtree(path, onerror=on_error)
            if not path.exists():
                return
            os.rmdir(path)
            return
        except OSError as error:
            last_error = error
            terminate_packaged_processes(path)
            time.sleep(0.5)
    if last_error is not None:
        raise last_error


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def transfer_file(source: Path, destination: Path, copy_bytes: bool) -> str:
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        destination.unlink()
    if copy_bytes:
        shutil.copy2(source, destination)
        return "copy"
    try:
        os.link(source, destination)
        return "hardlink"
    except OSError:
        shutil.copy2(source, destination)
        return "copy-fallback"


def transfer_tree(
    source: Path,
    destination: Path,
    copy_bytes: bool,
    *,
    predicate: callable | None = None,
) -> tuple[int, int, dict[str, int]]:
    file_count = 0
    logical_bytes = 0
    modes: dict[str, int] = {}
    for source_file in source.rglob("*"):
        if not source_file.is_file():
            continue
        relative = source_file.relative_to(source)
        if predicate is not None and not predicate(relative):
            continue
        mode = transfer_file(source_file, destination / relative, copy_bytes)
        modes[mode] = modes.get(mode, 0) + 1
        file_count += 1
        logical_bytes += source_file.stat().st_size
    return file_count, logical_bytes, modes


def rewrite_package_config(source: Path, destination: Path) -> None:
    data: dict[str, Any] = json.loads(source.read_text(encoding="utf-8"))
    engine = data.setdefault("engine", {})
    engine["modules_dir"] = "pluginsRuntime"
    engine["cache_files"] = "cache"
    engine["config"] = "config"

    plugins = data.setdefault("plugins", {})
    assets = plugins.get("engine.assets.starvault")
    if isinstance(assets, dict):
        # StarVault resolves codecs_dir relative to the active modules root.
        assets["codecs_dir"] = "codecs"
        layers = assets.get("layers", [])
        if isinstance(layers, list):
            for layer in layers:
                if not isinstance(layer, dict):
                    continue
                for key in ("path", "dir"):
                    value = layer.get(key)
                    if isinstance(value, str) and "gameAssets" in value:
                        layer[key] = "gameAssets/"

    destination.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def active_plugin_file(relative: Path) -> bool:
    if "archive" in {part.lower() for part in relative.parts}:
        return False
    return relative.suffix.lower() == ".dll" or relative.name == "codec_manifest.json"


def build_package(output: Path, copy_bytes: bool) -> dict[str, Any]:
    workspace_root = Path(__file__).resolve().parents[1]
    northstar_root = workspace_root.parents[1]
    source_exe = workspace_root / "target" / "release" / "game-ready-fps.exe"
    source_config = workspace_root / "config.json"
    source_plugins = northstar_root / "pluginsRuntime"
    source_assets = northstar_root / "gameAssets"
    source_shared_content = northstar_root / "Shared" / "Content"

    for required in (
        source_exe,
        source_config,
        source_plugins,
        source_assets,
        source_shared_content,
    ):
        if not required.exists():
            raise FileNotFoundError(f"required package source is missing: {required}")

    output.parent.mkdir(parents=True, exist_ok=True)
    terminate_packaged_processes(output)
    remove_tree(output)
    output.mkdir(parents=True)

    transfer_modes: dict[str, int] = {}
    exe_mode = transfer_file(source_exe, output / "game-ready-fps.exe", copy_bytes)
    transfer_modes[exe_mode] = transfer_modes.get(exe_mode, 0) + 1
    rewrite_package_config(source_config, output / "config.json")

    plugin_count, plugin_bytes, plugin_modes = transfer_tree(
        source_plugins,
        output / "pluginsRuntime",
        copy_bytes,
        predicate=active_plugin_file,
    )
    asset_count, asset_bytes, asset_modes = transfer_tree(
        source_assets,
        output / "gameAssets",
        copy_bytes,
    )
    shared_count, shared_bytes, shared_modes = transfer_tree(
        source_shared_content,
        output / "Shared" / "Content",
        copy_bytes,
    )
    for modes in (plugin_modes, asset_modes, shared_modes):
        for key, value in modes.items():
            transfer_modes[key] = transfer_modes.get(key, 0) + value

    active_plugins = sorted(
        str(path.relative_to(output)).replace("\\", "/")
        for path in (output / "pluginsRuntime").rglob("*.dll")
    )
    top_level_plugins = [
        path for path in (output / "pluginsRuntime").glob("*.dll") if path.is_file()
    ]
    if len(top_level_plugins) < 9:
        raise RuntimeError(
            f"package has only {len(top_level_plugins)} top-level provider DLLs; expected at least 9"
        )

    manifest = {
        "schema": "northstar.game-ready.package.v1",
        "created_unix_ms": int(time.time() * 1000),
        "package_root": str(output.resolve()),
        "source_workspace": str(workspace_root),
        "transfer_modes": transfer_modes,
        "files": {
            "executable": {
                "path": "game-ready-fps.exe",
                "bytes": (output / "game-ready-fps.exe").stat().st_size,
                "sha256": sha256_file(output / "game-ready-fps.exe"),
            },
            "config": {
                "path": "config.json",
                "bytes": (output / "config.json").stat().st_size,
                "sha256": sha256_file(output / "config.json"),
            },
            "plugins": {
                "count": plugin_count,
                "logical_bytes": plugin_bytes,
                "active": active_plugins,
            },
            "assets": {
                "count": asset_count,
                "logical_bytes": asset_bytes,
            },
            "shared_content": {
                "count": shared_count,
                "logical_bytes": shared_bytes,
            },
        },
    }
    required_runtime_assets = [
        output / "gameAssets" / "maps" / "forest_road_operation.ymap",
        output / "gameAssets" / "ui" / "engine" / "main_menu.neui",
        output / "gameAssets" / "ui" / "engine" / "pause_menu.neui",
        output / "Shared" / "Content" / "textures" / "highres" / "vegetation.ytd",
    ]
    missing_runtime_assets = [str(path) for path in required_runtime_assets if not path.is_file()]
    if missing_runtime_assets:
        raise RuntimeError(
            "package is missing required runtime assets: " + ", ".join(missing_runtime_assets)
        )
    actual_asset_count = sum(1 for path in (output / "gameAssets").rglob("*") if path.is_file())
    if actual_asset_count != asset_count:
        raise RuntimeError(
            f"package asset transfer incomplete: manifest={asset_count} actual={actual_asset_count}"
        )

    manifest_path = output / "package-manifest.json"
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return manifest


def smoke_package(output: Path) -> int:
    workspace_root = Path(__file__).resolve().parents[1]
    pause_smoke = workspace_root / "scripts" / "game_ready_pause_flow_smoke.py"
    command = [
        sys.executable,
        str(pause_smoke),
        "--exe",
        str(output / "game-ready-fps.exe"),
        "--runtime-root",
        str(output),
        "--keep-logs",
    ]
    print(">", subprocess.list2cmdline(command), flush=True)
    try:
        result = subprocess.run(command, cwd=output, text=True, timeout=240)
    except subprocess.TimeoutExpired:
        terminate_packaged_processes(output)
        print("FAIL packaged pause-flow smoke exceeded 240 seconds", file=sys.stderr)
        return 1
    if result.returncode:
        terminate_packaged_processes(output)
        return result.returncode

    source_root = str(workspace_root.parents[1].resolve()).lower()
    runtime_outputs = [
        output / "target" / "smoke" / "game-ready-fps-pause-flow.stdout.log",
        output / "target" / "smoke" / "game-ready-fps-pause-flow.stderr.log",
    ]
    runtime_outputs.extend((output / "cache" / "logs").glob("*.ndjson"))
    leaks: list[str] = []
    for path in runtime_outputs:
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace").lower()
        if source_root in text:
            leaks.append(str(path))
    if leaks:
        print(
            "FAIL packaged runtime referenced the developer source root in: "
            + ", ".join(leaks),
            file=sys.stderr,
        )
        return 1

    canonical = output / "cache" / "logs" / "current.ulog.ndjson"
    if canonical.exists():
        print("FAIL packaged runtime recreated canonical current.ulog.ndjson", file=sys.stderr)
        return 1

    print("PACKAGE_SMOKE_OK")
    return 0


def main() -> int:
    options = parse_args()
    output = options.output.resolve()
    workspace_root = Path(__file__).resolve().parents[1]
    if workspace_root == output or workspace_root in output.parents:
        print("Package output must be outside the NewEngine source tree.", file=sys.stderr)
        return 2

    try:
        if not options.keep_existing:
            manifest = build_package(output, options.copy)
            files = manifest["files"]
            print(
                "PACKAGE_BUILT "
                f"root={output} plugins={files['plugins']['count']} "
                f"assets={files['assets']['count']} "
                f"logical_bytes={files['plugins']['logical_bytes'] + files['assets']['logical_bytes'] + files['shared_content']['logical_bytes']} "
                f"modes={manifest['transfer_modes']}"
            )
        elif not (output / "package-manifest.json").is_file():
            raise FileNotFoundError(
                f"--keep-existing requires {output / 'package-manifest.json'}"
            )
    except Exception as error:
        print(f"FAIL package build: {error}", file=sys.stderr)
        return 1

    if options.skip_smoke:
        return 0
    return smoke_package(output)


if __name__ == "__main__":
    raise SystemExit(main())
