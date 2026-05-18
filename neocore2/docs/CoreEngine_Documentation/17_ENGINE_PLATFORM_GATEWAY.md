# 17 — Engine Platform Gateway

## Summary

`engine.platform` is now the engine-facing platform gateway.

```text
engine.platform
  -> host-owned platform gateway route
  -> native window handles
  -> surface metrics
  -> renderer/UI/runtime consumers
```

The old direct platform-window service shape is removed from the consumer contract. Runtime plugins and adapters should not call a platform-window-specific service id. They call `engine.platform` and use platform API methods.

## Service identity

```text
consumer gateway: engine.platform
provider service: engine.platform        # current host-owned baseline
future provider service: platform.api or vendor platform service
backend capability: platform.backend
origin: EngineOwned
priority: 0
owner: newengine-runtime-host.platform-gateway
```

The current implementation is host-owned because the native platform runtime owns the window callback lifecycle. It is still registered as a normal engine-owned route candidate through `register_engine_owned_gateway(...)`, not as a hidden direct service.

## Methods

```text
info_json
invoke_json
shutdown_v1
window_snapshot_json_v1
```

`window_snapshot_json_v1` returns the existing `PlatformWindowReadyV1` DTO:

```text
NativeWindowHandlesV1
PlatformSurfaceMetricsV1
```

This keeps render/backend initialization stable without exposing a direct `platform.window.*` service identity.

## Runtime path

```text
Vulkan/render provider
  -> HostApiV1.call_service_v1(engine.platform, window_snapshot_json_v1, {})
  -> ActiveGatewayRegistry
  -> engine-owned platform provider service
  -> PlatformWindowReadyV1
  -> native raw-window-handle conversion
```

## Invariants

- Consumers must call `engine.platform`.
- Window data is a method/DTO inside the platform gateway, not the service id.
- Platform backend capability is `platform.backend`.
- Current baseline is engine-owned and can later be replaced by a plugin/provider route.
- No compatibility alias is kept for the removed direct service id.
