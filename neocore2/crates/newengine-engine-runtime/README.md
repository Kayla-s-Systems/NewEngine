# newengine-engine-runtime

Reusable runtime composition layer for standalone GameFirst runtime apps.

This crate owns systems that are engine runtime responsibilities, not application responsibilities:

- scene bridge and runtime scene commands;
- gameplay components/schedules used by runtime profiles;
- viewport bridge;
- render controller that talks only to `newengine-render-api`;
- material/texture residency orchestration above the render backend.

Standalone games depend on this crate through product profiles such as `newengine-game-ready-profile`. They must not call a native graphics API, create pipelines, upload textures, build shadow passes or assemble postFX directly.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-engine-runtime`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
