# newengine-render-ui-bridge

Renderer-neutral bridge from `engine.ui` output to the runtime frame graph.

This crate does not know about concrete UI provider internals and does not depend on Vulkan.
It only routes an already-built `UiDrawList` into the `RenderDrawListKind::Ui`
path so the active renderer backend can composite it through its own UI pass.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-render-ui-bridge`

**Role:** User interface runtime assets or UI provider implementation data.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
