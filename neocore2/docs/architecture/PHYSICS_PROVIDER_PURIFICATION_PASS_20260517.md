# Physics provider purification pass — 2026-05-17

## Goal

The engine must own only the stable physics protocol and host-side adapter.
Concrete physics backend implementation details belong to provider plugins.

Target invariant:

```text
engine/runtime/core
  depends on newengine-physics-api only
  registers/uses PhysicsApiRef
  performs ECS <-> PhysicsFrameInput/Output synchronization
  does not import or describe concrete physics providers

provider plugin
  declares physics.api service
  declares physics.backend capability
  maps DTOs to its private backend implementation
```

## Cleanup

Backend-specific implementation crates are not engine workspace members.
Concrete provider code lives under `Plugins/` and is built by the plugin build
pipeline.

Engine-facing physics API uses backend-neutral capability vocabulary:

```text
PhysicsBackendClass::Null
PhysicsBackendClass::Deterministic
PhysicsBackendClass::Native

PhysicsFeature::NativeBackend
PhysicsFeature::HeightfieldColliders
PhysicsFeature::MeshColliders
```

The API does not expose provider names, vendor names or provider-specific
constructors.

## Discovery contract

Physics selection is capability/service based:

```text
provides service: physics.api
provides capability: physics.backend
backend_priority: provider-owned selection hint
```

Plugin-host selection must not encode concrete physics backend file names or ids.
If a backend needs to be discoverable before ABI probing, that metadata must come
from runtime plugin manifest data produced by the plugin pipeline, not from
engine hardcoded backend lists.

## Runtime contract

Runtime modules bind through the same adapter shape as render:

```text
Physics provider plugin
  -> physics.api service
  -> PhysicsServiceClient
  -> ServiceBackedPhysicsApi
  -> PhysicsApiRef
  -> PhysicsSyncModule
```

The provider receives DTO packets and returns DTO packets. All ECS mutation stays
in host-side sync/apply code.
