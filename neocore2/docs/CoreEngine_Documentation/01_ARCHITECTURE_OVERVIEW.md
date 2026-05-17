# 01 — Архитектура CoreEngine / NewEngine

## 1. Философия

Целевая формула:

```text
Engine as Host. Service as Plugin. Feature as Provider.
```

Core/runtime не должен быть оболочкой над конкретным renderer/physics/input/UI backend. Core/runtime отвечает за lifecycle, scheduler, Resources, startup graph, service registry, plugin ABI, capability validation и adapter binding.

Backend implementation живёт в plugin provider. Runtime systems работают только через stable API crates и typed adapter refs.

## 2. Главные слои

```text
NewEngine/neocore2/crates/newengine-core
  Engine lifecycle, Module<E>, Resources, named APIs, services, startup validation

NewEngine/neocore2/crates/newengine-plugin-api
  stable ABI: PluginRootV1, PluginModuleV3, HostApiV1, ServiceV1, PluginDescriptor, CapabilityDesc

NewEngine/neocore2/crates/newengine-plugin-host
  DLL scan/load, ABI descriptor probe, capability/service selection, plugin snapshots

NewEngine/neocore2/crates/newengine-runtime-host
  platform shell, render_runtime adapter, physics_runtime adapter, input/platform bridges

NewEngine/neocore2/crates/newengine-engine-runtime
  reusable gameplay/render orchestration and ECS sync; no concrete backend implementation

Plugins/*
  runtime provider workspaces: renderer, physics, assets, input, logging, platform, UI
```

## 3. Descriptor-first plugin discovery

Runtime discovery no longer treats the filename as plugin identity.

```text
.dll/.so/.dylib path
  -> load/probe exported ABI symbols
  -> PluginSignatureV1 / PluginDescriptor / PluginInfo
  -> declared services and capabilities
  -> provider selection
```

Filename is allowed only as a filesystem path and deterministic tie-breaker after two descriptors are semantically identical. It must never be used to decide that a DLL is a renderer, physics backend, UI provider, or asset provider.

Backend selection uses:

```text
render:  provides service render.api  + capability render.backend
physics: provides service physics.api + capability physics.backend
priority: backend_priority in capability describe_json
```

## 4. Render boundary

```text
Renderer provider plugin
  provides service render.api
  provides capability render.backend
      ↓
RenderBackendRuntimeModule
  RenderServiceClient
  ServiceBackedRenderApi : dyn RenderApi
  Resources.register_api(RENDER_API_ID, RenderApiRef)
      ↓
RuntimeRenderController
  builds RenderFrameEnvelope
  submit_frame(...)
```

No in-process `NullRenderApi`, no `NEWENGINE_RENDER_BACKEND`, no `auto`, no backend aliases.

## 5. Physics boundary

Physics now follows the same service adapter model.

```text
Physics provider plugin
  provides service physics.api
  provides capability physics.backend
      ↓
PhysicsBackendRuntimeModule
  PhysicsServiceClient
  ServiceBackedPhysicsApi : dyn PhysicsApi
  Resources.register_api(PHYSICS_API_ID, PhysicsApiRef)
      ↓
PhysicsSyncModule
  ECS snapshots/commands/colliders -> PhysicsFrameInput
  physics.api StepFrame
  PhysicsFrameOutput -> ECS apply
```

The engine owns the physics protocol, not the backend. A native physics provider, deterministic provider, null provider, or future provider all implement the same `physics.api` contract.

Hard invariant:

```text
newengine-engine-runtime must not import provider implementation crates.
No backend receives &mut World.
No native physics handles cross the ABI boundary.
No engine code contains provider-specific branch logic.
```

## 6. Asset/data boundary

AssetManager remains a runtime provider behind `asset.manager`. Runtime texture references use `.neytd@entry` selectors. Packaged asset sources are mounted through `.nepak` VFS layers.

```text
logical path
  -> VFS layer resolution
  -> filesystem / nepak / optional remote source
  -> AssetManager service methods
  -> typed runtime packets, e.g. NTRT texture payload
```

## 7. Release-oriented direction

Remaining work is no longer “make physics replaceable”; it is hardening:

- provider conformance/replay tests;
- zero-warning CI;
- richer physics events/contacts;
- binary hot path packets where JSON is too expensive;
- input/UI API formalization;
- shutdown/reload tests.
