#!/usr/bin/env python3
"""Compatibility entrypoint retained for the closed P3 gate.

Tool/runtime producer knowledge is owned by the typed P4 registry in
`newengine-contract-conformance`; execution lives in p4_tool_runtime_conformance.py.
"""
from p4_tool_runtime_conformance import main

if __name__ == "__main__":
    raise SystemExit(main())
