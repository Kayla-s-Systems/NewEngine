# newengine-input-contexts-api

Context stack and modal capture contract for `engine.input.contexts`.

## Internal architecture

- `contracts.rs` — `engine.input.contexts` service contract and service-info DTO.
- `context.rs` — context, lifetime, capture policy and ordered context stack.
- `capture.rs` — provider-neutral modal capture state.

`lib.rs` only re-exports the stable public contract.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-input-contexts-api`

**Role:** Text shaping, text rendering, or font-related implementation.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
