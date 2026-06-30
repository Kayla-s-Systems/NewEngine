# newengine-text-api

UI-owned text subdomain contract for `engine.ui.text`.

This crate is intentionally only a contract: font fallback, shaping, atlas allocation and localization are provider-owned. UI systems such as UI providers consume this gateway; renderers receive already shaped/atlased UI draw packets and do not own text layout policy.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-text-api`

**Role:** Text shaping, text rendering, or font-related implementation.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
