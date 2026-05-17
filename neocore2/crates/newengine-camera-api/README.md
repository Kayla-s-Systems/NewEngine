# newengine-camera-api

Stable engine-facing contract for `engine.camera`.

This crate is data-only: it contains gateway constants and serializable DTOs for
resolved camera frames, post-effect intent and diagnostics. Concrete camera
runners/directors live in camera providers/runtime crates. Render providers must
not import camera implementation crates.
