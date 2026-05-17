# newengine-camera

Camera domain types and camera-state helpers shared by gameplay and runtime camera systems.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.


## 2026-05-17 director-system pass

The camera domain now exposes the foundations needed for a Rockstar-style camera architecture without copying the reference implementation directly:

- metadata contracts for camera objects, modes, contexts, lens settings and director blend policy;
- `CameraResolvedFrame` as a `CameraFrame` plus post-effect sidecar for DOF, motion blur, exposure and shake;
- reusable camera interpolation over rig/projection/post-effects;
- `CameraDirectorState`, `CameraRenderState` and `CameraDirectorRunner` for explicit director ownership.

Renderer-facing code should keep consuming `CameraFrame`. Higher-level runtime/cinematic systems should use `CameraResolvedFrame` and director outputs so camera policy remains outside the renderer.


Runtime integration note: `CameraResolvedFrame` is now consumed by the runtime manager and exported into render postfx envelopes, so DOF/motion-blur/exposure/shake settings are no longer a dormant sidecar.
