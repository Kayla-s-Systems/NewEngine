# newengine-gameplay-runtime

Core-owned baseline providers for the gameplay foundation domains:

- `engine.tags`
- `engine.tasks`
- `engine.animation`
- `engine.navigation`
- `engine.ai`

These providers are compiled with the runtime profile so gameplay foundations are
available without a mandatory external plugin. They still register through the
same gateway/capability route model, so external providers can replace them.
AI observes frame DTOs and returns intent DTOs; runtime apply stages own all
world/entity/ECS mutation.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-gameplay-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
