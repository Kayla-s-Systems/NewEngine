# Input gamepad + CONFIG root pass — 2026-05-19

## Purpose

This pass separates disposable cache data from durable user settings and makes input device selection explicit.

```text
CACHE_FILES = generated disposable data, safe to clean
CONFIG      = durable user/player/editor settings, not safe to clean
```

## Engine root constants

`newengine-core` now exposes a CONFIG root on the same architectural level as CACHE_FILES:

```text
NEWENGINE_CONFIG
CONFIG
NEWENGINE_CONFIG_READY
```

Startup config supports both root-level and `engine.*` declarations:

```json
{
  "CONFIG": "config",
  "engine": {
    "config": "config",
    "cache_files": "cache"
  }
}
```

`StartupLoader` publishes both roots after config resolution. `CONFIG` is used by user-setting systems such as input bindings.

## Input device model

The input plugin supports a runtime device mode:

```text
keyboard_mouse
 gamepad
 hybrid
```

Hybrid is the default. Keyboard/mouse and gamepad can be active together, which allows a game to support FPS, third person and menu/gameplay interactions without forcing the user into a single device class.

## Gamepad backend safety

The input plugin uses `gilrs` with Windows XInput only:

```toml
gilrs = { version = "0.11.1", default-features = false, features = ["xinput"] }
```

This intentionally avoids the default Windows Gaming Input path, which can panic inside a background backend thread on some machines. Gamepad init and polling are also guarded so input backend failure degrades to no-gamepad instead of crossing the plugin FFI boundary as a panic.

## Bindings persistence

`engine.input.bindings` persists the gameplay bindings profile to:

```text
CONFIG/input/bindings.gameplay.json
```

The gateway remains engine-owned for now and supports:

```text
profile_json_v1
save_profile_json_v1
reset_profile_json_v1
```

The default profile is written once when no user profile exists. Later edits update the CONFIG file, not CACHE_FILES.

## Gamepad semantic actions

`newengine-input-bindings-api` + `newengine-input-actions-api` now support:

```text
GamepadButton bindings
GamepadAxis bindings
InputDevicePreference
InputActionFrame.look_axis
```

Default gameplay mappings include keyboard/mouse and gamepad:

```text
WASD / left stick        -> player movement
mouse / right stick      -> camera look
F / select / mode        -> camera.view.next
1/2/3 / dpad directions  -> camera view presets
left shift / left thumb  -> sprint
```

## Invariant

Gameplay should consume semantic actions, not physical device names. Physical keys/buttons/axes belong to `engine.input.bindings`; raw input belongs to `engine.input`; camera view switching belongs to `engine.camera`.
