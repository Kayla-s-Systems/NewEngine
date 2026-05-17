# Plugin-owned content architecture

NewEngine keeps executable provider discovery and declarative content catalogs separate.

## Runtime plugin identity

Runtime plugin identity comes from ABI metadata only:

```text
DLL -> PluginSignatureV1 / PluginDescriptor -> services + capabilities
```

No runtime provider should be identified by filename prefix, filename pattern, or deployment manifest entry.

## Content catalog

Declarative content is a separate optional catalog. It is not a plugin discovery manifest and must not be used to decide which DLL provides a service.

Current content catalog path:

```text
plugin-content.manifest.json
```

The catalog may publish scene/map/prefab/material/generator payloads owned by plugins or feature packs. Runtime/editor layers adapt those JSON payloads into domain-specific ECS data.

## Thin-engine boundary

- `newengine-plugin-host` exposes a typed content catalog loader.
- `newengine-plugin-api` defines generic capability metadata.
- Domain runtimes adapt content payloads to ECS.
- Backend providers remain selected by service/capability descriptors, not by content manifests.

## Physics note

Physics is not content. Physics backend implementation is a service provider behind `physics.api`. Engine/runtime code talks to `PhysicsApiRef` and DTO packets only.
