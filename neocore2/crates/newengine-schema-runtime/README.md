# newengine-schema-runtime

Core-owned baseline provider for `engine.schema`.

This crate is part of the engine foundation, not an externally required plugin.
It registers a normal replaceable gateway route:

```text
engine.schema -> schema.api -> engine.schema.registry
capability: schema.registry
origin: EngineRuntime
```

External first-party/game/mod providers may replace it through the standard
Gateway/Capability registry. The baseline route remains visible in diagnostics
and can become shadowed rather than becoming a hidden singleton or hardcoded
Inspector branch.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-schema-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
