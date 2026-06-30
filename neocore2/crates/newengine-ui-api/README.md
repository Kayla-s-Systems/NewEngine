# newengine-ui-api

Stable engine-facing UI service contract for `engine.ui`.

The UI gateway is owned by the engine host. Concrete UI providers implement `ui.api` or a descriptor-declared provider service and advertise `ui.backend` metadata. Render, camera and gameplay systems publish UI-neutral telemetry/state to `engine.ui`; they do not talk to concrete UI providers.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-ui-api`

**Role:** User interface runtime assets or UI provider implementation data.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
