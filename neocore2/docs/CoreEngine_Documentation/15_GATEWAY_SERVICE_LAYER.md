# Gateway Service Layer

## Purpose

The Gateway Service Layer is the host-owned facade over plugin services.

Old model:

```text
consumer -> render.api
consumer -> physics.api
consumer -> asset.manager / asset_manager.api
```

Target model:

```text
consumer -> engine.render
consumer -> engine.physics
consumer -> engine.assets
consumer -> engine.input
```

The engine resolves each gateway to the active provider service from plugin descriptors.

## Provider metadata contract

A provider declares a concrete service and a backend capability whose JSON describes the route:

```json
{
  "service_kind": "assets",
  "engine_gateway": "engine.assets",
  "contract": "asset_manager.api",
  "backend_priority": 100
}
```

Fields:

```text
service_kind      engine vocabulary string, e.g. assets/render/physics/input
engine_gateway    stable facade id consumers call
contract          provider service id registered by the plugin
backend_priority  provider selection priority; higher wins
```

Unknown `service_kind` values are not fatal. The host logs a warning and ignores the route.

## Runtime routing

```text
HostApiV1.call_service_v1(engine.render, method, payload)
  -> ActiveGatewayRegistry snapshot
  -> route selected by gateway + provider service + owner plugin
  -> provider ServiceV1.call(method, payload)
```

Routing is data-driven and must not contain per-domain branches.

## Selection rule

When multiple providers expose the same gateway:

1. Higher `backend_priority` wins.
2. Provider service id is used as a deterministic tie-breaker.
3. Plugin id is used as the final deterministic tie-breaker.

## Degradation policy

```text
no active route       -> service unavailable / feature degraded
bad metadata          -> warning + ignored route
unknown service_kind  -> warning + ignored route
strict env enabled    -> fatal startup contract error
```

## Relationship to typed adapters

Gateway routing does not replace typed APIs. It only finds the provider service.

```text
engine.render  -> ServiceBackedRenderApi  -> RenderApiRef
engine.physics -> ServiceBackedPhysicsApi -> PhysicsApiRef
engine.assets  -> AssetServiceClient      -> runtime asset packets
```
