#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

REGISTRY_SCHEMA = "northstar.tool_runtime_conformance.v1"
REGISTRY_VERSION = 1


def run(cmd: list[str], cwd: Path, *, label: str, env: dict[str, str] | None = None) -> None:
    print(f"[P4][RUN] {label}: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, env=env)
    if result.stdout:
        print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode:
        raise SystemExit(f"[P4][FAIL] {label}: rc={result.returncode}")


def run_capture_json(cmd: list[str], cwd: Path, *, label: str, env: dict[str, str] | None = None) -> bytes:
    print(f"[P4][RUN] {label}: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, env=env)
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode:
        if result.stdout:
            print(result.stdout, end="" if result.stdout.endswith("\n") else "\n")
        raise SystemExit(f"[P4][FAIL] {label}: rc={result.returncode}")
    payload = result.stdout.strip()
    try:
        json.loads(payload)
    except json.JSONDecodeError as error:
        raise SystemExit(
            f"[P4][FAIL] {label}: decoder returned invalid JSON: {error}\n{payload}"
        ) from error
    return payload.encode("utf-8")


def cargo_env(workspace_root: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(workspace_root / "target-p4-conformance")
    return env


def load_registry(neocore: Path) -> list[dict[str, Any]]:
    cmd = [
        "cargo",
        "run",
        "-q",
        "-p",
        "newengine-contract-conformance",
        "--example",
        "tool_runtime_registry_json",
    ]
    result = subprocess.run(cmd, cwd=neocore, text=True, capture_output=True, env=cargo_env(neocore))
    if result.stderr:
        print(result.stderr, end="" if result.stderr.endswith("\n") else "\n", file=sys.stderr)
    if result.returncode:
        raise SystemExit(f"[P4][FAIL] typed registry export: rc={result.returncode}")
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise SystemExit(f"[P4][FAIL] typed registry export returned invalid JSON: {error}") from error
    if payload.get("schema") != REGISTRY_SCHEMA:
        raise SystemExit(
            f"[P4][FAIL] registry schema mismatch got={payload.get('schema')!r} expected={REGISTRY_SCHEMA!r}"
        )
    if payload.get("version") != REGISTRY_VERSION:
        raise SystemExit(
            f"[P4][FAIL] registry version mismatch got={payload.get('version')!r} expected={REGISTRY_VERSION}"
        )
    specs = payload.get("specs")
    if not isinstance(specs, list) or not specs:
        raise SystemExit("[P4][FAIL] typed registry contains no specs")
    return specs


def render_arg(value: str, values: dict[str, str]) -> str:
    rendered = value
    for placeholder, replacement in values.items():
        rendered = rendered.replace("{" + placeholder + "}", replacement)
    if "{" in rendered or "}" in rendered:
        raise SystemExit(f"[P4][FAIL] unresolved command placeholder in {value!r} -> {rendered!r}")
    return rendered


def prepare_fixture(testdata: Path, temp: Path, spec: dict[str, Any]) -> Path:
    fixture = spec["fixture"]
    source = temp / Path(fixture["source_relative"])
    kind = fixture["kind"]
    if kind == "file":
        testdata_name = fixture.get("testdata_name")
        if not testdata_name:
            raise SystemExit(f"[P4][FAIL] spec={spec['id']} file fixture has no testdata_name")
        fixture_path = testdata / testdata_name
        if not fixture_path.is_file():
            raise SystemExit(f"[P4][FAIL] missing fixture spec={spec['id']} path={fixture_path}")
        source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(fixture_path, source)
    elif kind == "generated_directory":
        source.mkdir(parents=True, exist_ok=True)
    else:
        raise SystemExit(f"[P4][FAIL] spec={spec['id']} unknown fixture kind={kind!r}")
    return source


def rust_check(neocore: Path, output: Path, spec_id: str) -> None:
    run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "newengine-contract-conformance",
            "--example",
            "check_list_file",
            "--",
            str(output),
            spec_id,
        ],
        neocore,
        label=f"{spec_id}: canonical runtime contract",
        env=cargo_env(neocore),
    )


def conformance_workspace(repo: Path, neocore: Path, workspace: str) -> Path:
    roots = {
        "neocore": neocore,
        "asset_manager": repo / "PluginsSrc" / "AssetManager",
    }
    try:
        return roots[workspace]
    except KeyError as error:
        raise SystemExit(f"[P4][FAIL] unsupported conformance workspace={workspace!r}") from error


def dto_parity_check(
    repo: Path,
    neocore: Path,
    temp: Path,
    output: Path,
    spec: dict[str, Any],
) -> None:
    asset_manager = spec.get("asset_manager_decode")
    runtime = spec.get("runtime_decode")
    projection = spec.get("canonical_projection")
    if asset_manager is None and runtime is None and projection is None:
        return
    if not isinstance(asset_manager, dict) or not isinstance(runtime, dict) or not projection:
        raise SystemExit(
            f"[P4][FAIL] spec={spec['id']} incomplete DTO parity declaration"
        )

    spec_id = str(spec["id"])
    logical_path = Path(str(spec["output_relative"])).as_posix()
    am_cmd = [
        "cargo", "run", "-q",
        "-p", str(asset_manager["package"]),
        "--example", str(asset_manager["example"]),
        "--", str(output), logical_path, str(asset_manager["output_kind"]),
    ]
    runtime_cmd = [
        "cargo", "run", "-q",
        "-p", str(runtime["package"]),
        "--example", str(runtime["example"]),
        "--", str(output), logical_path,
    ]
    am_workspace = conformance_workspace(repo, neocore, str(asset_manager["workspace"]))
    runtime_workspace = conformance_workspace(repo, neocore, str(runtime["workspace"]))
    am_json = run_capture_json(
        am_cmd,
        am_workspace,
        label=f"{spec_id}: AssetManager native DTO",
        env=cargo_env(am_workspace),
    )
    runtime_json = run_capture_json(
        runtime_cmd,
        runtime_workspace,
        label=f"{spec_id}: domain runtime native DTO",
        env=cargo_env(runtime_workspace),
    )

    am_path = temp / "Conformance" / f"{spec_id}.asset_manager.json"
    runtime_path = temp / "Conformance" / f"{spec_id}.runtime.json"
    am_path.parent.mkdir(parents=True, exist_ok=True)
    am_path.write_bytes(am_json)
    runtime_path.write_bytes(runtime_json)
    run(
        [
            "cargo", "run", "-q",
            "-p", "newengine-contract-conformance",
            "--example", "compare_native_dto",
            "--", spec_id, logical_path, str(am_path), str(runtime_path),
        ],
        neocore,
        label=f"{spec_id}: canonical DTO parity ({projection})",
        env=cargo_env(neocore),
    )


def validate_authored_schema_semantics(source: Path, spec: dict[str, Any]) -> None:
    authored = spec.get("authored_schema")
    if not authored:
        return
    if not source.is_file():
        raise SystemExit(
            f"[P4][FAIL] spec={spec['id']} authored schema requires file source: {source}"
        )
    attribute = str(authored.get("declaration_attribute") or "").strip()
    schema_id = str(authored.get("schema_id") or "").strip()
    contract_key = str(authored.get("contract_key") or "").strip()
    if not attribute or not schema_id or not contract_key:
        raise SystemExit(
            f"[P4][FAIL] spec={spec['id']} authored schema semantic is incomplete: {authored!r}"
        )
    try:
        document = ET.parse(source)
    except ET.ParseError as error:
        raise SystemExit(
            f"[P4][FAIL] spec={spec['id']} authored source is not valid XML: {error}"
        ) from error
    actual = (document.getroot().attrib.get(attribute) or "").strip()
    if actual != schema_id:
        raise SystemExit(
            f"[P4][FAIL] spec={spec['id']} authored schema mismatch "
            f"contract={contract_key!r} attribute={attribute!r} got={actual!r} expected={schema_id!r}"
        )
    print(
        f"[P4][PASS] spec={spec['id']} authored-schema contract={contract_key} "
        f"{attribute}={schema_id}"
    )


def execute_spec(
    repo: Path,
    neocore: Path,
    tools: dict[str, Path],
    testdata: Path,
    spec: dict[str, Any],
) -> None:
    spec_id = str(spec["id"])
    tool_key = str(spec["tool_key"])
    tool = tools.get(tool_key)
    if tool is None or not tool.is_file():
        raise SystemExit(f"[P4][FAIL] spec={spec_id} missing installed tool key={tool_key!r} path={tool}")

    with tempfile.TemporaryDirectory(prefix=f"northstar-p4-{spec_id}-") as raw_temp:
        temp = Path(raw_temp)
        source = prepare_fixture(testdata, temp, spec)
        validate_authored_schema_semantics(source, spec)
        output = temp / Path(spec["output_relative"])
        output.parent.mkdir(parents=True, exist_ok=True)
        values = {
            "root": str(temp),
            "source": str(source),
            "source_rel": str(source.relative_to(temp)),
            "output": str(output),
            "output_rel": str(output.relative_to(temp)),
        }

        for index, command in enumerate(spec["commands"], start=1):
            phase = str(command["phase"])
            args = [render_arg(str(arg), values) for arg in command["args"]]
            run(
                [str(tool), *args],
                temp,
                label=f"{spec_id}:{phase}:{index}",
            )
            if phase == "produce" and not output.is_file():
                raise SystemExit(
                    f"[P4][FAIL] spec={spec_id} producer did not create expected output: {output}"
                )

        if not output.is_file():
            raise SystemExit(f"[P4][FAIL] spec={spec_id} expected output missing: {output}")
        rust_check(neocore, output, spec_id)
        dto_parity_check(repo, neocore, temp, output, spec)
    print(
        f"[P4][PASS] spec={spec_id} tool={tool_key} contract={spec['schema_contract_key']} "
        f"content_kind={spec['content_kind']}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="NorthStar P4 declarative tool/runtime conformance matrix executor"
    )
    parser.add_argument(
        "--spec",
        action="append",
        default=[],
        help="run only the named ToolRuntimeConformanceSpec id; may be repeated",
    )
    ns = parser.parse_args()

    neocore = Path(__file__).resolve().parents[1]
    repo = neocore.parent.parent
    maintenance = repo / "tools" / "maintenance"
    sys.path.insert(0, str(maintenance))
    import northstar_native_assets as native_assets

    suite = native_assets.suite_root(repo)
    tools = native_assets.tool_paths(repo, suite)
    testdata = neocore / "crates" / "newengine-contract-conformance" / "testdata"
    specs = load_registry(neocore)

    requested = set(ns.spec)
    if requested:
        known = {str(spec["id"]) for spec in specs}
        unknown = sorted(requested - known)
        if unknown:
            raise SystemExit(f"[P4][FAIL] unknown spec id(s): {', '.join(unknown)}")
        specs = [spec for spec in specs if str(spec["id"]) in requested]

    for spec in specs:
        execute_spec(repo, neocore, tools, testdata, spec)
    print(f"[P4] TOOL/RUNTIME CONFORMANCE MATRIX PASS specs={len(specs)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
