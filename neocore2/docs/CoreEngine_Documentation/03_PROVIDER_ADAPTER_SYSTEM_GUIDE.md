# Provider / Adapter System Guide

## Terms

**Gateway**: stable engine-facing facade id, such as `engine.render`.

**Provider service**: concrete plugin-owned service id, such as `render.api`, `asset_manager.api`, or a third-party equivalent.

**Backend provider**: plugin that declares a backend capability and maps itself to an engine gateway through metadata.

**Adapter**: host-side typed wrapper over a service protocol, registered in `Resources` as a typed API ref.

## Strict rules

- No backend identity by filename.
- No backend nickname aliases in engine selection logic.
- No hidden in-process fallback backend.
- Null backends must be real service provider plugins.
- Runtime systems import API crates/adapters, not provider implementation crates.
- Backend priority is declared metadata, not hard-coded engine knowledge.
- Consumers call `engine.*` gateways, not provider service ids.
- Unknown provider kinds warn and are ignored by default.
- Provider origin/trust tier is host-assigned and must never be trusted from plugin JSON.

## Provider self-description

```json
{
  "service_kind": "render",
  "engine_gateway": "engine.render",
  "contract": "vendor.render.api",
  "backend_priority": 100
}
```

The plugin does not import an engine enum. It writes metadata. The engine validates known strings and ignores unknown kinds.

## Gateway routing

```text
consumer call: engine.render
        -> ActiveGatewayRegistry
        -> selected provider route
        -> provider service call
```

No domain-specific `if render else physics else assets` routing tree is allowed.
