# newengine-definitions-runtime

Runtime-hosted `engine.assets.definitions` runtime service. `.ytyp` Definition Entry semantics live here; `engine.assets` remains the VFS/bytes/codec owner only.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-definitions-runtime`

**Role:** Declarative object, type, archetype, or gameplay definitions.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
