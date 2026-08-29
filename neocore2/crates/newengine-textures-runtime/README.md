# newengine-textures-runtime

Provider-neutral `textures.api` semantic service implementation for `.ytd` texture dictionaries.

This crate builds the service but does not register a Host route. `PluginsSrc/TexturesRuntime` owns first-party plugin identity, backend priority and `engine.assets.textures` route metadata, allowing a compatible plugin to replace the implementation without rebuilding GameReady or AssetInspector.

Boundary rule:

```text
engine.assets.textures owns .ytd semantics and runtime texture packets.
engine.assets owns VFS bytes and codec dispatch.
renderer/UI/materials consume texture packets or validation DTOs, never raw .ytd bytes.
```

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-textures-runtime`

**Role:** Texture dictionary semantic service factory; no Host registration ownership.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
