# Architecture cleanup: storage roots and input API extraction

Date: 2026-05-19  
Status: implemented refactor pass

## Purpose

This pass removes repeated root-resolution code, extracts the input service contract into a real API crate and keeps the application/runtime path aligned with the CoreEngine rule:

```text
Engine owns lifecycle, roots, gateways and validation.
Provider owns backend implementation.
Domain API crates own stable constants and runtime contract specs.
```

## Storage root cleanup

`CACHE_FILES` and `CONFIG` are now both described by the same internal root specification type:

```text
newengine-core::storage_root::EngineStorageRootSpec
```

Domain wrappers remain explicit:

```text
newengine-core::cache_files
newengine-core::config_root
```

The wrappers preserve semantics:

```text
CACHE_FILES -> disposable generated data, safe to clean
CONFIG      -> durable user settings, not disposable
```

This removes duplicated resolver/publisher/path-canonicalization code from `cache_files.rs` and `config_root.rs` while preserving clear public API names for each root.

## Startup config cleanup

Startup config now has a small data enum for engine-owned roots:

```rust
StartupStorageRootKind::CacheFiles
StartupStorageRootKind::Config
```

The loader applies root-level and `engine.*` root paths through one declarative path instead of parallel hard-coded `cache_files` and `config` loops.

## Input API extraction

A new engine crate owns the second-level input gateway contract:

```text
crates/newengine-input-api
```

It defines:

```text
ENGINE_INPUT_SERVICE_ID      = engine.input
INPUT_SERVICE_ID             = newengine.input.v1
INPUT_BACKEND_CAPABILITY_ID  = input.backend
INPUT_RUNTIME_REQUIREMENT_SPEC
```

`newengine-core` startup validation now consumes this contract instead of keeping local input string literals. The input plugin also depends on the API crate for service ids, capability ids and method names.

## Input bindings cleanup

`newengine-input-bindings-api` keeps the third-level data/config DTO domain:

```text
engine.input.bindings
```

The default gameplay profile is split into dedicated keyboard, gamepad-button and gamepad-axis builders. This keeps the future settings UI/editor work from editing one large mixed literal block.

## Legacy cleanup

The old internal names `*_ENV_LEGACY` were removed. `CACHE_FILES` and `CONFIG` are not legacy: they are intentionally supported alias constants alongside `NEWENGINE_CACHE_FILES` and `NEWENGINE_CONFIG`.

## Resulting layering

```text
newengine-input-api       -> input gateway contract
newengine-input-bindings-api      -> binding/profile DTO domain
newengine-input-bindings-runtime  -> engine.input.bindings service host
newengine-input-profile-gameready -> GameReady FPS default actions/bindings
newengine-core            -> startup validation + CONFIG/CACHE root publication
InputPlugin               -> provider implementation behind engine.input
engine-runtime/gameplay   -> consumes semantic actions, not physical keys
```
