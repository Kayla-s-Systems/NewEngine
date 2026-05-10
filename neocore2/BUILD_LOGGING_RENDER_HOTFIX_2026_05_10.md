# Build logging + VulkanRenderer hotfix — 2026-05-10

## Fixes

- Fixed `VulkanRenderer` compile error in `vulkan/renderer/init.rs` by importing the engine deterministic `HashMap` alias:
  `newengine_math::collections_prelude::NeHashMap as HashMap`.
- Removed unused legacy logging tee sink file and its unused ANSI stripping helper.
- Removed unused `winit-platform` config serialization helper that was only producing dead-code warnings.

## Build logs

`Plugins/build_all_plugins.cmd` now self-wraps through `Plugins/build_with_log.ps1` unless `NEWENGINE_PLUGIN_BUILD_LOG_ACTIVE=1` is already set.

Generated files:

```text
NewEngine/neocore2/logs/build/plugin-sync-YYYYMMDD-HHMMSS.log
NewEngine/neocore2/logs/build/plugin-sync-latest.log
```

`runGame.bat` also logs the game cargo run/build phase:

```text
NewEngine/neocore2/logs/run/game-ready-fps-YYYYMMDD-HHMMSS.log
NewEngine/neocore2/logs/run/game-ready-fps-latest.log
```

Both log paths are printed to console.

## Incremental plugin invalidation

`Plugins/plugin_needs_rebuild.ps1` no longer treats `NewEngine/neocore2/Cargo.lock` as a global invalidator by default. That lockfile changes for app/editor dependency updates and was forcing unrelated runtime DLLs to rebuild.

Set this when a full strict lock-based invalidation pass is needed:

```cmd
set NEWENGINE_STRICT_PLUGIN_LOCK=1
```
