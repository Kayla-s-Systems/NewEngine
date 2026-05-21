# Stabilization pass — data-driven Definition Entries / YTYP

## Goal

Move object metadata away from hardcoded scene/runtime tables and toward a
self-registering asset-codec model.

```text
asset bytes
  -> registered codec descriptor
  -> engine.assets decode
  -> engine.model canonical DTO
  -> runtime construction
```

## Implemented

- Added `newengine-codec-ytyp` as an isolated AssetManager codec worker.
- Added `.ytyp` descriptor as `definitionType`, not as `plainText` and not as an AssetManager built-in.
- Added `DefinitionEntry`, `DefinitionBounds`, `DefinitionDictionaries` and
  `DefinitionEntriesManifest` as the canonical model-domain DTOs.
- Added `engine.model / model.definition_entries_json_v1` so model runtime reads
  YTYP metadata through `engine.assets` and the codec registry.
- Replaced duplicated YDD/YTYP decode glue in model runtime with one generic
  `decode_model_manifest<T>()` path.
- Moved model feature-domain list into `MODEL_FEATURE_DOMAINS` to avoid repeated
  hardcoded lists in service info and service description.
- Changed AssetManager codec workspace members to a glob so new codec worker
  crates are discovered by directory/manifest instead of editing the central
  workspace member list for every new format.
- Added explicit WIP opt-out metadata for `newengine-codec-spm` until the missing
  SpeedTree domain crate is available.
- Hardened codec dispatch so plain-text codecs never become global fallback
  handlers for arbitrary unknown files.

## Runtime shape

```text
metadata/foo.ytyp
  -> asset.codec.ytyp
  -> model.definition_entries_json
  -> DefinitionEntriesManifest
```

The output uses `definition_entries` as the canonical collection name. `archetype`
remains source vocabulary from CMapTypes, not a second engine model.

## Remaining large-object debt

This pass does not claim every historical >550 line file is split. The largest
known follow-up targets are still service/runtime mega files such as:

```text
Plugins/AssetManager/newengine-AssetManager/src/module/service.rs
NewEngine/neocore2/crates/newengine-core/src/engine/module_boot.rs
NewEngine/neocore2/crates/newengine-plugin-host/src/host_context.rs
Plugins/VulkanRenderer/newengine-modules-render-vulkan-ash/src/render_api/graph_executor.rs
```

The next stabilization pass should split those around explicit contracts:
request parsing, lifecycle worker, codec dispatch, service method router,
telemetry/status projection and backend-specific execution phases.


## Follow-up: `.ytyp` is not plain text

The codec classification was corrected from `plainText` to `definitionType`.
That keeps the data contract centered on Definition Entries while allowing the
source bytes to evolve from CMapTypes XML to the reserved `NEYTYP01` binary
envelope with raw or deflate-compressed payloads. AssetManager still only routes
by descriptor; XML parsing, binary envelope parsing and inflate are codec-owned.
