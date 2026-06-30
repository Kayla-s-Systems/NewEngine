# newengine-render-feature-gameready

Profile-owned GameReady render feature pack.

This crate implements `newengine-render-feature-api` providers for terrain,
primitive mesh/UI draw-list extraction and GameReady light/shadow policy. It has
no dependency on `newengine-engine-runtime`; the product profile composes the
returned providers into the active runtime controller.

The renderer backend remains replaceable behind `render.api`. This crate owns
profile policy, not backend submission or runtime controller state.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-render-feature-gameready`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
