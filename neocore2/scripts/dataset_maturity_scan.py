#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]
TOOLS_ROOT = REPO_ROOT / "tools" / "scripts"
if str(TOOLS_ROOT) not in sys.path:
    sys.path.insert(0, str(TOOLS_ROOT))

from northstar_bridge.contracts import BridgeContext, BridgeError  # noqa: E402
from northstar_bridge import dataset  # noqa: E402


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Index-first dataSet maturity scanner for North Star Engine.")
    parser.add_argument("--write", action="store_true", help="Write JSON/MD reports to .takesome/dataSet/index and docs/audits.")
    parser.add_argument("--strict", action="store_true", help="Fail on missing gateway/capability/NullProvider/conformance or visible gaps.")
    parser.add_argument("--limit", type=int, default=5000)
    parser.add_argument("--max-files", type=int, default=30000)
    args = parser.parse_args(argv)

    ctx = BridgeContext(root=REPO_ROOT, write_enabled=args.write, python_cmd=[sys.executable], interactive=False)
    try:
        if args.write:
            write_result = dataset.write_maturity_index(ctx, {"limit": args.limit, "max_files": args.max_files})
            print(json.dumps(write_result, ensure_ascii=False, indent=2))
            scan_path = REPO_ROOT / str(write_result.get("scan_path", ""))
            scan = json.loads(scan_path.read_text(encoding="utf-8")) if scan_path.exists() else dataset.maturity_scan(ctx, {"limit": args.limit, "max_files": args.max_files})
        else:
            scan = dataset.maturity_scan(ctx, {"limit": args.limit, "max_files": args.max_files})
            print(json.dumps(scan, ensure_ascii=False, indent=2))
        findings = dataset.strict_findings(scan)
        if args.strict and findings:
            for finding in findings:
                print("[ERROR] " + json.dumps(finding, ensure_ascii=False), file=sys.stderr)
            return 1
        print(f"[OK] dataset maturity scan completed records={len(scan.get('module_completeness_matrix') or [])} strict_findings={len(findings)}")
        return 0
    except BridgeError as exc:
        print(f"[ERROR] {exc} code={exc.code} data={json.dumps(exc.data, ensure_ascii=False)}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
