# NewEngine Camera Runtime Direction

This document captures the intended growth direction for the NewEngine camera layer based on the attached camera reference archive. It is architectural guidance only; the engine should keep its own Rust-native contracts and avoid copying implementation details.

## Current problem

The current camera path is still too tightly coupled to the render controller and play-mode switch. A AAA-grade runtime should not treat camera as a single fly/orbit helper. It needs a camera system with explicit ownership, modes, directors, blending, input contexts, and runtime gates.

## Target shape

```text
newengine-camera-contracts/
  camera_id.rs
  camera_pose.rs
  camera_lens.rs
  camera_mode.rs
  camera_context.rs
  camera_director.rs
  camera_transition.rs
  camera_stack.rs

newengine-camera-runtime/
  manager.rs
  director_registry.rs
  active_camera_stack.rs
  transitions.rs
  shake.rs
  collision.rs
  constraints.rs
  input_context.rs
  modes/
    debug_free.rs
    editor_orbit.rs
    editor_fly.rs
    first_person.rs
    third_person_follow.rs
    third_person_aim.rs
    cinematic.rs
    scripted_spline.rs
    replay.rs
```

## Required concepts

### Camera Manager

Owns the active camera stack, current output pose, lens, transition state, and debug telemetry. Render should consume `CameraFrame`, not decide camera policy.

### Directors

A director is a high-level owner of camera behavior:

- editor director;
- gameplay director;
- cinematic director;
- scripted director;
- replay director;
- debug director.

Only one director should own the final camera output at a time, but directors may request transitions through the manager.

### Contexts

Camera behavior should be context-driven, not hard-coded:

- on-foot first person;
- on-foot third person;
- aiming;
- melee;
- vehicle;
- spectator;
- cutscene;
- debug free camera.

The context decides input mapping, constraints, collision rules, damping, FOV policy, and shake policy.

### Transitions and blends

Camera switches must be explicit transitions:

```text
from camera A
  to camera B
  blend curve
  duration
  interruption policy
  input lock policy
```

No instant implicit camera snapping during play-mode changes except for fatal recovery/debug paths.

### Launch-gate integration

The camera must respect scene readiness:

```text
CPU scene ready
GPU resources resident
camera target valid
player control allowed
physics allowed
```

Before launch gate release, gameplay cameras must not possess the player. The active camera may remain editor/debug/loading camera or a deterministic preview camera.

## Next implementation pass

1. Move camera policy out of `render_controller/module_impl/mod.rs` into camera runtime services.
2. Add `CameraManagerResource` to the scene/runtime world.
3. Replace direct `attach_active_camera_to_player` calls with `CameraDirectorRequest::PossessPlayer`.
4. Add `CameraTransitionPlan` for Play activation.
5. Add diagnostics:

```text
CameraRuntimeReport {
  active_director,
  active_mode,
  target_entity,
  transition_state,
  input_context,
  gate_blocked,
}
```

## Anti-goals

- Render controller must not decide gameplay camera mode.
- Camera code must not start gameplay simulation.
- Player spawn must not imply player possession.
- Public `Play` mode must not be exposed before launch gate release.
