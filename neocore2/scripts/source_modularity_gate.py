#!/usr/bin/env python3
"""Progressive source-modularity gate for North Star.

The gate is intentionally policy-driven. It scans production Rust across the North Star workspace,
enforces explicit regression budgets, and applies a repository-wide line target. Historical files
that are already above the target must be registered explicitly as debt allowances. A debt
allowance is a ratchet, not an exemption: the file may not grow beyond its recorded budget, while
any new unmanaged file above the global target fails immediately.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

SCHEMA_VERSION = 2
POLICY_DEFAULT = "config/architecture/source-modularity.v1.json"
POLICY_KEYS = {
    "schema_version",
    "rust_roots",
    "production_line_target",
    "enforce_global_target",
    "exclude_globs",
    "enforced_budgets",
    "debt_allowances",
}


@dataclass(frozen=True)
class Policy:
    rust_roots: tuple[str, ...]
    production_line_target: int
    enforce_global_target: bool
    exclude_globs: tuple[str, ...]
    enforced_budgets: dict[str, int]
    debt_allowances: dict[str, int]


def _positive_int(value: object, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value <= 0:
        raise ValueError(f"{field} must be a positive integer")
    return value


def _path_budget_map(value: object, field: str) -> dict[str, int]:
    if not isinstance(value, dict):
        raise ValueError(f"{field} must be an object")

    budgets: dict[str, int] = {}
    for raw_path, raw_budget in value.items():
        if not isinstance(raw_path, str) or not raw_path:
            raise ValueError(f"{field} keys must be non-empty paths")
        normalized = Path(raw_path).as_posix()
        if normalized.startswith("../") or normalized == "..":
            raise ValueError(f"{field} path escapes the workspace: {raw_path}")
        budgets[normalized] = _positive_int(raw_budget, f"{field}[{raw_path}]")
    return budgets


def load_policy(path: Path) -> Policy:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise ValueError(f"cannot read policy '{path}': {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ValueError(f"cannot parse policy '{path}': {exc}") from exc

    if not isinstance(value, dict):
        raise ValueError("source modularity policy root must be an object")
    unknown = sorted(set(value) - POLICY_KEYS)
    if unknown:
        raise ValueError(f"unknown source modularity policy keys: {', '.join(unknown)}")
    if value.get("schema_version") != SCHEMA_VERSION:
        raise ValueError(
            f"unsupported source modularity schema_version={value.get('schema_version')!r}; "
            f"expected {SCHEMA_VERSION}"
        )

    roots = value.get("rust_roots")
    if not isinstance(roots, list) or not roots or not all(isinstance(item, str) and item for item in roots):
        raise ValueError("rust_roots must be a non-empty string array")
    normalized_roots = tuple(Path(item).as_posix() for item in roots)
    if any(root.startswith("../") or root == ".." for root in normalized_roots):
        raise ValueError("rust_roots must be workspace-relative and may not escape the workspace")

    excludes = value.get("exclude_globs", [])
    if not isinstance(excludes, list) or not all(isinstance(item, str) and item for item in excludes):
        raise ValueError("exclude_globs must be a string array")
    enforce_global_target = value.get("enforce_global_target", False)
    if not isinstance(enforce_global_target, bool):
        raise ValueError("enforce_global_target must be a boolean")

    production_line_target = _positive_int(
        value.get("production_line_target"), "production_line_target"
    )
    enforced_budgets = _path_budget_map(value.get("enforced_budgets", {}), "enforced_budgets")
    debt_allowances = _path_budget_map(value.get("debt_allowances", {}), "debt_allowances")
    invalid_debt = sorted(
        path for path, budget in debt_allowances.items() if budget <= production_line_target
    )
    if invalid_debt:
        raise ValueError(
            "debt_allowances must be strictly above production_line_target; invalid: "
            + ", ".join(invalid_debt)
        )

    return Policy(
        rust_roots=normalized_roots,
        production_line_target=production_line_target,
        enforce_global_target=enforce_global_target,
        exclude_globs=tuple(excludes),
        enforced_budgets=enforced_budgets,
        debt_allowances=debt_allowances,
    )


def excluded(relative_path: str, patterns: Iterable[str]) -> bool:
    return any(fnmatch.fnmatch(relative_path, pattern) for pattern in patterns)


def line_count(path: Path) -> int:
    # splitlines() is independent of LF/CRLF checkout policy and does not treat a trailing newline
    # as an extra source line. Inline test modules remain part of the file-size signal deliberately:
    # colocating a large test suite with production code is itself source-modularity debt.
    return len(path.read_text(encoding="utf-8", errors="replace").splitlines())


def scan(workspace_root: Path, policy: Policy) -> dict[str, int]:
    results: dict[str, int] = {}
    for raw_root in policy.rust_roots:
        root = workspace_root / raw_root
        if not root.is_dir():
            raise ValueError(f"configured rust root does not exist: {raw_root}")
        for path in root.rglob("*.rs"):
            relative = path.relative_to(workspace_root).as_posix()
            if excluded(relative, policy.exclude_globs):
                continue
            results[relative] = line_count(path)
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--policy", default=POLICY_DEFAULT)
    parser.add_argument(
        "--report-only",
        action="store_true",
        help="report budget violations but return success; useful while inspecting a branch",
    )
    parser.add_argument("--top", type=int, default=20, help="maximum backlog rows to print")
    args = parser.parse_args()

    neocore_root = Path(__file__).resolve().parents[1]
    workspace_root = Path(__file__).resolve().parents[3]
    policy_path = (neocore_root / args.policy).resolve()
    try:
        policy = load_policy(policy_path)
        results = scan(workspace_root, policy)
    except ValueError as exc:
        print(f"source-modularity: POLICY ERROR: {exc}", file=sys.stderr)
        return 2

    budget_violations: list[tuple[str, int, int]] = []
    for path, budget in sorted(policy.enforced_budgets.items()):
        actual = results.get(path)
        if actual is None:
            budget_violations.append((path, -1, budget))
        elif actual > budget:
            budget_violations.append((path, actual, budget))

    debt_regressions: list[tuple[str, int, int]] = []
    retired_debt: list[tuple[str, int, int]] = []
    for path, allowance in sorted(policy.debt_allowances.items()):
        actual = results.get(path)
        if actual is None:
            debt_regressions.append((path, -1, allowance))
        elif actual > allowance:
            debt_regressions.append((path, actual, allowance))
        elif actual <= policy.production_line_target:
            retired_debt.append((path, actual, allowance))

    backlog = sorted(
        (
            (lines, path)
            for path, lines in results.items()
            if lines > policy.production_line_target
        ),
        reverse=True,
    )
    unmanaged_backlog = [
        (lines, path) for lines, path in backlog if path not in policy.debt_allowances
    ]
    managed_backlog = [
        (lines, path) for lines, path in backlog if path in policy.debt_allowances
    ]

    violation_count = len(budget_violations) + len(debt_regressions)
    if policy.enforce_global_target:
        violation_count += len(unmanaged_backlog)

    print(
        "source-modularity: "
        f"scanned={len(results)} target={policy.production_line_target} "
        f"global_enforced={policy.enforce_global_target} "
        f"enforced={len(policy.enforced_budgets)} debt_allowances={len(policy.debt_allowances)} "
        f"violations={violation_count} over_target={len(backlog)} "
        f"managed_debt={len(managed_backlog)} unmanaged_over_target={len(unmanaged_backlog)}"
    )

    for path, actual, budget in budget_violations:
        actual_text = "missing" if actual < 0 else str(actual)
        print(f"VIOLATION budget {path}: lines={actual_text} budget={budget}")
    for path, actual, allowance in debt_regressions:
        actual_text = "missing" if actual < 0 else str(actual)
        print(f"VIOLATION debt-regression {path}: lines={actual_text} allowance={allowance}")
    if policy.enforce_global_target:
        for lines, path in unmanaged_backlog:
            print(
                f"VIOLATION unmanaged-over-target {path}: "
                f"lines={lines} target={policy.production_line_target}"
            )

    for path, actual, allowance in retired_debt:
        print(
            f"RETIRE debt-allowance {path}: lines={actual} "
            f"target={policy.production_line_target} old_allowance={allowance}"
        )

    if backlog:
        print("over-target source files (largest first):")
        for lines, path in backlog[: max(0, args.top)]:
            debt = policy.debt_allowances.get(path)
            suffix = f" debt_allowance={debt}" if debt is not None else " UNMANAGED"
            print(f"  {lines:5}  {path}{suffix}")

    should_fail = bool(budget_violations or debt_regressions)
    if policy.enforce_global_target and unmanaged_backlog:
        should_fail = True
    if not args.report_only and should_fail:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
