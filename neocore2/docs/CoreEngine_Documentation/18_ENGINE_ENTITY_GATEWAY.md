# 18 — Entity gateway: `engine.entity`

## Purpose

`engine.entity` is the stable service facade for entity identity and coarse lifecycle operations.
It prevents service/runtime consumers from coupling to `newengine_entity::EntityId` or concrete ECS internals.

```text
consumer / tool / service
  -> engine.entity
  -> ActiveGatewayRegistry
  -> selected entity.api provider or engine-owned scene entity bridge
  -> opaque handle / lifecycle DTOs
```

## Current implementation

Current route is engine-owned:

```text
owner: newengine-game-runtime.entity-gateway
service: engine.entity
capability: entity.backend
origin: EngineOwned
priority: 0
```

It wraps the active `SceneBridge` world and exposes entity operations through the same host service registry path as `engine.scene`, `engine.ecs` and `engine.camera`.

## Service contract

API crate:

```text
newengine-entity-api
  ENGINE_ENTITY_SERVICE_ID = engine.entity
  ENTITY_SERVICE_ID = entity.api
  ENTITY_BACKEND_CAPABILITY_ID = entity.backend
```

Methods:

```text
info_json
invoke_json
list_json_v1
exists_json_v1
spawn_json_v1
despawn_json_v1
shutdown_v1
```

DTO identity is `EntityHandle { stable_id: u64 }`. It is deliberately opaque: consumers can pass it back to the gateway, but they do not receive or construct native `EntityId` values.

## Boundary rule

Bad service/runtime boundary:

```text
consumer -> EntityId
consumer -> World::exists(EntityId)
consumer -> World::spawn() / World::despawn(EntityId)
```

Good service/runtime boundary:

```text
consumer -> engine.entity list_json_v1
consumer -> engine.entity exists_json_v1
consumer -> engine.entity spawn_json_v1
consumer -> engine.entity despawn_json_v1
```

`engine.ecs` remains the broader world summary/snapshot/command facade. `engine.entity` is intentionally narrower and should be preferred when the consumer only needs entity identity or lifecycle.

## Provider override target

Future provider shape:

```text
Entity provider plugin
  provides service entity.api or vendor entity service
  provides capability entity.backend
  metadata:
    service_kind=entity
    engine_gateway=engine.entity
    contract=<provider service id>
    backend_priority=<priority>
```

This lets a game/profile replace the built-in entity service without changing consumers.
