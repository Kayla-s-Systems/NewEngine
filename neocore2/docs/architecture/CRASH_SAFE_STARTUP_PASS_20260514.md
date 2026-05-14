# Crash-safe startup pass — 2026-05-14

## Problem
`game-ready-fps` can terminate with `STATUS_ACCESS_VIOLATION (0xC0000005)` before normal engine logs are flushed. This usually means the process crosses an unsafe dynamic-library/ABI boundary before diagnostics are available.

## Applied hardening

### 1. Game launcher early log
`apps/game-ready-fps/src/main.rs` now writes a direct file log before normal logger/plugin services are initialized.

Default candidates:
- `NewEngine/neocore2/logs/runtime/game-ready-early.log`
- `logs/runtime/game-ready-early.log`
- `game-ready-early.log`

Override:

```bat
set NEWENGINE_GAME_READY_EARLY_LOG=C:\temp\game-ready-early.log
```

The log marks each critical startup boundary:
- app entry
- startup config load
- engine construction
- runtime profile registration
- bootstrap plugin preload
- platform runtime detection
- platform config resolve
- host runtime creation
- runtime FFI run call

### 2. Plugin-host scan profile filter before `Library::new`
`newengine-plugin-host` now filters obvious profile-mismatched DLLs before `dlopen`/`LoadLibrary`.

Example: a debug/dev app no longer probes `*-release.dll` unless explicitly allowed.

Override:

```bat
set NEWENGINE_ALLOW_MIXED_PLUGIN_PROFILE=1
```

### 3. Platform runtime is no longer probed as a normal plugin
`platform-winit` is marked as a platform runtime by filename during plugin-host discovery. The bootstrap plugin scanner no longer calls `Library::new` / `export_plugin_root` on it.

The platform runtime is loaded only by `newengine-runtime-host` at the actual platform handoff point.

### 4. Platform runtime discovery no longer loads every candidate by default
`detect_platform_runtime_path()` now uses filename/profile filtering first and does not call platform metadata ABI by default.

Optional unsafe diagnostics:

```bat
set NEWENGINE_PLATFORM_RUNTIME_VALIDATE_SYMBOL=1
set NEWENGINE_PLATFORM_RUNTIME_METADATA_PROBE=1
```

### 5. Platform config resolve no longer calls plugin metadata by default
`resolve_platform_runtime_config()` now uses host-side startup config + plugin overrides without calling `export_plugin_root`.

Optional legacy metadata path:

```bat
set NEWENGINE_PLATFORM_CONFIG_METADATA_PROBE=1
```

## Diagnosis rule
After a crash, inspect `game-ready-early.log` first. The last line identifies the earliest failing stage.

| Last line | Likely fault boundary |
|---|---|
| `engine.preload_bootstrap_plugins.begin` | plugin scan/load before platform runtime |
| `platform.detect.begin` | platform DLL discovery |
| `platform.config.resolve.begin` | platform config/metadata path |
| `runtime.run.begin` | actual platform runtime FFI call |
| no file at all | crash before Rust `main()` or file cannot be created |
