# newengine-camera-api

Stable engine-facing contract for `engine.camera`.

This crate is data-only: it contains gateway constants and serializable DTOs for
resolved camera frames, post-effect intent and diagnostics. Concrete camera
runners/directors live in camera providers/runtime crates. Render providers must
not import camera implementation crates.

## 2026-05-26 large-world snapshot fields

`CameraFrameSnapshot` now carries explicit large-world metadata in addition to legacy renderer-ready `f32` fields:

- `position_ws_f64`
- `world_origin_ws_f64`
- `position_origin_relative_ws`

This keeps the protocol compatible with simple providers while allowing future camera providers to publish double-precision authored world position and camera-local render origins without changing the gateway method shape.
