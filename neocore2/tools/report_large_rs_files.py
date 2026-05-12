#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
import argparse


def main() -> int:
    parser = argparse.ArgumentParser(description="Report Rust files above a line-count threshold.")
    parser.add_argument("root", nargs="?", default=".")
    parser.add_argument("--limit", type=int, default=400)
    args = parser.parse_args()

    root = Path(args.root)
    rows: list[tuple[int, Path]] = []
    for path in root.rglob("*.rs"):
        if any(part in {"target", ".git"} for part in path.parts):
            continue
        try:
            lines = len(path.read_text(encoding="utf-8", errors="ignore").splitlines())
        except OSError:
            continue
        if lines > args.limit:
            rows.append((lines, path))

    for lines, path in sorted(rows, reverse=True):
        print(f"{lines:5} {path}")
    print(f"total={len(rows)} limit={args.limit}")
    return 1 if rows else 0


if __name__ == "__main__":
    raise SystemExit(main())
