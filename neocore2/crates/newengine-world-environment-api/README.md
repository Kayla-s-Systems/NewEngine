# newengine-world-environment-api

Stable DTO contract for the `engine.world.environment` gateway.

Environment is world meaning, not renderer state. Providers receive DTOs and return DTOs; render, AI, audio, physics and tools consume the resolved environment frame through gateway-owned contracts.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-world-environment-api`

**Role:** Environment, sky, weather, lighting, or world atmosphere assets.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
