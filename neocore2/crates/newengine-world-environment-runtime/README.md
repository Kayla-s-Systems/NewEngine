# newengine-world-environment-runtime

Engine-runtime provider routes for `engine.world.environment`.

This crate intentionally owns only baseline provider behavior and diagnostics. It does not mutate ECS/world storage, inspect renderer state or become a sky/render helper.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-world-environment-runtime`

**Role:** Environment, sky, weather, lighting, or world atmosphere assets.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
