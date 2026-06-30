# newengine-camera-api

Stable engine-facing contract for `engine.camera`.

This crate is data-only: it contains gateway constants and serializable DTOs for resolved camera frames, post-effect intent and diagnostics. Concrete camera runners/directors live in camera providers/runtime crates. Render providers must not import camera implementation crates.

## 2026-05-26 world-origin snapshot fields

`CameraFrameSnapshot` carries explicit world-origin metadata in addition to renderer-ready `f32` fields:

- `position_ws_f64`
- `world_origin_ws_f64`
- `position_origin_relative_ws`

This keeps the protocol compatible with simple providers while allowing camera providers to publish double-precision authored world position and camera-local render origins without changing the gateway method shape.

## 2026-05-26 API cleanup

The API crate exposes only the active `engine.camera` gateway contract. Earlier dormant declarations for `engine.camera.modes` and `engine.camera.animations` were removed because they did not have concrete provider contracts or conformance tests in this snapshot. Future camera subdomains should be added through real provider/capability descriptors, not placeholder public constants.

`CameraProjectionKind` contains only projection kinds that are actually serialized by the runtime bridge: `Perspective` and `Orthographic`.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-camera-api`

**Role:** Engine repository directory. Keep files here scoped to this directory's domain and avoid cross-domain ownership drift.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
