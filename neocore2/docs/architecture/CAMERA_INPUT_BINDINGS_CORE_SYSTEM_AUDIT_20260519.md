# Camera / input bindings / Core-System audit — 2026-05-19

## Runtime evidence

Current launch proves the service host shape is healthy: `engine.camera`, `engine.loading`, `engine.ecs`, `engine.entity`, `engine.scene` and `engine.platform` are engine-owned route candidates, while render/physics/assets/input/ui/log are routed through provider services. The camera gateway passes runtime contract validation as `engine.camera`.

The remaining camera gap was not gateway routing. It was domain behavior: gameplay input still reached camera/player movement as a raw movement bitmask assembled from physical keys, and the playable camera had no first-class view-mode request path.

## camera.zip comparison

The reference camera package is structured around several concepts NewEngine should converge toward:

```text
base directors / camera interface
runtime/gameplay/cinematic/debug/cutscene directors
third-person camera implementation
frame helpers: damped spring, catch-up, interpolator, propagator
shakers/envelopes/hand/animated frame effects
collision/control helpers
```

NewEngine already had the right high-level direction:

```text
newengine-camera-api          gateway-facing DTOs
newengine-camera-runtime      manager, directors, transitions, viewport, nav
newengine-camera              frames, rigs, projections, modifiers, effects
engine.camera                 engine-owned route candidate
```

The missing practical pieces for this pass were:

- view switching as a camera-domain command;
- gameplay mode selection between FPS and third person;
- semantic input actions instead of direct key checks;
- camera frame snapshot carrying the active view mode for render/debug/UI diagnostics.

## Implemented in this pass

### Third-level input domains

Added engine-owned crate:

```text
crates/newengine-input-bindings
```

It establishes the third-level domain pattern:

```text
engine.input.bindings  -> data/configuration profile
engine.input.actions   -> semantic interpretation output
```

The binding profile maps physical inputs to semantic actions:

```text
player.move.forward
player.move.back
player.move.left
player.move.right
player.move.up
player.move.down
player.sprint
camera.view.next
camera.view.first_person
camera.view.third_person.follow
camera.view.third_person.aim
```

Gameplay/camera systems now consume semantic action output and compatibility movement masks from this crate instead of hard-coding W/A/S/D/Q/E/Shift in the engine runtime input projection.

### Engine-owned input bindings gateway

Added `engine.input.bindings` as an engine-owned gateway service in `newengine-engine-runtime`:

```text
service id: engine.input.bindings
capability: input.bindings.backend
kind: input.bindings
owner: newengine-engine-runtime.input-bindings-gateway
```

This keeps bindings as engine-owned domain data now, while leaving the same route mechanism open for future profile/plugin overrides.

### Camera view switching

Added `CameraViewMode` to `newengine-camera-api`:

```text
FirstPerson
ThirdPersonFollow
ThirdPersonAim
```

Added view commands:

```text
CameraViewCommand::Next
CameraViewCommand::Previous
CameraViewCommand::Set(CameraViewMode)
```

`engine.camera` now exposes:

```text
snapshot_json_v1
view_set_json_v1
view_next_json_v1
invoke_json   # empty payload remains snapshot-compatible; command payload applies view command
```

The direct gameplay input path maps:

```text
F      -> camera.view.next
1      -> camera.view.first_person
2      -> camera.view.third_person.follow
3      -> camera.view.third_person.aim
```

### Runtime camera mode policy

`CameraManagerResource` now tracks `view_mode`. When the view changes while the player is possessed, it emits a new possess request so `CameraRuntimeService` can reconfigure the active follow controller.

Mapping:

```text
CameraViewMode::FirstPerson        -> GameplayCameraRunnerKind::FirstPerson
CameraViewMode::ThirdPersonFollow  -> GameplayCameraRunnerKind::ThirdPersonFollow
CameraViewMode::ThirdPersonAim     -> GameplayCameraRunnerKind::ThirdPersonAim
```

Camera snapshots now include the active view mode, and render/debug diagnostics report it.

## Core.zip / System.zip comparison

The reference Core/System layout emphasizes:

```text
core lifecycle / game session state machine
system-level task scheduling
settings/mapping manager
control/pad/keyboard mapping layer
file/data managers
benchmark/perf/GPU capture hooks
platform and entitlement isolation
```

NewEngine matches several architectural principles already:

- service host rather than concrete backend wrapper;
- startup graph and runtime contract validation;
- gateway registry and provider selection;
- engine-owned route candidates for transitional built-ins;
- plugin-backed render/physics/assets/input/ui/logging;
- thin app entrypoint after launcher refactor.

Remaining big-world/performance gaps:

1. **Scheduler maturity** — current startup graph and runtime modules are good, but large worlds need clearer task classes, priorities, budgets and frame-lane scheduling.
2. **Input configuration maturity** — this pass starts the equivalent of a mapping manager with `engine.input.bindings`; next should add persistent user profiles and rebinding UI.
3. **Streaming budget model** — terrain streaming exists, but frame logs still show extraction spikes; chunk/draw-list caches and dirty-region scheduling are needed.
4. **Camera helper parity** — third-person modes exist now, but the camera still needs more helper-level features from the reference: collision probe, damped catch-up, shoulder swapping, obstruction fade, and per-view tuning assets.
5. **System diagnostics** — current logs are strong, but System.zip-style benchmark/perf capture should become first-class `engine.diagnostics` / `engine.profiler` domains later.

## Next recommended pass

```text
1. Add camera view tuning profile assets.
2. Add third-person collision/obstruction probe.
3. Add DampedSpring/CatchUp helper module to newengine-camera-runtime.
4. Add persistent input binding profile loading/saving through engine.input.bindings.
5. Add engine.input.actions as a live frame service if plugins/tools need to inspect resolved actions.
```
