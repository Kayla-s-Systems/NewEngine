# 16 — ECS gateway: `engine.ecs`

## Purpose

`engine.ecs` is the stable service facade for ECS-level world inspection and coarse structural commands.
It prevents service/runtime consumers from coupling to the concrete `newengine_ecs::World` type.

```text
consumer / tool / service
  -> engine.ecs
  -> ActiveGatewayRegistry
  -> selected ecs.api provider or engine-owned scene ECS bridge
  -> summary / snapshot / command DTOs
```

## Current implementation

Current route is engine-owned:

```text
owner: newengine-ecs-runtime.ecs-gateway
service: engine.ecs
capability: ecs.backend
origin: EngineOwned
priority: 0
```

It wraps the active `SceneBridge` world and exposes the gateway through the same host service registry path as `engine.scene` and `engine.camera`.

## Service contract

API crate:

```text
newengine-ecs-api
  ENGINE_ECS_SERVICE_ID = engine.ecs
  ECS_SERVICE_ID = ecs.api
  ECS_BACKEND_CAPABILITY_ID = ecs.backend
```

Methods:

```text
info_json
invoke_json
summary_json_v1
snapshot_json_v1
command_json_v1
shutdown_v1
```

`summary_json_v1` returns provider-neutral counters:

```text
tick
entity_count
storage_count
resource_count
entities_changed_tick
```

`snapshot_json_v1` returns summary plus opaque stable entity ids. It does not expose component storage layout.

`command_json_v1` currently supports coarse operations only:

```text
set_tick
advance_tick
spawn_empty
```

Typed component mutation remains an in-process ECS concern until a domain-safe component schema is introduced.

## Boundary rule

Bad service/runtime boundary:

```text
consumer -> &mut World
consumer -> world.query::<T>()
consumer -> world.insert(entity, component)
```

Good service/runtime boundary:

```text
consumer -> engine.ecs summary_json_v1
consumer -> engine.ecs snapshot_json_v1
consumer -> engine.ecs command_json_v1
```

Internal high-performance systems may still use typed ECS access inside the engine/runtime crate that owns the system, but cross-domain service consumers should not depend on `newengine_ecs::World`.

## Provider override target

Future provider shape:

```text
ECS provider plugin
  provides service ecs.api or vendor ecs service
  provides capability ecs.backend
  metadata:
    service_kind=ecs
    engine_gateway=engine.ecs
    contract=<provider service id>
    backend_priority=<priority>
```

This lets a game/profile replace the built-in ECS world service without changing consumers.
