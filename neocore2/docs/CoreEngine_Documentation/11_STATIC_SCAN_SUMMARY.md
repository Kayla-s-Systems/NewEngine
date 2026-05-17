# 11 — Static scan summary

## Current expected invariants

Engine/runtime/doc zones should not contain concrete native physics provider integration terms. Provider-specific notes may exist inside the provider plugin workspace under `Plugins/`.

Engine zones to scan:

```text
NewEngine/neocore2/Cargo.toml
NewEngine/neocore2/crates
NewEngine/neocore2/apps
NewEngine/neocore2/docs
```

Expected absent from engine/runtime:

```text
direct imports of concrete physics world services
direct runtime physics step over ECS World
&mut World passed into backend
filename-based physics provider detection
physics backend id branches in engine code
provider-specific native physics imports
```

Expected present:

```text
physics.api
physics.backend
PhysicsFrameInput
PhysicsFrameOutput
PhysicsApiRef
PhysicsBackendRuntimeModule
PhysicsSyncModule
HeightfieldColliderDto
MeshColliderDto
```

## Runtime evidence target

A healthy startup shows:

```text
loaded : 7
physics backend: service bridge bound ... capability:physics.backend+service:physics.api
runtime contract ok: service='physics.api'
```

This confirms the backend is visible as a service provider, not as an in-engine implementation.
