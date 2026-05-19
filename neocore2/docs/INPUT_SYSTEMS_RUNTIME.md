# Input Systems Runtime

`engine.input` is not a single switch. Runtime control is split into explicit systems so gameplay can disable exactly the layer it owns without destroying raw device state.

## Systems

| System id | Owner | Meaning | Typical use |
| --- | --- | --- | --- |
| `engine.input.raw` | `newengine.input` | Raw keyboard, mouse and gamepad polling from the active input backend. | Usually enabled for the whole process. Disable only for hard input lock. |
| `engine.input.bindings` | `newengine-input-bindings-runtime` | Active profile and binding resolver. | Disable when semantic actions must not be emitted. |
| `engine.input.actions` | `newengine-input-actions-api` | Semantic action frame after profile resolution. | Disable to keep raw input available while blocking gameplay/UI actions. |
| `engine.input.contexts` | `newengine-input-contexts-api` | Declared future context/capture stack. | Cutscenes, modal UI, dialogue, photo mode, console. Currently logged as declared/inactive until the context runtime is installed. |
| `engine.input.gamepad` | `newengine.input/gilrs` | Gamepad visibility and activity. | Device diagnostics and optional gamepad-specific suppression. |
| `engine.input.camera_look` | `newengine-camera-runtime` | Camera look deltas from mouse/gamepad. | Disable during cutscenes, scripted camera, dialogue, loading locks. |
| `engine.input.gameplay_movement` | `newengine-gameplay.player-controller` | Player motor movement/sprint semantic effects. | Disable during cutscenes, menus, interactions, scripted movement. |
| `engine.input.pause_menu` | `newengine-ui-menu-runtime` | Pause/menu toggle, navigation and accept/back actions. | Disable for unskippable sequences or when another modal surface captures input. |

## Initialization order

GameReady installs its input profile before constructing `RuntimeRenderController`. This is important because the pause-menu runtime snapshots the active profile during construction. If the bindings gateway were initialized with an empty generic profile first, keyboard/gamepad gameplay actions would resolve to an empty action frame while raw mouse-look could still appear to work.

Expected startup logs:

```text
input bindings: initialized profile id='newengine.gameready.input.profile' actions=... bindings=... axes=...
input bindings gateway: engine-owned route registered id='engine.input.bindings' ...
input systems: initialized systems=8 enabled=7 disabled=[engine.input.contexts]
input systems: raw input polling online service='engine.input' method='state_json'
# DEBUG only: input systems: transition system='engine.input.gameplay_movement' active false->true ...
# DEBUG only: input systems: frame=... raw=true keys_down=... gamepads=... actions=... move=...
```

If `actions=0`, `bindings=0` or `raw=false` appears after the world is running, the fault is now localized:

- `raw=false`: input provider/service polling is unavailable.
- `raw=true`, `keys_down>0`, `actions=0`: binding profile/action resolution is wrong.
- `actions>0`, `move_mask=0`: bindings resolve, but movement actions are not mapped to movement effects.
- `move_mask!=0`, player does not move: gameplay movement/physics/motor system is disabled or broken.

## Enabling and disabling systems

At runtime, `RuntimeRenderController` exposes:

```rust
use newengine_engine_runtime::input_systems::InputRuntimeSystem;

render_controller.set_input_system_enabled(
    InputRuntimeSystem::GameplayMovement,
    false,
    "cutscene captures player movement",
);

render_controller.set_input_system_enabled(
    InputRuntimeSystem::CameraLook,
    false,
    "scripted camera owns view",
);

render_controller.set_input_system_enabled(
    InputRuntimeSystem::GameplayMovement,
    true,
    "cutscene finished",
);
render_controller.set_input_system_enabled(
    InputRuntimeSystem::CameraLook,
    true,
    "cutscene finished",
);
```

The raw device layer stays alive unless `InputRuntimeSystem::RawInput` is disabled. This allows UI prompts, debug overlays, telemetry and device diagnostics to keep seeing keyboard/gamepad state while gameplay movement is captured.

## Pause-menu capture

`engine.pause_menu` is implemented as a modal input capture:

1. pause/menu actions are evaluated first;
2. if the menu is open, it captures `engine.input.gameplay_movement` and `engine.input.camera_look`;
3. raw input and pause/menu navigation remain enabled.

This is the same model future cutscenes should use: do not delete input, do not mutate player controller directly, capture or disable the semantic systems that should not produce effects.

## Snapshot logging

Use:

```rust
render_controller.log_input_systems_snapshot("before cutscene");
let snapshot = render_controller.input_systems_snapshot();
```

The snapshot records `enabled`, `active`, `captured`, `reason` and the frame index for every system. Normal startup emits only a compact catalog line; transition/frame summaries are `DEBUG` and sampled/deduplicated so pause-menu capture cannot spam `captured=false/true` every frame.
