#!/usr/bin/env python3
"""Deny reintroducing one-crate-per-extension NEF8/ListFile descriptors."""
from __future__ import annotations

import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
ALLOWED = {"newengine-asset-format-nef8"}


def main() -> int:
    violations: list[str] = []
    for crate_dir in sorted(CRATES.glob("newengine-asset-format-*")):
        if crate_dir.name not in ALLOWED:
            violations.append(
                f"{crate_dir.relative_to(ROOT)}: per-extension format crate is forbidden; declare the format in newengine-asset-format-nef8"
            )

    registry = CRATES / "newengine-asset-format-nef8" / "src" / "lib.rs"
    if not registry.exists():
        violations.append("crates/newengine-asset-format-nef8/src/lib.rs: unified NEF8 registry is missing")
    else:
        text = registry.read_text(encoding="utf-8", errors="replace")
        required = ["pub struct Nef8FormatSpec", "pub fn descriptors()", "pub fn descriptor_for_extension", "pub mod ytd", "pub mod ytyp", "pub mod nemat"]
        for token in required:
            if token not in text:
                violations.append(f"{registry.relative_to(ROOT)}: missing registry surface `{token}`")

    if violations:
        print("asset-format-boilerplate scan failed:")
        for item in violations:
            print(f"  {item}")
        print("\nKeep NEF8/ListFile format identity in one data registry: newengine-asset-format-nef8.")
        return 1

    print("asset-format-boilerplate scan passed: NEF8/ListFile formats are centralized in newengine-asset-format-nef8.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
