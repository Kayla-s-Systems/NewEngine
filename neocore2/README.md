# neocore2

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2`

**Role:** Core Rust engine workspace containing host/runtime crates, configuration, scripts, and engine-side infrastructure.

**Local contents:** 9 direct subdirectories, 4 direct files.

**Direct file examples:** `Cargo.lock`, `Cargo.toml`, `config.json`, `deny.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
