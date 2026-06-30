# newengine-textures-runtime

Runtime-hosted `engine.assets.textures` runtime service for `.ytd` texture dictionaries.

Boundary rule:

```text
engine.assets.textures owns .ytd semantics and runtime texture packets.
engine.assets owns VFS bytes and codec dispatch.
renderer/UI/materials consume texture packets or validation DTOs, never raw .ytd bytes.
```

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-textures-runtime`

**Role:** Text shaping, text rendering, or font-related implementation.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
