# Input key registry and model adapter pass — 2026-05-19

## Input problem fixed

`Escape` must not be a platform hardcoded close shortcut and it must not depend on native enum numeric casts.

Target/current flow:

```text
winit native key
  -> explicit platform key map
  -> canonical engine key code
  -> engine.input.bindings key registry
  -> binding registry
  -> action dispatch frame
  -> pause-menu listener consumes engine.menu.toggle_pause
```

The platform provider now maps `winit::keyboard::KeyCode` variants explicitly to `newengine-input-bindings::key_code` constants. It no longer casts `KeyCode as u32` because native discriminants are not an engine ABI.

## Registry shape

`engine.input.bindings` owns explicit registrations for:

```text
keys      -> canonical keyboard code/id/label
bindings  -> keyboard/mouse/gamepad input to action
axes      -> gamepad axis to semantic axis target
actions   -> action id plus effect descriptors
listeners -> owner/id/priority/consumption
```

Provider/gameplay/UI code should register through manifest APIs instead of hardcoded local tables:

```text
register_key_json_v1
register_action_json_v1
register_binding_json_v1
register_listener_json_v1
register_manifest_json_v1
key_catalog_json_v1
action_catalog_json_v1
```

`engine.menu.toggle_pause` is a normal binding target. Default migration guarantees that a keyboard pause binding and a gamepad pause binding exist if an older saved profile lacked them:

```text
ESC   -> engine.menu.toggle_pause
Start -> engine.menu.toggle_pause
```

`ESC` is explicitly removed from `ui.menu.back` to avoid double ownership.

## Model adapter problem fixed

The player model path was becoming a local hand-assembled loader inside `newengine-engine-runtime`:

```text
OBJ parser + MTL parser + YMT probe + neytd selector derivation
```

That logic now has a dedicated reusable crate:

```text
crates/newengine-model-adapter
```

Target/current flow:

```text
GameReadyPlayerModelSpec
  -> ModelAssetRequest
  -> ModelAssetAdapter
  -> AssetManager logical model path
  -> mesh parts
  -> skeleton metadata
  -> material bindings
  -> .neytd@entry texture refs
  -> engine-runtime ECS visual binding
```

The adapter owns logical path validation, OBJ/MTL parsing, `.ymt` metadata probing, humanoid anchor fallback, material descriptor generation and `.neytd` dictionary selector derivation. Engine-runtime only registers returned meshes/materials into runtime registries and attaches the visual entity hierarchy.

## Important invariant

```text
Input owns control declarations.
Model adapter owns asset hierarchy resolution.
AssetManager owns bytes.
Material registry owns runtime material ids.
Renderer owns GPU realization.
Gameplay/runtime owns entity attachment only.
```
