# Plugin Override Priority

## Purpose

NewEngine treats built-in systems as baseline provider candidates, not as privileged runtime destiny.

```text
consumer -> engine.camera
             ↓
          ActiveGatewayRegistry
             ↓
          selected provider service
```

A plugin or mod that implements the same gateway contract can replace an engine-owned provider through the same route registry path.

## Core invariant

```text
The engine owns the gateway.
The provider owns the implementation.
The profile owns override policy.
The plugin owns capability declaration.
The registry owns deterministic selection.
```

Consumers continue to call stable ids such as `engine.camera`, `engine.render`, `engine.physics`, `engine.assets`, `engine.ecs` and `engine.entity`. They do not call provider ids directly.

## Trusted origin tiers

Provider origin is host-assigned. It must never be trusted from plugin descriptor JSON.

```text
DevOverride       +50_000
UserMod           +40_000
GamePlugin        +30_000
FirstPartyPlugin  +20_000
EngineOwned       +10_000
NullProvider          0
```

The registry computes:

```text
active_score = origin_bias(origin) + backend_priority
```

`backend_priority` remains useful, but only inside the origin tier. A user mod with priority `0` beats an engine-owned route with priority `5000` because the mod origin tier is higher.

## Override modes

```text
Open
ProfileControlled
Locked
```

Current default enforcement:

- `Locked` gateways reject plugin-owned routes.
- `Open` gateways accept valid providers.
- `ProfileControlled` gateways are selectable by default until profile policy narrows them further.

Locked trust-root gateway ids:

```text
engine.plugin_host
engine.abi
engine.gateway_registry
engine.security
engine.scheduler.core
engine.capability_validator
```

Profile-controlled gateway ids:

```text
engine.render
engine.physics
engine.assets
engine.scene
engine.save
engine.network
```

Everything else is treated as open by default.

## Provider metadata

Plugins still declare ordinary backend capability metadata:

```json
{
  "service_kind": "camera",
  "engine_gateway": "engine.camera",
  "contract": "kayla.cinematic_camera.api",
  "backend_priority": 100
}
```

The descriptor declares capability. The loader/profile assigns origin. The registry combines both.

## Diagnostics

Gateway route snapshots now expose:

```text
gateway_id
service_kind
provider_service_id
provider_owner_id
backend_capability_id
origin
override_mode
backend_priority
active_score
active/shadowed state
```

The plugin table prints both `prio` and `score`, so an overridden built-in provider is visible instead of silently losing.

## Acceptance tests

The implementation adds registry-level tests for:

```text
plugin_origin_tier_overrides_engine_owned_even_with_lower_backend_priority
engine_owned_is_used_when_no_plugin_provider_exists
higher_priority_wins_inside_same_origin_tier
locked_gateway_rejects_plugin_route
tie_breakers_are_deterministic
```
