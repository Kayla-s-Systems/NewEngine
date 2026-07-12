# newengine-input-bindings-runtime

Runtime-hosted `engine.input.bindings` service runtime. It stores/canonicalizes the active input profile, persists it under config, resolves semantic action frames from raw input and registers the engine-runtime route.

## Internal architecture

- `state.rs` — singleton gateway state and default-profile installation.
- `persistence.rs` — profile path, load and save operations.
- `service.rs` — JSON service router.
- `api.rs` — runtime snapshot, resolution and mutation API.
- `registration.rs` — engine gateway registration.

Profile save now returns the snapshot captured under the original lock instead of reacquiring the global mutex.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-input-bindings-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
