# Camera Runtime Session Service Pass — 2026-05-12

## Goal

Move the remaining camera/session side effects out of `render_controller` and into the camera runtime layer.

## Implemented

### 1. Camera session ownership

Added `newengine-camera-runtime/src/session.rs`:

- `CameraRuntimeSessionMode`
- `CameraPlaySessionSnapshot`
- `CameraRuntimeSessionResource`
- `CameraRuntimeSessionService`

`RuntimeRenderController` no longer stores:

- `last_play_mode`
- `play_session`
- `runtime_session`
- `PlaySessionSnapshot`

The controller now maps `EditorPlayMode` into `CameraRuntimeSessionMode` and delegates camera session transitions to camera-runtime.

### 2. Runtime world snapshot resource

Runtime world snapshot state moved from render-controller fields to ECS resource:

- `RuntimeWorldSessionResource`
- `capture_runtime_world_session(...)`
- `restore_runtime_world_session(...)`

This keeps gameplay rollback state with the world, not with the renderer.

### 3. Scene/profile camera config

Game-ready profiles now support declarative camera selection:

```json
"camera": {
  "mode": "first_person",
  "first_person": { "eye_height": 0.85 },
  "third_person_follow": { "offset": [0.35, 1.65, 4.5], "smooth_time": 0.08, "max_speed": 0.0 },
  "third_person_aim": { "offset": [0.55, 1.55, 2.2], "smooth_time": 0.035, "max_speed": 0.0 },
  "spring_arm": { "enabled": true, "probe_radius": 0.18, "collision_padding": 0.08, "min_distance": 0.75 }
}
```

The profile is converted into `CameraRuntimeProfileConfig` and inserted as a world resource.

### 4. Third-person spring-arm constraints

Added `newengine-camera-runtime/src/constraints.rs`:

- `CameraSpringArmConfig`
- `CameraSpringArmCollider`
- `CameraSpringArmCollisionWorld`
- `constrain_spring_arm_offset_ls(...)`

The render/runtime layer builds an engine-specific collision proxy list from `CollisionBody + Transform`; camera-runtime remains collision-model agnostic.

### 5. Cinematic/scripted director runners

Added runtime-level director runner placeholders:

- `CinematicDirectorRunner`
- `ScriptedDirectorRunner`

`CameraDirectorRequest` now supports direct pose requests for cinematic/scripted camera systems.

## Result

`render_controller` now owns rendering orchestration only. Camera session ownership, play-camera snapshotting, possession/release and gameplay camera mode configuration live in camera-runtime/world resources.

## Next pass

- move `RuntimeWorldSessionResource` from engine-runtime gameplay into a more general runtime session crate;
- add broadphase-backed spring-arm collision source instead of per-frame full collision-body scan;
- expose scripted/cinematic camera tracks through scene/profile assets;
- add overlay fields for session transition reports.
