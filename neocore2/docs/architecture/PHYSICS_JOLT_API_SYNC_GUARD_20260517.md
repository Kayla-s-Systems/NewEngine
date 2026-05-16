# Physics Jolt API sync guard — 2026-05-17

## Problem

`Plugins/JoltPhysics` depends on the current `newengine-physics-api` capability surface.
The Jolt provider reports native capabilities through:

```rust
PhysicsBackendCapabilities::jolt_native_default()
```

A stale source tree where `newengine-physics-api` still exposes only
`jolt_adapter_skeleton()` produces this compiler error:

```text
error[E0599]: no function or associated item named `jolt_native_default`
```

## Fix

The current API crate owns the native Jolt capability constructor and the formal
terrain/static collider DTOs:

```text
HeightfieldColliderDto
MeshColliderDto
PhysicsColliderDto
PhysicsFrameColliderSnapshot
PhysicsBackendCapabilities::jolt_native_default()
```

`Plugins/build_all_plugins.cmd` now performs a Jolt-specific preflight check before
running Cargo. If the repository root being built is stale or mismatched, plugin
sync fails immediately with a source-sync diagnostic instead of letting Cargo fail
later inside `newengine-physics-jolt-adapter`.

## Architecture note

No compatibility alias is reintroduced. `jolt_adapter_skeleton()` remains removed.
The active provider is `newengine.physics.jolt`, which reports `physics.api` and
`physics.backend` through the native capability set.
