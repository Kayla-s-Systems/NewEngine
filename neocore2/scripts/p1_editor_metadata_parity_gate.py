#!/usr/bin/env python3
"""P1 gate: Editor discovery and deployed first-party manifests carry V2 selection metadata."""
from pathlib import Path
import json
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
NORTHSTAR = REPO.parent
REGISTRY = REPO / "editor/northstar-gui-editor-gateway/src/registry/mod.rs"
TOOLS = REPO / "editor/northstar-gui-editor-gateway/src/tools/mod.rs"
TOOL_RUNTIME = REPO / "editor/northstar-gui-editor-host/src/tool_runtime.rs"
RESOLVER = ROOT / "crates/newengine-service-api/src/resolver.rs"
CODECS_ROOT = NORTHSTAR / "PluginsSrc/AssetManager/codecs"
DEPLOYED = NORTHSTAR / "pluginsRuntime/codecs/codec_manifest.json"
GENERATOR = CODECS_ROOT / "generate_codec_manifest.py"
CODEC_BUILD = CODECS_ROOT / "buildAllCodecs.cmd"
GLOBAL_BUILD = NORTHSTAR / "PluginsSrc/build_all_plugins.cmd"

errors = []
registry = REGISTRY.read_text(encoding="utf-8")
tools = TOOLS.read_text(encoding="utf-8")
tool_runtime = TOOL_RUNTIME.read_text(encoding="utf-8")
resolver = RESOLVER.read_text(encoding="utf-8")

for required in (
    "pub struct ProviderCapabilityMetadata",
    "pub capability_metadata: Vec<ProviderCapabilityMetadata>",
    "pub system_tags: Vec<String>",
    "pub backend_priority: i32",
    "provider.backend_priority",
    ".with_capability_metadata(",
):
    if required not in registry:
        errors.append(f"editor provider projection is missing V2 metadata flow: {required}")

for required in (
    '"capabilities_v2"',
    '"capability_metadata"',
    '"capability_version"',
    '"contract_version"',
    '"system_tags"',
    '"backend_priority"',
    '"runtime_provider_id"',
    'validate_codec_v3_entry',
    'validate_tool_provider_v2',
    'json_strings_value(entry, "formats")',
    "ProviderCapabilityMetadata::legacy",
):
    if required not in tools:
        errors.append(f"tool/codec manifest parser is missing V2/legacy metadata support: {required}")

for required in (
    '"northstar.tool_provider.v2"',
    '"northstar.tool_provider.v1"',
    "ToolManifest::parse",
    "invalid self-description",
    "runtime-self-describing-tool-legacy",
):
    if required not in tool_runtime:
        errors.append(f"self-describing tool discovery is missing V2-first compatibility flow: {required}")

for required in (
    "pub struct CompositionCapabilityMetadata",
    "pub capability_metadata: Vec<CompositionCapabilityMetadata>",
    "pub fn with_capability_metadata",
    "typed_capability",
    ".and_then(|capability| capability.version)",
    ".and_then(|capability| capability.contract_version)",
):
    if required not in resolver:
        errors.append(f"shared CompositionSolver cannot consume lossless V2 capability metadata: {required}")

if "fn provider_rank(" in registry:
    errors.append("retired editor provider_rank resolution authority returned")

# Source-authored first-party metadata is the authority for generated deployed manifests.
source_workers = {}
for manifest_path in sorted(CODECS_ROOT.glob("*/Cargo.toml")):
    with manifest_path.open("rb") as stream:
        doc = tomllib.load(stream)
    package = doc.get("package") or {}
    metadata = (((package.get("metadata") or {}).get("newengine") or {}).get("codec") or {})
    if metadata.get("enabled") is not True:
        continue
    name = package.get("name")
    runtime_provider_id = metadata.get("runtime_provider_id")
    capabilities = metadata.get("capabilities") or []
    if not name or not runtime_provider_id or not isinstance(metadata.get("backend_priority"), int):
        errors.append(f"{manifest_path}: enabled codec lacks package/runtime provider identity or backend_priority")
        continue
    if not metadata.get("system_tags"):
        errors.append(f"{manifest_path}: enabled codec lacks explicit system_tags")
    if "formats" in metadata:
        errors.append(f"{manifest_path}: codec metadata must not declare asset formats; engine.assets.types owns format identity")
    if not capabilities or any(not isinstance(item, dict) for item in capabilities):
        errors.append(f"{manifest_path}: capabilities must be V2 metadata objects")
    for capability in capabilities:
        if not capability.get("id") or not isinstance(capability.get("capability_version"), int):
            errors.append(f"{manifest_path}: capability lacks id/capability_version: {capability!r}")
        if capability.get("contract_id") and not isinstance(capability.get("contract_version"), int):
            errors.append(f"{manifest_path}: capability contract lacks version: {capability!r}")
    source_workers[name] = {
        "runtime_provider_id": runtime_provider_id,
        "backend_priority": metadata.get("backend_priority"),
        "system_tags": sorted(metadata.get("system_tags") or []),
        "capability_ids": sorted(item.get("id") for item in capabilities if isinstance(item, dict)),
    }

