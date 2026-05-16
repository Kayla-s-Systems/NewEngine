# Physics service adapter pass — 2026-05-16

## Goal

Physics must be replaceable in the same architectural sense as render:

```text
Physics backend plugin
  provides capability physics.backend
  provides service physics.api
      ↓ HostApiV1.call_service_v1
PhysicsServiceClient
      ↓ typed adapter
ServiceBackedPhysicsApi : dyn PhysicsApi
      ↓ Resources.register_api(PHYSICS_API_ID)
PhysicsSyncModule
      ↓ ECS packet extraction/application
PhysicsFrameInput -> physics.api -> PhysicsFrameOutput
```

The engine is not a wrapper around Jolt. The engine owns `physics.api`; Jolt is one backend adapter for that API. Future Bullet, PhysX and Havok integrations must implement the same service contract instead of changing engine-runtime.

## Added crates and runtime modules

```text
NewEngine/neocore2/crates/newengine-physics-api
  stable physics.api service protocol
  PhysicsFrameInput / PhysicsFrameOutput packet boundary
  PhysicsBackendInfo / PhysicsBackendCapabilities
  negotiation, diagnostics and problem DTOs

NewEngine/neocore2/crates/newengine-core/src/physics
  PhysicsApi trait
  PhysicsApiRef
  PHYSICS_API_ID / PHYSICS_API_PROVIDE

NewEngine/neocore2/crates/newengine-runtime-host/src/physics_runtime
  PhysicsServiceClient
  ServiceBackedPhysicsApi
  PhysicsProviderResolver
  PhysicsBackendRuntimeModule
  ResolvedPhysicsBackendConfig
```

## ECS sync boundary

`newengine-engine-runtime/src/gameplay/physics.rs` is now a host-side sync adapter:

```text
ECS read side
  PhysicsBodyDesc
  Transform
  Velocity
  Bounds
      ↓
PhysicsFrameInput
      ↓
PhysicsApiRef.step_frame(...)
      ↓
PhysicsFrameOutput
      ↓
ECS write side
  Transform
  Velocity
  PhysicsStepReport
```

Backends no longer receive `&mut World`. ECS mutation stays in engine-runtime, on the host side of the service boundary.

## Removed hard runtime coupling

The reusable engine/runtime path no longer imports or calls:

```text
newengine_physics_runtime::PhysicsWorldService
PhysicsWorldStepSettings
step_runtime_physics(world, dt)
service.step(world, dt, settings)
```

`run_schedule(...)` now accepts an optional `PhysicsApiRef` and calls `step_service_physics(...)`.

## Backend provider plugins

```text
Plugins/PhysicsDeterministic
  plugin id: newengine.physics.deterministic
  provides service: physics.api
  provides capability: physics.backend
  packet deterministic/AABB implementation

Plugins/NullPhysics
  plugin id: newengine.physics.null
  provides service: physics.api
  provides capability: physics.backend
  explicit null provider; no hidden fallback

Plugins/JoltPhysics
  plugin id: newengine.physics.jolt
  provides service: physics.api
  provides capability: physics.backend
  adapter-shell provider for the future native Jolt mapping
```

The Jolt provider is intentionally only a service adapter shell in this pass. Native Jolt body/shape/contact/query mapping remains the next implementation step. This keeps the architectural dependency direction correct before native backend work expands.

## Provider selection

Plugin discovery now tracks physics service/provider metadata in the same shape as render:

```text
provides_physics_backend
provides_physics_service
```

Runtime target selection loads one active physics backend provider. Priority in this pass:

```text
1. newengine.physics.deterministic
2. other non-null native adapter providers, including newengine.physics.jolt
3. newengine.physics.null
```

`NullPhysics` is selected only when no non-null physics backend is available.

## Strict invariants after this pass

```text
engine-runtime imports physics API/contracts, not provider implementations
no physics backend receives ECS World
Null physics is a real service provider plugin
backend selection is by physics.backend + physics.api provider metadata
Jolt is an adapter behind physics.api, not a type the engine wraps
```

## Remaining work

```text
P0 follow-up:
  implement native Jolt mapping inside Plugins/JoltPhysics
  add physics.api conformance/replay tests
  add startup contract/profile requirement for physics.api when a profile needs physics
  move discovery from id-pattern fallback to manifest/capability-first metadata only

P1 follow-up:
  support hot-path binary packet transport if JSON becomes too expensive
  add query result mapping and contact event roundtrip in PhysicsSyncModule
  add debug draw extraction through physics.api diagnostics
```
