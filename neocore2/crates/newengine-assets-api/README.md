# newengine-assets-api

A minimal, stable contract surface for interacting with an AssetManager-like service.

This crate intentionally contains only:

- small data-oriented enums/traits,
- a tiny `wait_ready` helper.

Concrete implementations live in `newengine-assets` (client) and/or runtime plugins.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-assets-api`

**Role:** Static provider-local assets used by this crate/plugin/module.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
