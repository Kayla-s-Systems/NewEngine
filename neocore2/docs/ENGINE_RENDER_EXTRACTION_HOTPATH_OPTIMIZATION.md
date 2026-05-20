# Engine render extraction hotpath optimization

This patch targets the second-stage bottleneck after scene residency/culling.
Terrain extraction is no longer the constant offender in the current trace: a stable frame can reach roughly 13 ms total, but motion/streaming still creates spikes.

## Observed problem

The current profile shows two remaining hot paths:

1. Primitive/foliage extraction spikes. Runtime primitive draw extraction repeated material resolution, texture residency polling, bind-group lookup and UBO writes per entity. Foliage is naturally many entities sharing the same mesh/material buckets, so this should be bucket-level work.
2. Terrain GPU residency spikes. A streamed terrain chunk upload costs around 10 ms on the current service-backed Vulkan path. Uploading one new chunk on several consecutive frames produces visible FPS drops.

## Runtime rule

```text
scene traversal
  -> cull to active render set
  -> group by mesh/material/render-state bucket
  -> resolve material + textures once per bucket
  -> write shared UBO once per bucket
  -> upload one instance buffer per bucket
  -> draw instanced
```

Do not resolve material/texture/UBO state once per placed tree part when all instances share the same bucket.

## Changes

- Primitive forward pass now caches a `PrimitiveGpuPlan` per frame by primitive id, material id, color, sky-follow mode and pass type.
- Primitive shadow pass uses the same bucket plan cache.
- Shared lit/shadow UBOs are written once per unique bucket instead of once per entity.
- Duplicate `set_pipeline` call in primitive instanced replay was removed.
- Terrain GPU upload pump now supports `NEWENGINE_TERRAIN_GPU_UPLOAD_INTERVAL_FRAMES` to avoid consecutive 10 ms upload spikes when crossing a chunk boundary.

## Tuning

```bat
set NEWENGINE_TERRAIN_GPU_UPLOADS_PER_FRAME=1
set NEWENGINE_TERRAIN_GPU_UPLOAD_INTERVAL_FRAMES=6
set NEWENGINE_RUNTIME_OPAQUE_PRIMITIVE_BUDGET=64
set NEWENGINE_RUNTIME_SHADOW_PRIMITIVE_BUDGET=32
```

The long-term fix is backend-side async/staging upload and a renderer-owned transient upload queue with fences. This patch makes the current service-backed path survivable while that backend work is built.
