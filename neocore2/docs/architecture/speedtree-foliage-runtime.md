# SpeedTree foliage runtime

NorthStar integrates SpeedTree as a foliage capability over the existing asset, model,
scene, material and render gateways. It does not add a second engine runtime and it
does not vendor the SpeedTree SDK.

## Ownership and flow

1. A project profile names an opaque SpeedTree `.srt` or Modeler `.spm` source in `FoliageSettings`.
2. `engine.assets` owns source bytes, import scheduling, cache keys and invalidation.
3. An optional `AssetImporterV1` provider with capability
   `assets.models.foliage.srt_importer` or `assets.models.foliage.spm_importer` compiles the licensed source into engine-owned
   `.nefoliage`, `.ydd`, `.nemat` and `.ytd` runtime assets.
4. `engine.assets.models` validates the foliage manifest and exposes only model-domain
   DTOs. Materials are registry-backed `MaterialId` handles.
5. Scene/world owns placement and ECS mutation.
6. Render extraction reads foliage components and emits deterministic instance commands.
   It never mutates ECS state or starts worker threads.
7. A renderer may advertise `render.foliage.gpu_culling` plus indirect draw support.
   Otherwise the deterministic CPU extraction path is mandatory.

## Authored profile

The existing YMAP profile accepts foliage policy as attributes or child values:

```xml
<foliage
  enabled="true"
  canonical_path="Source/foliage/speedtree/oak/Oak_Hero_Forest.spm"
  prefab="oak_compiled"
  seed="6075990651965790246"
  density="0.72"
  max_count="4096"
  min_scale="0.85"
  max_scale="1.35"
  material_variant="summer"
  wind_enabled="true"
  wind_strength="0.42"
  wind_gust_frequency="0.17"
  wind_direction_x="0.78"
  wind_direction_y="0.0"
  wind_direction_z="0.62"
  lod0_distance="28"
  lod1_distance="64"
  lod2_distance="120"
  impostor_distance="120"
  lod_crossfade_width="6"
  cull_distance="300"
  shadow_cull_distance="180"
  prefer_gpu_culling="true"
/>
```

The declared prefab remains the compiled `.ydd@entry` fallback. Import failure never
silently disables existing foliage: GameReady logs the missing provider and continues
with the compiled YDD path and CPU distance culling.

## Importer provider contract

The engine intentionally does not decode proprietary `.srt` or `.spm` bytes. A licensed/provider-side importer
must:

- register `assets.models.foliage.srt_importer` for `.srt` and/or `assets.models.foliage.spm_importer` for `.spm`;
- accept the matching `northstar.importer.speedtree_srt.v1` or `northstar.importer.speedtree_spm.v1` descriptor;
- include source hash, importer version, settings hash and target platform in its cache key;
- emit contiguous zero-based LOD entries with finite distance ranges;
- emit registry-backed material handles and optional billboard atlas metadata;
- publish dependencies so source changes invalidate every generated runtime asset;
- preserve deterministic output for the same source, settings and platform.

## Current hardware boundary

The current instanced renderer consumes shared YDD geometry and compact transform/material
instance buffers. Wind is preserved in foliage extraction DTOs, but the existing instance
vertex ABI has not been widened for SpeedTree wind attributes. The licensed importer
provider, provider-produced runtime manifest consumption and a hardware smoke test remain
required before claiming full SpeedTree Runtime SDK parity.
