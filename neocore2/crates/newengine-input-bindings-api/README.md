# newengine-input-bindings-api

Configuration/profile API for `engine.input.bindings`. This crate defines binding DTOs, manifests, profile canonicalization and service constants. It deliberately contains no GameReady FPS default action or binding set.

## Internal architecture

- `contracts.rs` — service constants and service-info contract.
- `binding.rs` / `axis.rs` — binding and gamepad-axis DTOs.
- `registration.rs` — key, binding and manifest registration DTOs.
- `profile.rs` + `profile/` — profile model, canonicalization, mutation and query behavior.
- `resolve.rs` — raw-frame matching and semantic action dispatch.
- `labels.rs` — canonical display labels.

The action-resolution hot path does not allocate a temporary action map or duplicate-action set per frame.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-input-bindings-api`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
