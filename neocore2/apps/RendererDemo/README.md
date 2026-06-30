# RendererDemo



Renderer-focused runtime smoke target for NewEngine.



Run from `EngineRepo/NewEngine/neocore2`:



```bash

cargo run -p renderer-demo

```



Expected result:



- opens a platform window titled `RendererDemo: Shaded Lighting Scene`

- loads the `maps/game_ready_highlands.ymap` scene profile

- uses the GameReady material feature pack and light extraction providers

- renders a shaded/lit demo scene through the configured render backend



This app is intentionally a thin launcher over `GameReadyRuntimeProfile`; renderer ownership stays in `Plugins/VulkanRenderer` and the GameReady render feature pack.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/apps/RendererDemo`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 3 direct files.

**Direct file examples:** `build.rs`, `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