for build_file in (CODEC_BUILD, GLOBAL_BUILD):
    text = build_file.read_text(encoding="utf-8")
    if "generate_codec_manifest.py" not in text:
        errors.append(f"build pipeline does not regenerate V3 codec manifest: {build_file}")

if not GENERATOR.exists():
    errors.append("codec manifest generator is missing")
elif "package.metadata.newengine.codec" not in GENERATOR.read_text(encoding="utf-8"):
    errors.append("codec manifest generator does not declare Cargo metadata authority")

if not DEPLOYED.exists():
    errors.append("deployed pluginsRuntime/codecs/codec_manifest.json is missing")
else:
    deployed = json.loads(DEPLOYED.read_text(encoding="utf-8"))
    if deployed.get("schema") != "northstar.asset_manager.codec_manifest.v3":
        errors.append(f"deployed codec manifest is not V3: {deployed.get('schema')!r}")
    if deployed.get("metadata_authority") != "package.metadata.newengine.codec":
        errors.append("deployed codec manifest does not identify source metadata authority")
    deployed_workers = {item.get("codec"): item for item in deployed.get("codecs", [])}
    for name, source in source_workers.items():
        item = deployed_workers.get(name)
        if item is None:
            errors.append(f"deployed V3 manifest misses enabled codec {name}")
            continue
        if item.get("runtime_provider_id") != source["runtime_provider_id"]:
            errors.append(f"{name}: deployed runtime_provider_id drift")
        if item.get("backend_priority") != source["backend_priority"]:
            errors.append(f"{name}: deployed backend_priority drift")
        if sorted(item.get("system_tags") or []) != source["system_tags"]:
            errors.append(f"{name}: deployed system_tags drift")
        if item.get("formats"):
            errors.append(f"{name}: deployed codec manifest illegally carries asset format ownership")
        caps = item.get("capabilities") or []
        if any(not isinstance(cap, dict) or not isinstance(cap.get("capability_version"), int) for cap in caps):
            errors.append(f"{name}: deployed capabilities are not typed V2 objects")
        if sorted(cap.get("id") for cap in caps) != source["capability_ids"]:
            errors.append(f"{name}: deployed capability IDs drift from Cargo metadata")
        dll = item.get("dll")
        if not dll or not (DEPLOYED.parent / dll).exists():
            errors.append(f"{name}: deployed manifest points at missing DLL {dll!r}")
        if not item.get("sha256") or not isinstance(item.get("bytes"), int):
            errors.append(f"{name}: deployed artifact identity lacks sha256/bytes")

# Runtime source priorities must agree with the first-party Editor provider metadata.
priority_sources = {
    "newengine-codec-listfile": CODECS_ROOT / "newengine-codec-listfile/src/lib.rs",
    "newengine-codec-nepak": CODECS_ROOT / "newengine-codec-nepak/src/lib.rs",
}
for name, source_path in priority_sources.items():
    source = source_workers.get(name)
    if source is None:
        continue
    runtime_source = source_path.read_text(encoding="utf-8")
    priority_token = f"priority: {source['backend_priority']}"
    if priority_token not in runtime_source:
        errors.append(f"{name}: Cargo backend_priority does not match runtime CodecServiceSpec")
    provider_id_token = f'id: "{source["runtime_provider_id"]}"'
    if provider_id_token not in runtime_source:
        errors.append(f"{name}: Cargo runtime_provider_id does not match runtime CodecServiceSpec")

if errors:
    print("[p1-editor-metadata-parity] FAILED")
    for error in errors:
        print(f"  - {error}")
    sys.exit(1)

print("[p1-editor-metadata-parity] OK")
print("  source codec capabilities -> codec_manifest.v3; asset format identity -> engine.assets.types")
print(f"  first_party_codecs={len(source_workers)} deployed={len(source_workers)}")
