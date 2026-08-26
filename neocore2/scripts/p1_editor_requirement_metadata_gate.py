#!/usr/bin/env python3
"""P1 gate: FormatTypeDescriptor can express typed CompositionRequirement policy."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
ASSETS = REPO / "editor/northstar-gui-editor-assets/src/format_types/mod.rs"
REGISTRY = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"
RESOLVER = ROOT / "crates/newengine-service-api/src/resolver.rs"

errors = []
assets = ASSETS.read_text(encoding="utf-8")
registry = REGISTRY.read_text(encoding="utf-8")
resolver = RESOLVER.read_text(encoding="utf-8")

for required in (
    "pub struct FormatTypeCapabilityRequirement",
    "pub capability_requirements: Vec<FormatTypeCapabilityRequirement>",
    "pub min_capability_version: u32",
    "pub max_capability_version: Option<u32>",
    "pub contract_id: Option<String>",
    "pub min_contract_version: u32",
    "pub max_contract_version: Option<u32>",
    "pub required_tags: Vec<String>",
    "pub preferred_tags: Vec<String>",
    "pub forbidden_tags: Vec<String>",
    '"capability_requirements"',
    "normalize_capability_requirements",
    "typed format capability requirements require valid JSON",
):
    if required not in assets:
        errors.append(f"FormatType requirement model/parsing missing: {required}")

for required in (
    "composition_requirement_for_format_capability",
    "requirement.min_capability_version",
    "requirement.max_capability_version",
    "requirement.contract_id.clone()",
    "requirement.min_contract_version",
    "requirement.max_contract_version",
    "requirement.required_tags.clone()",
    "requirement.preferred_tags.clone()",
    "requirement.forbidden_tags.clone()",
    ".capability_requirements",
):
    if required not in registry:
        errors.append(f"Editor requirement -> CompositionRequirement projection missing: {required}")

if "fn provider_rank(" in registry:
    errors.append("retired editor provider_rank authority returned")

for required in (
    "pub min_capability_version: u32",
    "pub max_capability_version: Option<u32>",
    "pub contract_id: Option<String>",
    "pub min_contract_version: u32",
    "pub max_contract_version: Option<u32>",
    "pub required_tags: Vec<String>",
    "pub preferred_tags: Vec<String>",
    "pub conflict_tags: Vec<String>",
    "requirements_for_gateway",
    "candidate_matches(",
):
    if required not in resolver:
        errors.append(f"shared resolver lacks generic typed requirement support: {required}")

# Explicit metadata must be able to add capabilities beyond the legacy boolean surface.
if "capabilities.extend(" not in registry or "requirement.capability_id.clone()" not in registry:
    errors.append("explicit FormatType requirement cannot introduce a custom capability")

if errors:
    print("[p1-editor-requirement-metadata] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-editor-requirement-metadata] OK")
print("  FormatTypeDescriptor policy -> CompositionRequirement -> CapabilityMatrix -> CompositionSolver")
