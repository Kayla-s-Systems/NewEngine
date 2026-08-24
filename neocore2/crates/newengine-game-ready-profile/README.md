# newengine-game-ready-profile

Product profile for the Game Ready FPS vertical slice.

This crate owns module composition and product launch policy only. Runtime-hosted scene/ECS/entity gateway services live in dedicated runtime crates and are installed by this profile when selected.

The profile consumes required `engine.assets.maps` and `engine.assets.textures` capabilities through stable gateways. It does not link or register concrete map/texture providers; `MapsRuntime`, `TexturesRuntime`, or compatible third-party plugins may provide, replace, or override those capabilities.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-game-ready-profile`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
