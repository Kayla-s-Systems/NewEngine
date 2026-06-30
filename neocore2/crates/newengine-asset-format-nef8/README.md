# newengine-asset-format-nef8

Unified first-party asset format registry for North Star Engine.

This crate replaces the previous one-crate-per-extension descriptor split. Each
format is still data-declared, but the declaration lives in one NEF8/package
registry table: extension, content kind, semantic gateway, handler, outputs and
consumer domains. The engine collects descriptors from this crate instead of
compiling dozens of tiny `newengine-asset-format-*` crates.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-asset-format-nef8`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
