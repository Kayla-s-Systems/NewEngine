# 08 — Source map

## Core

```text
NewEngine/neocore2/crates/newengine-core
  engine lifecycle, modules, Resources, named APIs, startup contracts

NewEngine/neocore2/crates/newengine-plugin-api
  stable plugin ABI and descriptors

NewEngine/neocore2/crates/newengine-plugin-host
  descriptor-first discovery, loading, capability selection, plugin snapshots
```

Key files:

```text
newengine-core/src/engine/core.rs
newengine-core/src/module/resources.rs
newengine-core/src/startup/
newengine-plugin-api/src/root.rs
newengine-plugin-api/src/module.rs
newengine-plugin-api/src/service.rs
newengine-plugin-api/src/capability.rs
newengine-plugin-host/src/manager/discovery/
```

## Render

```text
NewEngine/neocore2/crates/newengine-render-api
NewEngine/neocore2/crates/newengine-runtime-host/src/render_runtime
NewEngine/neocore2/crates/newengine-core/src/render
NewEngine/neocore2/crates/newengine-engine-runtime/src/render_controller
Plugins/VulkanRenderer
Plugins/NullRenderer
```

## Physics

```text
NewEngine/neocore2/crates/newengine-physics-api
  stable physics.api protocol and DTOs

NewEngine/neocore2/crates/newengine-core/src/physics
  PhysicsApi trait, PhysicsApiRef

NewEngine/neocore2/crates/newengine-runtime-host/src/physics_runtime
  PhysicsServiceClient, ServiceBackedPhysicsApi, ProviderResolver, RuntimeModule

NewEngine/neocore2/crates/newengine-engine-runtime/src/gameplay/physics.rs
  PhysicsSyncModule, ECS packet extraction and output apply

Plugins/PhysicsDeterministic
Plugins/NullPhysics
Plugins/<native physics provider workspaces>
  concrete provider implementations behind physics.api
```

No provider implementation crate belongs to engine-runtime.

## Asset codecs

```text
NewEngine/neocore2/crates/newengine-texture-container
  .neytd runtime texture dictionary codec

Plugins/AssetManager/newengine-AssetManager/src/source.rs
  .nepak VFS source reader

Plugins/AssetManager/newengine-AssetManager/src/texture_dictionary
  .neytd selection through AssetManager

tools/netexturetool
  .neytd authoring/build UI

tools/NePak
  .nepak authoring/build/inspect/extract/verify UI
```

## ECS gateway

```text
NewEngine/neocore2/crates/newengine-ecs-api
  engine.ecs constants, startup requirement and DTOs

NewEngine/neocore2/crates/newengine-ecs
  concrete in-process World; internal implementation detail behind engine.ecs at service boundaries

NewEngine/neocore2/crates/newengine-runtime-host/src/ecs_runtime
  EcsServiceClient for summary/snapshot/command calls through engine.ecs

NewEngine/neocore2/crates/newengine-game-runtime/src/lib.rs
  engine-owned engine.ecs ServiceV1 route over SceneBridge world
```

## Entity gateway

```text
NewEngine/neocore2/crates/newengine-entity-api
  engine.entity constants, startup requirement and opaque handle/lifecycle DTOs

NewEngine/neocore2/crates/newengine-entity
  concrete EntityId key type; internal implementation detail behind engine.entity at service boundaries

NewEngine/neocore2/crates/newengine-runtime-host/src/entity_runtime
  EntityServiceClient for list/exists/spawn/despawn calls through engine.entity

NewEngine/neocore2/crates/newengine-game-runtime/src/lib.rs
  engine-owned engine.entity ServiceV1 route over SceneBridge world
```

```text
newengine-platform-api      engine.platform / platform.backend / native window snapshot DTOs
```
