# Plugin / renderer / importer boundary rework — 2026-05-10

## Intent

The executable remains a thin entry point. Runtime-heavy systems are loaded from DLLs, while AssetManager importer DLLs are treated as AssetManager-owned workers, not engine plugins.

## Rules

1. `newengine-plugin-host` scans only the runtime plugin root.
2. `plugins/importers/*.dll` is never scanned by `newengine-plugin-host`.
3. Asset importer workers are loaded only by `newengine.assets` through its private `ImporterLoader`.
4. `GameReadyMapPlugin` is removed from the runtime plugin graph. Game-ready map data is engine-owned declarative content / standalone profile data.
5. `game-ready-fps` requires a real renderer. If `newengine.renderer.vulkan` is unavailable, the app fails fast instead of silently running with `newengine.renderer.null`.
6. `Plugins/build_all_plugins.cmd` performs incremental sync: build only missing/stale DLLs and remove stale/deprecated DLLs.

## Runtime consequences

Expected plugin host table no longer contains:

- `import.audio`
- `import.font`
- `import.3d`
- `import.image`
- `import.text`
- `newengine.game_ready.map`

Expected plugin host table contains, when built:

- `newengine.logging`
- `newengine.input`
- `newengine.platform.winit`
- `newengine.assets`
- `newengine.renderer.vulkan`

AssetManager still logs importer worker discovery internally, for example:

- `kalitech.import.audio.v1`
- `kalitech.import.image.v1`
- `kalitech.import.3d.v1`

Those services are private to AssetManager and are not globally registered as engine plugins.
