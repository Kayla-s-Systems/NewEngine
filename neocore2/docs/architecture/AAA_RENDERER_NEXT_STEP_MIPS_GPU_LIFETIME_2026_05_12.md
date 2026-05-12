# AAA renderer next step: texture stability + Vulkan lifetime

## Problem observed

Runtime captures still showed heavy grain after shadows became visible. Frame stats were already bounded to ~129 opaque indexed draws, so the remaining dominant artifact was not draw-count pressure. It was high-frequency material sampling on distant repeated terrain textures.

## Reference renderer alignment

The attached renderer reference is built around explicit renderer subsystems: render phases, draw lists, entity handlers, shadow systems, lighting systems, post FX and resource managers. The equivalent direction in NewEngine is:

1. stable resource residency before presentation;
2. mipmapped material residency;
3. explicit phase graph;
4. batched/instanced scene submission;
5. CSM/depth-comparison shadows;
6. post-processing and temporal AA.

This pass implements item 2 and hardens item 1.

## Changes in this pass

### Material mip chains

Material textures now request a full, capped mip chain through `TextureDesc::with_mips(...)`.

The Vulkan upload path now:

1. uploads mip 0;
2. generates lower mip levels through `vkCmdBlitImage`;
3. transitions every generated mip to `SHADER_READ_ONLY_OPTIMAL`.

This directly targets terrain shimmer/grain from minified repeated textures.

### Derivative-stable material sampling

The lit shader now samples albedo, normal and roughness through `textureGrad(...)` using explicit screen-space UV derivatives. This gives the hardware mip selector stable gradients and avoids accidental unstable implicit derivative behavior in large terrain triangles.

### Stable Vulkan process exit

The Vulkan plugin now defaults to stable process-exit mode:

```bat
NEWENGINE_VULKAN_STRICT_DROP=0
```

In this mode explicit backend teardown is skipped and OS process cleanup owns the final GPU resource reclamation. This is deliberate until the renderer has a fully deterministic destruction graph.

For backend teardown debugging:

```bat
set NEWENGINE_VULKAN_STRICT_DROP=1
```

## Expected result

After material textures become resident, terrain should be visibly less speckled at distance. The first run after this patch may spend slightly more upload time during the gated loading phase because mip chains are generated. That is acceptable: loading remains gated and the player should not see progressive texture residency.

## Next renderer-grade step

The next real AAA renderer step is not another per-object forward optimization. It is `GpuScene` + instance batches:

```text
Primitive/Material extraction
  -> MeshBatchKey { mesh, material, pipeline, shadow_mode }
  -> InstanceBuffer
  -> DrawIndexedInstanced
```

Target draw-list stats:

```text
opaque_forward: indexed 129 -> 8..24
shadow_casters: indexed 97 -> 8..24
```
