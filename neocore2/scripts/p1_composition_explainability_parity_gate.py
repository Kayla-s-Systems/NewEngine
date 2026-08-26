#!/usr/bin/env python3
"""P1 gate: Runtime and Editor consume one shared composition explanation graph."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
SERVICE = ROOT / "crates/newengine-service-api/src/resolver.rs"
RUNTIME_ACTIVE = ROOT / "crates/newengine-plugin-host/src/service_gateway/registry/active.rs"
RUNTIME_GATEWAY = ROOT / "crates/newengine-plugin-host/src/host_context/gateway/registry.rs"
RUNTIME_EXPORT = ROOT / "crates/newengine-plugin-host/src/host_context/gateway.rs"
EDITOR = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"

errors = []
service = SERVICE.read_text(encoding="utf-8")
runtime_active = RUNTIME_ACTIVE.read_text(encoding="utf-8")
runtime_gateway = RUNTIME_GATEWAY.read_text(encoding="utf-8")
runtime_export = RUNTIME_EXPORT.read_text(encoding="utf-8")
editor = EDITOR.read_text(encoding="utf-8")

for required in (
    "CompositionExplanationGraph",
    "GatewayCompositionExplanation",
    "CompositionCandidateExplanation",
    "CompositionRequirementEvaluation",
    "CompositionScoreBreakdown",
    "CompositionRejectionReason",
    "CompositionRejectionKind",
    "CompositionCandidateDisposition",
    "contract_resolutions",
    "outranked_by",
    "preferred_tag_matches",
    "with_preflight_rejection",
    "candidate_requirement_rejections",
):
    if required not in service:
        errors.append(f"shared service-api explanation model missing: {required}")

for reason in (
    "format_mismatch",
    "composition_forbidden_tag",
    "missing_capability",
    "missing_capability_version",
    "capability_version_below_minimum",
    "capability_version_above_maximum",
    "contract_mismatch",
    "missing_contract_version",
    "contract_version_below_minimum",
    "contract_version_above_maximum",
    "missing_required_tag",
    "forbidden_tag",
    "fallback_suppressed",
):
    if f'"{reason}"' not in service:
        errors.append(f"stable shared rejection code missing: {reason}")

for required in (
    "explain_format_type",
    "CompositionRejectionKind::FormatMismatch",
    "with_contract_resolutions",
    "contract_catalog_provenance_is_attached_to_shared_explanation_graph",
    "format_mismatch_is_reported_by_shared_explanation_graph",
    "candidate.summary()",
):
    if required not in editor:
        errors.append(f"Editor does not consume shared explainability graph: {required}")

# Format mismatch must be represented as a shared preflight rejection, not filtered
# out before the solver where it would disappear from Why-not diagnostics.
if ".filter(|provider| provider_supports_format_type(provider, format_type))" in editor:
    errors.append("Editor still drops format-incompatible providers before shared explanation")

for required in (
    "composition_explanation",
    "CompositionCandidateDisposition::Selected",
    "CompositionCandidateDisposition::Shadowed",
    "with_contract_catalog",
    "runtime_contract_resolution",
):
    if required not in runtime_active:
        errors.append(f"Runtime ActiveGatewayRegistry does not consume shared explanation: {required}")

for required in (
    "engine_composition_explanation",
    "explain_engine_gateway_composition",
    ".with_contract_catalog(&contract_catalog)",
):
    if required not in runtime_gateway:
        errors.append(f"Runtime public explanation/provenance path missing: {required}")

for required in (
    "engine_composition_explanation",
    "explain_engine_gateway_composition",
):
    if required not in runtime_export:
        errors.append(f"Runtime explanation API not exported: {required}")

if "provider_rank(" in editor:
    errors.append("Editor-local provider ranking authority reappeared")

if errors:
    print("[p1-composition-explainability-parity] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-composition-explainability-parity] OK")
print("  requirements/candidates -> shared evaluations -> selected/shadowed/rejected -> Runtime + Editor")
print("  contract provenance -> shared CompositionExplanationGraph")
