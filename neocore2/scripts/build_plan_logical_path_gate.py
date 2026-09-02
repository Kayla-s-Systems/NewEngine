#!/usr/bin/env python3
"""Reject repository build-plan entries that rely on runtime path inference."""

from __future__ import annotations

import json
from collections import defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
PLAN_PATHS = sorted((REPO_ROOT / "Projects").rglob("asset.build.json")) + [
    REPO_ROOT / "Shared" / "asset.build.json"
]


def iter_build_entries(value, path="root"):
    if isinstance(value, dict):
        if "output" in value and any(
            key in value for key in ("source", "source_dictionary", "source_dir")
        ):
            yield path, value
        for key, child in value.items():
            yield from iter_build_entries(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from iter_build_entries(child, f"{path}[{index}]")


def main() -> int:
    failures: list[str] = []
    checked_entries = 0
    for plan_path in PLAN_PATHS:
        if not plan_path.is_file():
            failures.append(f"missing build plan: {plan_path}")
            continue
        try:
            document = json.loads(plan_path.read_text(encoding="utf-8"))
        except Exception as error:
            failures.append(f"cannot parse {plan_path}: {error}")
            continue

        owners: dict[str, list[tuple[str, str]]] = defaultdict(list)
        for entry_path, entry in iter_build_entries(document):
            checked_entries += 1
            logical_path = str(entry.get("logical_path", "")).strip().replace("\\", "/")
            source = str(
                entry.get("source_dictionary")
                or entry.get("source")
                or entry.get("source_dir")
                or ""
            ).strip()
            if not logical_path:
                failures.append(
                    f"{plan_path}:{entry_path}: source='{source}' output='{entry.get('output')}' "
                    "must author explicit logical_path"
                )
                continue
            if logical_path.startswith("/") or ".." in logical_path.split("/"):
                failures.append(
                    f"{plan_path}:{entry_path}: unsafe logical_path='{logical_path}'"
                )
                continue
            owners[logical_path.casefold()].append((entry_path, source))

        for logical_path, entries in owners.items():
            distinct_sources = {source.casefold() for _, source in entries}
            if len(distinct_sources) > 1:
                failures.append(
                    f"{plan_path}: logical_path collision '{logical_path}' owners={entries}"
                )

    if failures:
        print("build-plan logical-path gate: FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        "build-plan logical-path gate: PASS "
        f"plans={len(PLAN_PATHS)} entries={checked_entries} explicit_logical_path=true"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
