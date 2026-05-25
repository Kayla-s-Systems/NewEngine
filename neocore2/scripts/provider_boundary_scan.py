#!/usr/bin/env python3
"""Reject direct provider/backend coupling from reusable runtime crates.

Rule:
  Engine owns the question.
  Provider owns the answer.
  Runtime calls engine.* gateways and capability registry only.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
PROTECTED_ROOTS = [
    ROOT / "crates" / "newengine-engine-runtime",
    ROOT / "crates" / "newengine-runtime-host",
    ROOT / "crates" / "newengine-core",
    ROOT / "crates" / "newengine-scene",
    ROOT / "crates" / "newengine-model-runtime",
    ROOT / "crates" / "newengine-material-runtime",
    ROOT / "crates" / "newengine-assets-ui-runtime",
]

# Provider/plugin/backend implementation names must not appear as code-level
# dependencies or control flow in reusable runtime. Comments are not fatal unless
# they carry concrete API calls; docs are intentionally skipped by this scanner.
DENY_PATTERNS = [
    (re.compile(r"\bAureliaUiProvider\b|\baurelia_ui_provider\b|\bnewengine-ui-provider-aurelia\b"), "direct Aurelia provider reference"),
    (re.compile(r"\bVulkanRenderer\b|\bvulkan_renderer\b|\bnewengine\.renderer\.vulkan\b"), "direct Vulkan renderer reference"),
    (re.compile(r"\bJoltPhysics\b|\bJoltPacket\w*\b|\bjoltc_sys\b|\bnewengine-physics-jolt\b"), "direct Jolt physics reference"),
    (re.compile(r"\bWgpuRenderer\b|\bwgpu_renderer\b"), "direct WGPU renderer reference"),
    (re.compile(r"\bif\s+.*provider\s*=="), "provider-name branch"),
    (re.compile(r"\bif\s+.*backend\s*=="), "backend-name branch"),
    (re.compile(r"\bui_provider\s*==|\bprovider_id\s*==|\bbackend_id\s*=="), "provider/backend id comparison"),
]

# Runtime crates may use plugin-host snapshots for diagnostics, but active route
# checks must go through newengine_core gateway helpers.
DENY_RUNTIME_HOST_CALLS = [
    (re.compile(r"newengine_plugin_host::has_service\s*\("), "direct service presence check; use newengine_core::has_engine_gateway_route"),
    (re.compile(r"newengine_plugin_host::engine_gateway_has_capability\s*\("), "direct capability check; use newengine_core::engine_gateway_has_capability"),
    (re.compile(r"newengine_plugin_host::resolve_service_for_engine_gateway\s*\("), "direct gateway resolution; use newengine_core::resolve_service_for_engine_gateway"),
]

DENY_ENGINE_RUNTIME_UI_IMPL = [
    (re.compile(r"newengine_ui::UiInputFrame|newengine_ui::draw::UiDrawList"), "engine runtime must use newengine-ui-api DTOs, not newengine-ui implementation exports"),
]

SKIP_SUFFIXES = {".md", ".txt", ".png", ".jpg", ".jpeg", ".ico", ".ytd", ".neui"}


def iter_files() -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for root in PROTECTED_ROOTS:
        if root.exists():
            for path in root.rglob("*"):
                if path.is_file() and path.suffix not in SKIP_SUFFIXES:
                    files.append(path)
    return files


def is_comment(line: str) -> bool:
    stripped = line.strip()
    return stripped.startswith("//") or stripped.startswith("#") or stripped.startswith("*")


def main() -> int:
    violations: list[str] = []
    for path in iter_files():
        rel = path.relative_to(ROOT)
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        patterns = list(DENY_PATTERNS)
        if "newengine-engine-runtime" in path.parts or "newengine-runtime-host" in path.parts:
            patterns += DENY_RUNTIME_HOST_CALLS + DENY_ENGINE_RUNTIME_UI_IMPL
        for idx, line in enumerate(lines, start=1):
            # comments may mention backend names for documentation, but not direct calls/imports.
            effective = line if not is_comment(line) else ""
            for pattern, reason in patterns:
                if pattern.search(effective):
                    violations.append(f"{rel}:{idx}: {reason}: {line.strip()}")

    if violations:
        print("provider-boundary scan failed:")
        for v in violations:
            print(f"  {v}")
        print("\nUse API domains, engine.* gateways and capability registry. Never call provider/backend implementations from reusable runtime.")
        return 1
    print("provider-boundary scan passed: reusable runtime has no direct provider/backend coupling.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
