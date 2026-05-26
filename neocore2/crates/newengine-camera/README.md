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

## 2026-05-26 large-world/simple-use pass

This pass adds the practical camera layer expected by runtime tools and gameplay prototypes:

- `Camera` — small facade for simple create/look-at/frame usage.
- `CameraController` — minimal Orbit/Fly wrapper for tools and demos.
- `CameraLens`, `CameraOrthoLens`, `CameraClipPolicy` — explicit lens and clipping policy data.
- `CameraWorldPoint`, `CameraWorldOrigin`, `CameraLargeWorldRig`, `LargeWorldCamera` — double-precision authored position with explicit camera-local origin lowering into renderer-safe `f32` frames.

Large-world rule:

```text
Authoritative world position stays f64.
Render-facing frame stays local f32 relative to an explicit origin.
Origin changes are inspectable data, not hidden singleton magic.
```

Minimal usage:

```rust
let mut camera = newengine_camera::Camera::from_size(1920, 1080);
camera.look_at(
    newengine_math::Vec3::new(0.0, 1.6, 4.0),
    newengine_math::Vec3::new(0.0, 1.0, 0.0),
);
let frame = camera.frame();
```

Large-world usage:

```rust
let camera = newengine_camera::LargeWorldCamera::new(
    newengine_camera::CameraWorldPoint::new(10_000_000.0, 80.0, -7_000_000.0),
    newengine_camera::CameraViewport::from_size(1920, 1080),
    newengine_camera::CameraLens::default(),
);
let large_frame = camera.frame();
let renderer_frame = large_frame.frame;
```
