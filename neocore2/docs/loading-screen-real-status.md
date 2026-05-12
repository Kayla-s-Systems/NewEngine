# Loading Screen Real Status Pipeline

The native bootstrap loading screen is intentionally **not** a fake percentage animation.
It is driven by the core lifecycle FSM and startup snapshots:

```text
newengine-core::EngineFsm
  -> Engine::start_incremental_step()
  -> EngineStartupSnapshot resource
  -> newengine-system-runtime::overlay_from_engine_startup_snapshot()
  -> PlatformLoadingOverlayV1.view_json
  -> winit-platform native loading renderer
```

## Rules

- The core FSM is the single lifecycle truth.
- Runtime host observes `Engine::startup_status()` and `Engine::run_state()`; it does not invent a second boot state.
- Platform loading renderer only renders `PlatformLoadingOverlayV1`; it does not infer subsystem status from the percentage.
- Assets for the screen are resolved through AssetManager from `neocore2/assets/loading`.
- Fatal startup errors keep the native loading screen alive and render safe-stop diagnostics instead of collapsing into a blank window.

## Subsystems shown on the loading screen

| Card | Source |
|---|---|
| PLATFORM | Window/surface callback and runtime-host state |
| ASSETS | engine plugin service count and AssetManager/importer availability |
| RENDERER | renderer backend label/status resource |
| SIMULATION | core FSM phase, module init/startup graph status |
| DIAGNOSTICS | current startup phase, current module, exact error context |

## Why startup is incremental

`Engine::start()` still exists as a compatibility API, but internally it drives
`Engine::start_incremental_step()` to completion. Platform runtimes should call
`start_incremental_step()` once per host tick so the splash screen can repaint
between expensive module/plugin phases.
