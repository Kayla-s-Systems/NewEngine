# 02 — Гайд по имеющемуся функционалу

## 1. Workspace

Main workspace:

```text
NewEngine/neocore2/Cargo.toml
```

Runtime plugins live outside the main engine workspace:

```text
Plugins/
Importers/
tools/
```

This is intentional. Engine crates own stable contracts; provider plugins own implementation.

## 2. Core engine

`newengine-core` provides:

- engine lifecycle and state transitions;
- module ordering and `Module<E>` lifecycle;
- typed `Resources`;
- named API registry;
- service registry and startup validation;
- startup graph/readiness events;
- scheduler/job integration;
- plugin manager integration.

`Module<E>` is an in-process composition contract. It is not a backend boundary.

## 3. Plugin system

`newengine-plugin-api` provides stable ABI types:

- `PluginRootV1`;
- `PluginModule`, `PluginModuleV2`, `PluginModuleV3`;
- `HostApiV1`;
- `ServiceV1`;
- `PluginDescriptor` and `CapabilityDesc`;
- descriptor-level service/capability metadata.

`newengine-plugin-host` performs descriptor-first discovery. It probes DLL ABI metadata and uses declared services/capabilities for selection. It does not infer runtime provider identity from the filename.

## 4. Render subsystem

Render backend is service-owned:

```text
render.api service
render.backend capability
RenderServiceClient
ServiceBackedRenderApi
RenderApiRef
RenderFrameEnvelope
```

Runtime render controller is orchestration only. Native GPU ownership belongs to the renderer provider plugin.

GameReady draw/light/material features are explicit profile feature packs, not default engine-runtime backend code.

## 5. Physics subsystem

Physics is now replaceable through `physics.api`.

Core pieces:

```text
newengine-physics-api
  PHYSICS_SERVICE_ID = physics.api
  PHYSICS_BACKEND_CAPABILITY_ID = physics.backend
  PhysicsFrameInput / PhysicsFrameOutput
  body snapshots, commands, queries, collider packets
  backend info/capabilities/problem details

newengine-core::physics
  PhysicsApi trait
  PhysicsApiRef
  PHYSICS_API_ID

newengine-runtime-host::physics_runtime
  PhysicsServiceClient
  ServiceBackedPhysicsApi
  PhysicsProviderResolver
  PhysicsBackendRuntimeModule

newengine-engine-runtime
  PhysicsSyncModule
  ECS extraction/apply logic only
```

Physics provider plugins implement `physics.api`. The engine does not care whether the backend is native, deterministic, null, or future third-party technology.

## 6. Physics frame data

Frame input contains backend-neutral DTOs:

```text
PhysicsFrameInput
  frame_index
  fixed_tick
  dt
  gravity
  contact_skin
  bodies[]
  colliders[]
  commands[]
  queries[]
```

Frame output contains:

```text
PhysicsFrameOutput
  pose_updates[]
  velocity_updates[]
  events[]
  query_hits[]
  report
```

Static terrain/world geometry is represented by formal collider packets:

```text
HeightfieldColliderDto
MeshColliderDto
```

## 7. Asset system

AssetManager provides VFS and import/runtime asset services:

- filesystem layers;
- `.nepak` package layers;
- optional remote/cache layers;
- importer worker pipeline;
- texture dictionary lookup;
- raw/text/blob/texture service methods.

## 8. Proprietary codecs

### `.neytd`

Runtime texture dictionary. Materials and UI reference entries by:

```text
textures/fps/world_surfaces.neytd@terrain_forest_floor
textures/fps/world_surfaces.neytd@hash:<u64>
```

The runtime receives selected texture packets from AssetManager; it does not decode PNG/JPG/etc at runtime.

### `.nepak`

Deterministic asset archive for VFS layers. It contains payload blobs, a binary index, and a root `index.json` virtual-path manifest.

## 9. Tools

- `tools/netexturetool` — authoring/build tool for `.neytd` texture dictionaries.
- `tools/NePak` — authoring/build/inspect/extract/verify tool for `.nepak` packages.
