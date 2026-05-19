# Input bindings dispatch and player skeleton bootstrap — 2026-05-19

## Input invariant

`engine.input.bindings` is the single declaration surface for semantic input.

Systems that want a key, mouse button, gamepad button or gamepad axis do not poll raw platform events directly and do not hard-code action ids in platform code. They register a manifest:

```text
InputBindingsManifest
  actions[]   -> semantic action definitions and effects
  bindings[]  -> keyboard/mouse/gamepad bindings
  listeners[] -> owner/id/priority/consume metadata
  axes[]      -> gamepad axis mappings
```

Runtime resolution is data-driven:

```text
platform-winit raw input
  -> engine.input
  -> engine.input.bindings profile
  -> InputActionFrame actions/events/effects
  -> listeners consume/observe by priority
  -> gameplay, camera and UI read semantic effects
```

## ESC policy

Escape is no longer a platform root shortcut. It must reach the engine input stream and is bound by default to:

```text
engine.menu.toggle_pause
```

The platform close button/window close request still uses the native close lifecycle. ESC is normal gameplay/UI input, not process/window lifecycle.

## Default listener tiers

```text
newengine-ui:pause-menu              priority=100 consume=true
newengine-camera-runtime:camera-view priority=50
newengine-gameplay:player-controller priority=10
```

The profile keeps these as declarative listener registrations. Additional plugins or runtime systems can register their own listeners through `register_manifest_json_v1` or `register_listener_json_v1`.

## Gamepad support

Default profile includes gamepad buttons and axes:

```text
Start      -> engine.menu.toggle_pause
South      -> engine.menu.accept
East       -> engine.menu.back
DPad       -> menu navigation and camera view shortcuts
LeftStick  -> movement axis
RightStick -> look axis
```

No gamepad-specific action branches should be added to gameplay systems. Device differences belong in binding/axis data.

## Player skeleton bootstrap

The player profile now supports a skeleton metadata asset:

```json
"model": {
  "source": "player/abigail/csb_abigail_static_y_up.obj",
  "texture_dictionary": "player/abigail/textures/abigail.neytd",
  "skeleton": "player/abigail/csb_abigail.ymt",
  "target_height": 1.78,
  "eye_height_ratio": 0.91
}
```

Current `.ymt` support is intentionally conservative:

```text
logical VFS path
  -> engine.assets raw_bytes_v1
  -> RSC7/YMT container detection
  -> content hash + byte metadata
  -> humanoid anchor skeleton derived for camera/attachment points
```

This gives the engine a stable skeleton metadata boundary now without pretending the compressed/native RSC payload is fully decoded. Future work can replace the fallback humanoid anchors with decoded native bone hierarchy, transforms, skin clusters and animation retarget data behind the same model/skeleton metadata surface.

## Runtime anchor rule

The first-person camera uses the model/skeleton eye anchor when available. Without a decoded native skeleton, the runtime derives `eye_center` from `target_height * eye_height_ratio`, then stores it in `PlayerModelBinding.feet_to_eye_height`.
