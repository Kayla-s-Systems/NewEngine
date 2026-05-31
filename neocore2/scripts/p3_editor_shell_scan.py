#!/usr/bin/env python3
"""P3 editor shell source gate for North Star Engine.

This verifies that the editor shell is expressed as provider-neutral UI node
composition (`engine.ui` / `UiNodeTreeRequest`) and that the major editor panels
are declared as `.neui`-backed data surfaces instead of concrete provider
branches.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys
from dataclasses import dataclass

SCRIPT_ROOT = pathlib.Path(__file__).resolve().parent
ENGINE_ROOT = SCRIPT_ROOT.parent
REPO_ROOT = ENGINE_ROOT.parents[1]

SCREEN_PROFILE = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "screen_profile.rs"
SCREEN_PROFILE_PARTS = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "screen_profile_parts"
UI_GATEWAY_FRAME = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "ui_gateway_frame.rs"
UI_GATEWAY_FRAME_PARTS = ENGINE_ROOT / "crates" / "newengine-runtime-host" / "src" / "platform_runtime" / "ui_gateway_frame_parts"
UI_NODE_API = ENGINE_ROOT / "crates" / "newengine-ui-api" / "src" / "node.rs"
ASSET_DOCUMENT_API = ENGINE_ROOT / "crates" / "newengine-assets-api" / "src" / "asset_document.rs"
EDITOR_CHROME_IMPORT = ENGINE_ROOT / "assets" / "ui" / "editor" / "editor_chrome.neui.import.json"
VALIDATION_TOOL = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "validation.py"
TOOLS_INIT = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "__init__.py"
TOOLS_INVARIANTS = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "invariants.py"
TOOLS_RUN = REPO_ROOT / "tools" / "scripts" / "takesome" / "tools" / "run.py"
CLI = REPO_ROOT / "tools" / "scripts" / "takesome" / "cli.py"
SUITE_REGISTRY = REPO_ROOT / "tools" / "scripts" / "takesome" / "suite" / "registry.py"

REQUIRED_PANEL_IDS = {
    "left.scene_tree": "Scene Tree",
    "right.inspector": "Inspector",
    "bottom.asset_browser": "Asset Browser",
    "bottom.import_queue": "Import Queue",
    "bottom.output_log": "Output Log",
    "bottom.profiler_diagnostics": "Profiler / Diagnostics",
    "center.viewport_gizmos": "Viewport Gizmos",
}

REQUIRED_NEUI_REFS = {
    "assets/ui/editor/editor_shell.neui@surface",
    "assets/ui/editor/scene_tree.neui@surface",
    "assets/ui/editor/inspector.neui@surface",
    "assets/ui/editor/content_browser.neui@editor.asset_browser",
    "assets/ui/editor/import_queue.neui@surface",
    "assets/ui/editor/output_log.neui@surface",
    "assets/ui/editor/profiler_diagnostics.neui@surface",
    "assets/ui/editor/viewport_gizmos.neui@surface",
}

REQUIRED_IMPORT_DESCRIPTORS = {
    "scene_tree.neui.import.json",
    "import_queue.neui.import.json",
    "output_log.neui.import.json",
    "profiler_diagnostics.neui.import.json",
    "viewport_gizmos.neui.import.json",
}


@dataclass(frozen=True)
class Finding:
    severity: str
    check: str
    path: pathlib.Path
    message: str
    excerpt: str = ""

    def render(self) -> str:
        try:
            rel = self.path.relative_to(REPO_ROOT)
        except Exception:
            rel = self.path
        suffix = f": {self.excerpt.strip()}" if self.excerpt.strip() else ""
        return f"[{self.severity}] {self.check}: {rel}: {self.message}{suffix}"


def read(path: pathlib.Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def read_split_source_unit(root_file: pathlib.Path, parts_dir: pathlib.Path) -> str:
    chunks = [read(root_file)]
    if parts_dir.exists():
        for part in sorted(parts_dir.glob("*.rs")):
            chunks.append(f"\n// include: {part.name}\n")
            chunks.append(read(part))
    return "\n".join(chunks)


def read_screen_profile_source_unit() -> str:
    """Read screen_profile.rs plus same-scope include! parts as one Rust source unit.

    The implementation intentionally keeps screen_profile.rs as a small ownership
    index and moves the heavy editor shell into include! parts. The P3 gate must
    validate the compiled source shape, not accidentally punish a clean module
    split.
    """

    return read_split_source_unit(SCREEN_PROFILE, SCREEN_PROFILE_PARTS)


def read_ui_gateway_frame_source_unit() -> str:
    return read_split_source_unit(UI_GATEWAY_FRAME, UI_GATEWAY_FRAME_PARTS)


def load_json(path: pathlib.Path) -> tuple[dict, list[Finding]]:
    if not path.exists():
        return {}, [Finding("ERROR", "p3-editor-shell", path, "required JSON file is missing")]
    try:
        return json.loads(path.read_text(encoding="utf-8")), []
    except json.JSONDecodeError as exc:
        return {}, [Finding("ERROR", "p3-editor-shell", path, f"invalid JSON: {exc}")]


def scan_build_regression_fix() -> list[Finding]:
    text = read(ASSET_DOCUMENT_API)
    out: list[Finding] = []
    if "impl Default for AssetDocumentRequest" not in text:
        out.append(Finding("ERROR", "p3-build-regression", ASSET_DOCUMENT_API, "AssetDocumentRequest default impl missing"))
    default_block = text[text.find("impl Default for AssetDocumentRequest"):text.find("impl AssetDocumentRequest", text.find("impl Default for AssetDocumentRequest"))]
    for token in ("schema_patch: None", "transaction: None"):
        if token not in default_block:
            out.append(Finding("ERROR", "p3-build-regression", ASSET_DOCUMENT_API, "AssetDocumentRequest::default must initialize P2 schema fields", token))
    return out


def scan_ui_transport() -> list[Finding]:
    out: list[Finding] = []
    screen = read_screen_profile_source_unit()
    gateway = read_ui_gateway_frame_source_unit()
    api = read(UI_NODE_API)
    for token in ("UiNodeTreeRequest", "UiNodeRequestSourceKind::Generated", "publish_screen_node_tree_request", "from_surface_node"):
        if token not in screen:
            out.append(Finding("ERROR", "editor-ui-transport", SCREEN_PROFILE, "editor shell must publish generated UiNodeTreeRequest through engine.ui", token))
    for token in ("UI_SERVICE_METHOD_APPLY_NODE_REQUEST_V1", "publish_node_tree_request", "UiNodeRequestAck"):
        if token not in gateway:
            out.append(Finding("ERROR", "editor-ui-transport", UI_GATEWAY_FRAME, "runtime-host must route node trees through engine.ui apply_node_request", token))
    for token in ("surface_style: Option<UiSurfaceStyle>", "admission_policy: Option<UiSurfaceAdmissionPolicy>", "from_surface_node", "from_component_node", "from_component_id"):
        if token not in api:
            out.append(Finding("ERROR", "editor-ui-transport", UI_NODE_API, "UiNodeTreeRequest must preserve generated shell surface/style data", token))
    if "UI_SERVICE_METHOD_SURFACE_NODE_V1" in screen and "hide_profile_surface" not in screen:
        out.append(Finding("WARN", "editor-ui-transport", SCREEN_PROFILE, "surface-node direct publish should only remain for hiding/unmount compatibility"))
    return out


def scan_panels() -> list[Finding]:
    out: list[Finding] = []
    screen = read_screen_profile_source_unit()
    for panel_id, label in REQUIRED_PANEL_IDS.items():
        if panel_id not in screen:
            out.append(Finding("ERROR", "editor-panels", SCREEN_PROFILE, "missing required editor panel id", f"{panel_id} ({label})"))
        if label not in screen:
            out.append(Finding("ERROR", "editor-panels", SCREEN_PROFILE, "missing required editor panel label", label))
    for ref in REQUIRED_NEUI_REFS:
        if ref not in screen and ref not in read(EDITOR_CHROME_IMPORT):
            out.append(Finding("ERROR", "editor-neui", SCREEN_PROFILE, "missing .neui-backed panel ref", ref))
    for token in ("engine.schema", "schema-property", "AssetDocument DTO", "UiViewportSlot", "selection DTO"):
        if token not in screen:
            out.append(Finding("ERROR", "editor-panels", SCREEN_PROFILE, "editor panels must describe DTO/schema/gizmo boundaries", token))
    return out


def scan_neui_descriptors() -> list[Finding]:
    out: list[Finding] = []
    data, findings = load_json(EDITOR_CHROME_IMPORT)
    out.extend(findings)
    if not findings:
        if data.get("composition_role") != "editor_shell":
            out.append(Finding("ERROR", "editor-neui", EDITOR_CHROME_IMPORT, "editor chrome import must declare editor_shell role"))
        if data.get("target_asset") != "ui/editor/editor_shell.neui":
            out.append(Finding("ERROR", "editor-neui", EDITOR_CHROME_IMPORT, "editor shell target must be editor_shell.neui"))
        docks = data.get("docks") or []
        seen = {str(item.get("slot_id")) for item in docks if isinstance(item, dict)}
        for panel_id in REQUIRED_PANEL_IDS:
            if panel_id not in seen:
                out.append(Finding("ERROR", "editor-neui", EDITOR_CHROME_IMPORT, "editor chrome import missing dock descriptor", panel_id))
    editor_dir = ENGINE_ROOT / "assets" / "ui" / "editor"
    for filename in sorted(REQUIRED_IMPORT_DESCRIPTORS):
        path = editor_dir / filename
        item, findings = load_json(path)
        out.extend(findings)
        if findings:
            continue
        if item.get("transport") != "UiNodeTreeRequest":
            out.append(Finding("ERROR", "editor-neui", path, "panel descriptor must declare UiNodeTreeRequest transport"))
        if not str(item.get("target_asset", "")).endswith(".neui"):
            out.append(Finding("ERROR", "editor-neui", path, "panel target_asset must be .neui"))
        if not str(item.get("source_gateway", "")).startswith("engine."):
            out.append(Finding("ERROR", "editor-neui", path, "panel source_gateway must be engine.*"))
    return out


def scan_tooling() -> list[Finding]:
    out: list[Finding] = []
    tool_paths = (TOOLS_INIT, TOOLS_INVARIANTS, TOOLS_RUN, CLI, VALIDATION_TOOL, SUITE_REGISTRY)
    joined = "\n".join(read(path) for path in tool_paths)
    for token in ("run_p3_editor_shell_scan", "editor-shell", "diag.editor.shell", "p3_editor_shell_scan.py"):
        if token not in joined:
            out.append(Finding("ERROR", "editor-tooling", REPO_ROOT / "tools" / "scripts" / "takesome", "tooling must expose P3 editor shell gate", token))
    return out


def run_checks() -> list[Finding]:
    findings: list[Finding] = []
    findings.extend(scan_build_regression_fix())
    findings.extend(scan_ui_transport())
    findings.extend(scan_panels())
    findings.extend(scan_neui_descriptors())
    findings.extend(scan_tooling())
    return findings


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="p3_editor_shell_scan.py")
    parser.add_argument("--summary-only", action="store_true")
    ns = parser.parse_args(argv)
    findings = run_checks()
    errors = [f for f in findings if f.severity == "ERROR"]
    warnings = [f for f in findings if f.severity == "WARN"]
    if not ns.summary_only:
        for finding in findings:
            print(finding.render())
    print(f"p3 editor shell scan: errors={len(errors)} warnings={len(warnings)}")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
