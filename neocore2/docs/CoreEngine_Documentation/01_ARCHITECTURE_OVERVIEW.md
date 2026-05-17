# CoreEngine / NewEngine Architecture

## Philosophy

```text
Engine as Host. Gateway as Facade. Service as Plugin. Feature as Provider.
```

Core/runtime must not become a wrapper around a single renderer, physics SDK, AssetManager, input plugin, or UI backend.
The engine owns lifecycle, scheduling, resources, startup graph, service registration, gateway routing, capability validation, and typed adapter binding.

Backend implementation lives in provider plugins. Runtime systems call engine-facing gateway ids and typed API refs, not concrete provider service ids.

## Main layers

```text
newengine-service-api      service vocabulary, requirement specs, engine gateway namespace
newengine-plugin-api       stable plugin ABI and descriptors
newengine-plugin-host      descriptor loading, gateway registry, provider selection
newengine-core             lifecycle, startup graph, Resources, declarative validation
newengine-runtime-host     platform shell and typed domain adapters
newengine-engine-runtime   gameplay/runtime orchestration without backend ownership
Plugins/*                  renderer, physics, assets, input, platform, logging providers
```

## Descriptor-first and gateway-first discovery

Runtime discovery does not treat file names, folder names, or old service ids as provider identity.

```text
library path -> ABI probe -> PluginDescriptor -> services/capabilities -> gateway route
```

Provider metadata example:

```json
{
  "service_kind": "render",
  "engine_gateway": "engine.render",
  "contract": "render.api",
  "backend_priority": 100
}
```

Unknown `service_kind` values are warned and ignored by default.

## Gateway Service Layer

Consumers call stable engine-owned facade ids:

```text
engine.assets
engine.render
engine.physics
engine.input
```

The host resolves each gateway to the active provider service.

## Degradation policy

Missing providers are not fatal by default. The engine logs the degraded service and continues when possible. Strict startup is an explicit policy controlled by runtime/profile flags.
