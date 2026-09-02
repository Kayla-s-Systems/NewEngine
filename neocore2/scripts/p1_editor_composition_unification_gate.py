#!/usr/bin/env python3
"""P1 gate: Editor and runtime share one capability resolution authority."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
REGISTRY = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"
MANIFEST = REPO / "editor/northstar-gui-editor-gateway/Cargo.toml"
RESOLVER = ROOT / "crates/newengine-service-api/src/resolver.rs"

errors = []
registry = REGISTRY.read_text(encoding="utf-8")
manifest = MANIFEST.read_text(encoding="utf-8")
resolver = RESOLVER.read_text(encoding="utf-8")

for forbidden in (
    "fn provider_rank(",
    "provider_rank(",
    ".sort_by(|a, b| provider_rank",
):
    if forbidden in registry:
        errors.append(f"editor registry contains retired local resolution authority: {forbidden}")

for required in (
    "CompositionSolver::resolve_input",
    "CapabilityMatrix::new",
    "CompositionSolverInput",
    ".with_capability_metadata(",
):
    if required not in registry:
        errors.append(f"editor registry is missing shared composition primitive: {required}")

for capability in (
    "asset.format.read",
    "asset.format.write",
    "asset.format.inspect",
    "asset.format.validate",
    "asset.format.diff",
    "asset.editor.edit_schema",
):
    if capability not in registry:
        errors.append(f"editor typed requirement projection is missing: {capability}")

if "newengine-service-api" not in manifest:
    errors.append("editor gateway must depend directly on shared newengine-service-api")

for shared_feature in (
    "capability_requirements: Vec<CompositionRequirement>",
    "pub fn requirements_for_gateway",
    "pub fn with_capabilities",
    "pub fn with_capability_metadata",
    ".all(|req| candidate_matches(req, candidate))",
):
    if shared_feature not in resolver:
        errors.append(f"shared resolver lacks multi-capability gateway support: {shared_feature}")

if errors:
    print("[p1-editor-composition-unification] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-editor-composition-unification] OK")
print("  FormatTypeDescriptor -> CapabilityMatrix -> CompositionSolver -> GatewayRoute")
