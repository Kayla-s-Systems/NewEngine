# newengine-camera

Camera domain types and camera-state helpers shared by gameplay, tools, cinematic and runtime camera systems.

## Architecture notes

This crate is part of the North Star Engine host/plugin architecture. Runtime-facing code should prefer `engine.camera` gateways and typed adapters over concrete provider implementation crates.

## 2026-05-17 director-system pass

The camera domain exposes the foundations needed for a production camera architecture without copying the reference implementation directly:

- metadata contracts for camera objects, modes, contexts, lens settings and director blend policy;
- `CameraResolvedFrame` as a `CameraFrame` plus post-effect sidecar for DOF, motion blur, exposure and shake;
- reusable camera interpolation over rig/projection/post-effects;
- `CameraDirectorState`, `CameraRenderState` and `CameraDirectorRunner` for explicit director ownership.

Renderer-facing code should keep consuming `CameraFrame`. Higher-level runtime/cinematic systems should use `CameraResolvedFrame` and director outputs so camera policy remains outside the renderer.

Runtime integration note: `CameraResolvedFrame` is consumed by the runtime manager and exported into render postfx envelopes, so DOF/motion-blur/exposure/shake settings are no longer a dormant sidecar.

## 2026-05-26 simple-use and world-origin pass

This pass adds the practical camera layer expected by runtime tools and gameplay prototypes:

- `Camera` — small facade for simple create/look-at/frame usage.
- `CameraController` — minimal Orbit/Fly wrapper for tools and demos.
- `CameraLens`, `CameraOrthoLens`, `CameraClipPolicy` — explicit lens and clipping policy data.
- `CameraWorldPoint`, `CameraWorldOrigin`, `CameraWorldRig`, `WorldCamera` — double-precision authored position with explicit camera-local origin lowering into renderer-safe `f32` frames.

Precision rule:

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

World-space usage:

```rust
let camera = newengine_camera::WorldCamera::new(
    newengine_camera::CameraWorldPoint::new(10_000_000.0, 80.0, -7_000_000.0),
    newengine_camera::CameraViewport::from_size(1920, 1080),
    newengine_camera::CameraLens::default(),
);
let world_frame = camera.frame();
let renderer_frame = world_frame.frame;
```

## 2026-05-26 AAA camera cleanup pass

This pass tightens the camera domain toward a production camera stack:

- removed the orphaned legacy `state.rs` wrapper;
- removed unused GL projection/frustum helper methods from the Vulkan/DX runtime baseline;
- added `CameraFrameHistory` for previous/current frame tracking, camera velocity and angular speed;
- integrated camera history into viewport layers through `CameraViewportManagerResource`;
- added sanitization and ergonomic helpers for camera input, rig, projection clipping and world-space movement.

Runtime rule:

```text
Camera history is explicit per-viewport data.
No render/backend code should reconstruct previous camera state from hidden globals.
```

## 2026-05-26 input capture contract pass

```text
listener alive = invariant
navigation gated = policy
```

UI capture must not unsubscribe or silence camera sampling. UI publishes capture state; the camera/input layer receives the sampled frame every tick and decides which deltas/actions may affect navigation or gameplay movement.
