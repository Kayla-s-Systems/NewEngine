# Plugin discovery descriptor-first pass — 2026-05-17

## Decision

Runtime plugin discovery no longer classifies plugins by DLL filename or by a generated runtime plugin manifest.

The runtime host now treats each dynamic library as self-describing:

```text
DLL
  -> ABI metadata probe
  -> PluginSignatureV1 / PluginDescriptor
  -> declared services + capabilities
  -> provider selection by capability metadata
```

## Removed from runtime discovery

```text
filename-pattern identity matching
runtime manifest as plugin identity source
filename-to-plugin-id inference
physics provider filename recognition
render provider filename recognition
provider-specific id priority rules
```

## Provider selection

Backend selection is based on declared capabilities and generic provider metadata:

```text
render backend candidate:
  provides service render.api
  provides capability render.backend

physics backend candidate:
  provides service physics.api
  provides capability physics.backend

priority:
  capability describe_json.backend_priority
```

The host does not need to know which concrete physics backend implementation is installed.

## Plugin responsibility

A backend plugin must declare the stable API and backend capability in its descriptor. Example shape:

```rust
PluginDescriptor::builder(id, name, version, PluginKind::Runtime)
    .provides_service("physics.api", 1, r#"{...}"#)
    .push(
        CapabilityDesc::new("physics.backend", Provides, Other, 1)
            .with_json(r#"{"backend_priority":200}"#),
    )
    .build()
```

## Invariant

Engine/runtime code owns the API boundary and provider resolver only. Backend implementation details and backend-specific naming stay inside provider plugins.
