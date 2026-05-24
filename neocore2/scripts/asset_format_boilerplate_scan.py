#!/usr/bin/env python3
"""Deny duplicated NEF8/ListFile descriptor boilerplate in format crates."""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
FORMAT_RE = re.compile(r"newengine-asset-format-(?!common$|catalog-nef8$|nepak$).+")
DENY_PATTERNS = [
    re.compile(r"=\s*AssetFileTypeDescriptor\s*\{"),
    re.compile(r"codec_type::LIST_FILE"),
    re.compile(r"ASSET_LIST_FILE_MANIFEST_OUTPUT"),
    re.compile(r"ASSET_LIST_FILE_HEADER_OUTPUT"),
    re.compile(r"ASSET_LIST_FILE_BODY_OUTPUT"),
    re.compile(r"native_container:\s*true"),
    re.compile(r"requires_magic:\s*true"),
]
REQUIRED = [
    "ListFileFormatDescriptorBuilder::new",
    "pub fn register_format()",
    "pub fn file_type_descriptor()",
    "pub const EXTENSION",
    "pub const CONTENT_KIND",
    "pub const ASSET_KIND",
    "pub const SEMANTIC_GATEWAY",
    "pub const PURPOSE",
]


def main() -> int:
    violations: list[str] = []
    for crate_dir in sorted(CRATES.glob("newengine-asset-format-*")):
        if not FORMAT_RE.fullmatch(crate_dir.name):
            continue
        lib = crate_dir / "src" / "lib.rs"
        if not lib.exists():
            continue
        text = lib.read_text(encoding="utf-8", errors="replace")
        for required in REQUIRED:
            if required not in text:
                violations.append(f"{lib.relative_to(ROOT)}: missing required self-declared builder surface `{required}`")
        for pattern in DENY_PATTERNS:
            if pattern.search(text):
                violations.append(
                    f"{lib.relative_to(ROOT)}: duplicated ListFile descriptor boilerplate matched `{pattern.pattern}`"
                )

    if violations:
        print("asset-format-boilerplate scan failed:")
        for item in violations:
            print(f"  {item}")
        print("\nMove shared NEF8/ListFile descriptor mechanics into newengine-asset-format-common.")
        return 1

    print("asset-format-boilerplate scan passed: format crates declare identity through the common builder.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
