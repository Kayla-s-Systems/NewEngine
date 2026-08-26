#!/usr/bin/env python3
"""P0/P1 gate: every runtime-unit/provider choice belongs to shared composition authority."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_UNITS = ROOT / "crates/newengine-runtime-host/src/app_launcher/runtime_units.rs"
THREADING = ROOT / "crates/newengine-runtime-host/src/threading_gateway.rs"
BOOTSTRAP = ROOT / "crates/newengine-runtime-host/src/app_launcher/bootstrap.rs"

errors: list[str] = []
runtime_units = RUNTIME_UNITS.read_text(encoding="utf-8")
threading = THREADING.read_text(encoding="utf-8")
bootstrap = BOOTSTRAP.read_text(encoding="utf-8")

for forbidden, message in (
    ("provider_for_capability", "runtime-unit dependency closure still owns a local provider selector"),
    ("fn close_dependencies(", "runtime-unit dependency closure still bypasses fixed-point solver requirements"),
):
    if forbidden in runtime_units:
        errors.append(message)

select_start = runtime_units.find("fn select_runtime_unit_keys(")
select_end = runtime_units.find("fn topological_runtime_unit_order(", select_start)
if select_start < 0 or select_end < 0:
    errors.append("runtime-unit selection function boundaries are missing")
else:
    selection = runtime_units[select_start:select_end]
    for token in (
        "CompositionSolver::resolve_input",
        "dependency_requirements",
        "RuntimeUnitRequirementDescriptor::required(dependency)",
        "loop {",
    ):
        # CompositionSolver lives in solve_candidates immediately above this function;
        # allow the shared helper as long as the fixed-point function calls it.
        if token == "CompositionSolver::resolve_input":
            if token not in runtime_units:
                errors.append("runtime-unit selection no longer delegates to CompositionSolver")
        elif token not in selection:
            errors.append(f"runtime-unit fixed-point closure missing: {token}")
    if selection.count("solve_candidates(") < 2:
        errors.append("runtime-unit roots/dependencies are not both resolved through shared solve_candidates")
    if re.search(r"\.max_by(?:_key)?\s*\(", selection):
        errors.append("runtime-unit selection contains a local max_by provider choice")

if "has_service(THREADING_SERVICE_ID)" in threading:
    errors.append("threading/jobs compatibility route still branches on concrete service presence")
if "register_native_jobs_gateway_alias_best_effort" not in threading:
    errors.append("threading/jobs compatibility alias must be scoped to the native threading provider")
if "registered && register_native_jobs_gateway_alias_best_effort()" not in threading:
    errors.append("engine.jobs compatibility alias must only be installed after native threading registration")
if "active_engine_gateway_route(ENGINE_THREADING_SERVICE_ID)" in threading:
    errors.append("RuntimeHost must not fabricate engine.jobs for an arbitrary selected threading provider")

if "has_service(newengine_assets_api::ASSET_PROVIDER_SERVICE_ID)" in bootstrap:
    errors.append("asset bootstrap still treats concrete provider presence as availability")
if re.search(r"AssetServiceClient::for_service\s*\([^)]*ASSET_PROVIDER_SERVICE_ID", bootstrap, re.S):
    errors.append("asset bootstrap still bypasses engine.assets through a concrete provider service")
if "has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID)" not in bootstrap:
    errors.append("asset bootstrap must define availability exclusively through engine.assets gateway")

if errors:
    print("[p0-composition-selection-authority] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p0-composition-selection-authority] OK")
print("  runtime-unit roots + transitive dependencies -> CompositionSolver fixed point")
print("  runtime-host compatibility paths -> gateway authority; native aliases never use service presence as selection")
