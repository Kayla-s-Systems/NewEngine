# newengine-lighting

Scene-domain lighting component/resource DTOs shared by ECS, render extraction and profile feature packs.

This crate is **not** a lighting renderer, not a shadow backend, and not a GPU tiled/clustered implementation. It contains only authoring/runtime scene data such as ambient, directional, point and spot light parameters plus explicit shadow settings.

Native execution belongs to the selected `engine.render` provider:

```text
ECS light components/resources
  -> render feature extraction
  -> render.api frame/light packets
  -> renderer-owned LightBuffer / LightGrid / ClusterGrid
  -> native deferred/tiled/clustered lighting
```

There is intentionally no automatic shadow method selector in this crate. Provider capability negotiation and profile policy decide whether cascaded, point, spot or disabled shadows are used.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-lighting`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
