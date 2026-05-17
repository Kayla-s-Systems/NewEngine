# Provider / Adapter System: Current State

The provider system is gateway-first.

```text
consumer -> engine gateway -> ActiveGatewayRegistry -> provider service -> typed adapter
```

Providers describe themselves with `service_kind`, `engine_gateway`, `contract`, and `backend_priority` metadata. The host validates the metadata and builds active routes from registered services and plugin descriptors.

No routing code should branch manually on concrete domains or provider ids.
