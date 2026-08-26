#!/usr/bin/env python3
"""P1 gate: Editor contract references are resolved through the normative runtime catalog."""
from pathlib import Path
import json
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
NORTHSTAR = REPO.parent
EDITOR_REGISTRY = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"
EDITOR_CARGO = REPO / "editor/northstar-gui-editor-gateway/Cargo.toml"
EDITOR_HOST = REPO / "editor/northstar-gui-editor-host/src/host/editor_host.rs"
CONTRACT_REGISTRY = ROOT / "crates/newengine-contract-registry/src/lib.rs"
CONTRACT_CATALOG = ROOT / "crates/newengine-runtime-contract-catalog/src/lib.rs"
ASSETS_API = ROOT / "crates/newengine-assets-api/src/lib.rs"
DEPLOYED_CODECS = NORTHSTAR / "pluginsRuntime/codecs/codec_manifest.json"

errors = []
editor = EDITOR_REGISTRY.read_text(encoding="utf-8")
cargo = EDITOR_CARGO.read_text(encoding="utf-8")
host = EDITOR_HOST.read_text(encoding="utf-8")
registry = CONTRACT_REGISTRY.read_text(encoding="utf-8")
catalog = CONTRACT_CATALOG.read_text(encoding="utf-8")
assets = ASSETS_API.read_text(encoding="utf-8")

for required in (
    "newengine-runtime-contract-catalog",
):
    if required not in cargo:
        errors.append(f"Editor gateway is not linked to runtime contract catalog: {required}")

for required in (
    "RuntimeContractCatalog",
    "from_providers_with_contract_catalog",
    "canonicalize_and_validate_provider_contracts",
    "canonicalize_and_validate_requirement_contract",
    "validate_offered_major",
    "validate_required_major_range",
    "entry.spec.key.clone()",
    "Result<Option<GatewayRoute>, String>",
):
    if required not in editor:
        errors.append(f"Editor contract validation/canonicalization path missing: {required}")

for required in (
    "load_with_contract_catalog",
    "from_providers_with_contract_catalog",
    "refresh_routes_and_workspace_model",
):
    if required not in host:
        errors.append(f"EditorHost can bypass/inhibit instance contract catalog parity: {required}")

for required in (
    "resolve_contract_reference",
    "canonical_contract_key",
    "validate_offered_major",
    "validate_required_major_range",
):
    if required not in catalog:
        errors.append(f"Runtime Contract Catalog lacks shared reference validation API: {required}")

for required in (
    "ASSET_DECODE_PROTOCOL_CONTRACT_SPEC",
    "CONTAINER_WRITE_BYTES_PROTOCOL_CONTRACT_SPEC",
    "ASSET_PREVIEW_PROTOCOL_CONTRACT_SPEC",
):
    if f"newengine_assets_api::{required}" not in registry:
        errors.append(f"normative Engine Contract Registry misses first-party asset contract {required}")

# Resolve the advertised ids of the asset contracts registered above. This gate is
# intentionally source-driven: deployed provider metadata must refer to an id that
# is represented by a ContractSpec in the normative trust root.
def const_string(name: str):
    m = re.search(rf"pub const {re.escape(name)}\s*:\s*&str\s*=\s*\"([^\"]+)\"", assets)
    return m.group(1) if m else None

def advertised_id_for_spec(spec_name: str):
    m = re.search(
        rf"pub const {re.escape(spec_name)}[^=]*=\s*newengine_contract_api::ContractSpec::new\((.*?)\n\s*\);",
        assets,
        re.S,
    )
    if not m:
        return None
    body = m.group(1)
    matches = re.findall(r"Some\(([^)]+)\)", body)
    if not matches:
        return None
    expr = matches[-1].strip()
    if expr.startswith('"') and expr.endswith('"'):
        return expr[1:-1]
    if "::" in expr:
        name = expr.rsplit("::", 1)[1]
        value = const_string(name)
        if value:
            return value
    return const_string(expr)

registered_codec_contract_ids = set()
for spec in (
    "ASSET_DECODE_PROTOCOL_CONTRACT_SPEC",
    "CONTAINER_WRITE_BYTES_PROTOCOL_CONTRACT_SPEC",
    "ASSET_PREVIEW_PROTOCOL_CONTRACT_SPEC",
):
    advertised = advertised_id_for_spec(spec)
    if not advertised:
        errors.append(f"cannot resolve advertised id for normative contract {spec}")
    else:
        registered_codec_contract_ids.add(advertised)

if DEPLOYED_CODECS.exists():
    manifest = json.loads(DEPLOYED_CODECS.read_text(encoding="utf-8"))
    used = set()
    for codec in manifest.get("codecs", []):
        for capability in codec.get("capabilities", []):
            if not isinstance(capability, dict):
                continue
            contract = capability.get("contract")
            if isinstance(contract, dict) and contract.get("id"):
                used.add(contract["id"])
    unknown = sorted(used - registered_codec_contract_ids)
    if unknown:
        errors.append(
            "deployed codec_manifest.v3 references contracts absent from the normative Engine registry: "
            + ", ".join(unknown)
        )
else:
    errors.append("deployed codec_manifest.json is missing")

if errors:
    print("[p1-editor-contract-registry-parity] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-editor-contract-registry-parity] OK")
print("  provider/format contract ref -> RuntimeContractCatalog -> canonical registry key -> CompositionSolver")
print(f"  deployed_contract_ids={len(registered_codec_contract_ids)}")
