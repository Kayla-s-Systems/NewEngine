# newengine-lighting

Scene-domain lighting component/resource DTOs shared by ECS, render extraction and profile feature packs.

This crate is **not** a lighting renderer, not a shadow backend, and not a GPU tiled/clustered implementation. It contains only authoring/runtime scene data such as ambient, directional, point and spot light parameters plus explicit shadow settings.

Native execution belongs to the selected `engine.render` provider:

```text
ECS light components/resources
  -> render feature extraction
  -> render.api frame/light packets
  -> renderer-owned LightBuffer / LightGrid / ClusterGrid
  -> native deferred/tiled/clustered lighting
```

There is intentionally no automatic shadow method selector in this crate. Provider capability negotiation and profile policy decide whether cascaded, point, spot or disabled shadows are used.
