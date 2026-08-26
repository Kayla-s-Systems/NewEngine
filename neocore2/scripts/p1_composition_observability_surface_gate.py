#!/usr/bin/env python3
"""P1 gate: one stable composition.snapshot_v1/diff_v1 observability surface is shared by Runtime and Editor."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
SERVICE_API = ROOT / "crates/newengine-service-api/src/observability.rs"
SERVICE_LIB = ROOT / "crates/newengine-service-api/src/lib.rs"
CONTRACT_REGISTRY = ROOT / "crates/newengine-contract-registry/src/lib.rs"
HOST_STATE = ROOT / "crates/newengine-plugin-host/src/host_context/state.rs"
HOST_REGISTRY = ROOT / "crates/newengine-plugin-host/src/host_context/gateway/registry.rs"
HOST_LIB = ROOT / "crates/newengine-plugin-host/src/lib.rs"
EDITOR = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"

errors = []
files = {
    "observability": SERVICE_API.read_text(encoding="utf-8"),
    "service_lib": SERVICE_LIB.read_text(encoding="utf-8"),
    "contract_registry": CONTRACT_REGISTRY.read_text(encoding="utf-8"),
    "host_state": HOST_STATE.read_text(encoding="utf-8"),
    "host_registry": HOST_REGISTRY.read_text(encoding="utf-8"),
    "host_lib": HOST_LIB.read_text(encoding="utf-8"),
    "editor": EDITOR.read_text(encoding="utf-8"),
}

for token in (
    'COMPOSITION_SNAPSHOT_SCHEMA_V1: &str = "composition.snapshot_v1"',
    'COMPOSITION_DIFF_SCHEMA_V1: &str = "composition.diff_v1"',
    'pub struct CompositionSnapshotV1',
    'pub struct CompositionDiffV1',
    'pub struct CompositionGatewaySnapshotV1',
    'pub struct CompositionCandidateSnapshotV1',
    'pub struct CompositionContractResolutionSnapshotV1',
    'pub fn from_plan(',
    'pub fn between(',
    'pub fn is_empty(&self)',
):
    if token not in files["observability"]:
        errors.append(f"shared observability DTO/diff surface missing: {token}")

for token in (
    'pub instance_id: u64',
    'pub composition_epoch: u64',
    'pub topology_generation: u64',
    'pub provenance: CompositionSnapshotProvenanceV1',
    'CompositionPlanModeV1::Frozen',
    'CompositionPlanModeV1::Live',
):
    if token not in files["observability"]:
        errors.append(f"snapshot identity/epoch/provenance field missing: {token}")

for token in (
    'COMPOSITION_SNAPSHOT_CONTRACT_SPEC',
    'COMPOSITION_DIFF_CONTRACT_SPEC',
    '"composition.snapshot.protocol"',
    '"composition.diff.protocol"',
):
    if token not in files["observability"] and token not in files["contract_registry"]:
        errors.append(f"normative observability contract missing: {token}")

for token in (
    'newengine_service_api::COMPOSITION_SNAPSHOT_CONTRACT_SPEC',
    'newengine_service_api::COMPOSITION_DIFF_CONTRACT_SPEC',
):
    if token not in files["contract_registry"]:
        errors.append(f"Engine Contract Registry does not include observability protocol: {token}")

for token in (
    'NEXT_HOST_CONTEXT_INSTANCE_ID',
    'pub(crate) instance_id: u64',
    'pub fn instance_id(&self) -> u64',
    'pub fn composition_snapshot_v1(&self)',
    'pub fn composition_snapshot_v1_json(&self)',
):
    if token not in files["host_state"]:
        errors.append(f"HostContext multi-instance observability path missing: {token}")

for token in (
    'pub fn engine_composition_snapshot_v1()',
    'pub fn engine_composition_snapshot_v1_json()',
    'generation_before & 1',
    'generation_after / 2',
    'host.frozen_composition_plan',
    'host.gateway_registry',
):
    if token not in files["host_registry"]:
        errors.append(f"runtime stable snapshot capture missing: {token}")

for token in (
    'engine_composition_snapshot_v1',
    'engine_composition_snapshot_v1_json',
):
    if token not in files["host_lib"]:
        errors.append(f"runtime public observability export missing: {token}")

for token in (
    'pub fn composition_snapshot_for_format_type(',
    'CompositionSnapshotV1::from_plan(',
    'editor.format_registry',
    'editor_format_composition_projects_to_shared_snapshot_v1',
):
    if token not in files["editor"]:
        errors.append(f"Editor is not consuming/projecting the shared snapshot DTO: {token}")

# The protocol must remain a DTO/read-model boundary, not serde derives added directly
# to the internal CompositionPlan/ExplanationGraph solver model.
resolver = (ROOT / "crates/newengine-service-api/src/resolver.rs").read_text(encoding="utf-8")
for forbidden in (
    '#[derive(Serialize, Deserialize)]\npub struct CompositionPlan',
    '#[derive(Serialize, Deserialize)]\npub struct CompositionExplanationGraph',
):
    if forbidden in resolver:
        errors.append("internal solver model was turned into wire schema; keep observability DTO separate")

if errors:
    print("[p1-composition-observability-surface] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-composition-observability-surface] OK")
print("  CompositionPlan/ExplanationGraph -> composition.snapshot_v1 DTO/JSON -> Runtime + Editor consumers")
print("  stable topology generation + instance id + frozen/live provenance -> composition.diff_v1")
