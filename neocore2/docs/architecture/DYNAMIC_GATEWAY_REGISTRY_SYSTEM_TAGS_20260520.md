# DynamicGatewayRegistry + SystemTags pass — 2026-05-20

## Purpose

The gateway registry is intentionally empty at startup. It is populated from facts:

```text
registered ServiceV1 entries
plugin descriptor backend-route metadata
engine-owned gateway facts
optional host/profile policy facts
```

The registry must not contain a hard-coded gateway table such as render/physics/assets/input. A new gateway becomes visible when a provider declares:

```json
{
  "service_kind": "weather",
  "engine_gateway": "engine.weather",
  "contract": "weather.api",
  "backend_priority": 100,
  "system_tags": ["runtime", "provider.backend", "override.open"]
}
```

## SystemTags

`SystemTags` classify routes without introducing enum sprawl:

```text
engine.domain
provider.backend
runtime
override.open
override.profile_controlled
override.locked
trust.root
```

Provider tags describe the capability. Host/profile policy facts own trust decisions. A plugin may describe itself, but it does not get to assign a trusted origin tier.

## Registry invariant

```text
DynamicGatewayRegistry = facts in, deterministic route selection out
```

The registry applies one generic pipeline:

1. normalize `engine_gateway`;
2. normalize `service_kind`;
3. require gateway domain and service kind to describe the same domain depth;
4. verify the provider service was actually registered by the declaring owner;
5. merge route tags with policy facts;
6. resolve override mode from policy/tags;
7. select by trusted origin tier, backend priority and deterministic tie-breakers.

No route is selected by filename, aliases, hand-written provider ids or per-domain `if` branches.

## EngineServiceKind after this pass

`EngineServiceKind` remains a convenience helper for first-party engine-owned declarations, but it is no longer the registry's extension boundary. Plugin routes are stored as normalized strings, so third-party domains can appear without editing the engine enum.

## Logging gateway cleanup

The logging gateway is now `engine.logging`, matching `service_kind=logging`. The old mixed spelling `engine.log` was removed from code paths touched by this pass.
