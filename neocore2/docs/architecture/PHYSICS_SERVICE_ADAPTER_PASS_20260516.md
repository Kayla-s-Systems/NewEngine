# Physics service adapter pass — 2026-05-16

## Goal

Physics must follow the same provider/adapter shape as render, but without
embedding any concrete physics backend into engine/runtime/core crates.

```text
Physics provider plugin
  provides service physics.api
  provides capability physics.backend
      ↓
PhysicsServiceClient
      ↓
ServiceBackedPhysicsApi : dyn PhysicsApi
      ↓
Resources.register_api(PHYSICS_API_ID)
      ↓
PhysicsSyncModule
      ↓
ECS <-> PhysicsFrameInput/PhysicsFrameOutput
```

## Engine-owned pieces

The engine may own:

```text
newengine-physics-api
  service id
  protocol version
  backend-neutral DTOs
  request/response envelopes
  diagnostics/problem schema

newengine-core::physics
  PhysicsApi trait
  PhysicsApiRef resource

newengine-runtime-host::physics_runtime
  PhysicsServiceClient
  ServiceBackedPhysicsApi
  PhysicsBackendRuntimeModule
  provider capability validation

newengine-engine-runtime physics sync
  ECS extraction
  PhysicsFrameInput construction
  PhysicsFrameOutput apply policy
```

The engine must not own provider-native body/world/shape logic.

## Provider-owned pieces

A concrete physics backend lives under `Plugins/` and owns:

```text
private native/backend implementation crates
ServiceV1 implementation for physics.api
provider descriptor
provider capabilities
provider config
backend DTO -> native mapping
native step/query/event implementation
```

## Frame boundary

Backends receive packets, never engine internals:

```text
bad:
  step(&mut World)
  store concrete backend world services in ECS resources
  expose native body handles to engine runtime

good:
  step(PhysicsFrameInput) -> PhysicsFrameOutput
```

## Discovery

Physics backend selection is driven by descriptor/capability/service metadata:

```text
service: physics.api
capability: physics.backend
backend_priority: provider-owned selection hint
```

Plugin-host must not encode provider-specific physics ids, filenames or vendor
names. Provider identity comes from ABI descriptor probing. Runtime plugin manifests are not used for backend identity.

## Acceptance criteria

```text
engine-runtime does not import backend implementation crates
runtime uses PhysicsApiRef only
providers implement physics.api
no &mut World crosses provider boundary
null/noop physics, deterministic physics and native physics are provider plugins,
not hidden runtime fallbacks
```
