# newengine-camera-runtime

Runtime camera manager and blend-runner logic. Produces camera snapshots for downstream systems without giving renderer code ownership of gameplay camera policy.

## Architecture notes

This crate is part of the North Star Engine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.


## 2026-05-17 completion pass

The runtime camera layer now contains director arbitration, per-director settings, transition events, viewport cache/fade state and resolved post-effect propagation. Higher-level gameplay/cinematic systems can submit director outputs to `CameraManagerResource::submit_director_output`, while the active runtime/gameplay path continues to produce the renderer-facing `CameraFrame`.


## 2026-05-26 viewport history pass

`CameraViewportLayer` now owns `CameraFrameHistory`. The runtime manager pushes the resolved frame into the active viewport layer during `present_frame(frame, dt)`. This makes camera history inspectable and per-viewport, which is required for TAA, motion blur, replay diagnostics and streaming heuristics.
