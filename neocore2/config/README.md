# config

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/config`

**Role:** Configuration files, descriptors, or runtime policy data.

**Local contents:** Runtime policy is grouped by subsystem. Runtime code should consume these files through the durable `StartupConfig::config_child(...)` authority instead of embedding operational tuning constants in implementation modules.

**Current policy examples:**

- `audio/orchestration.runtime.json` — bounded command/event capacities and provider pre-arm policy for world-audio orchestration.
- `architecture/source-modularity.v1.json` — progressive production source-size budgets and modularization backlog policy.
- `animation/` — authored animation/runtime graph policy.
- `input/` — input bindings and input policy.
- `render/` — rendering policy and feature configuration.

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.
- Operational limits, feature policy, and project/profile tuning belong in versioned config/schema data when they are not ABI constants or mathematical invariants.
- Runtime policy parsers should fail closed on unsupported schema versions and unknown keys so stale configuration cannot silently change behavior.

<!-- NORTHSTAR-DIR-README:END -->
