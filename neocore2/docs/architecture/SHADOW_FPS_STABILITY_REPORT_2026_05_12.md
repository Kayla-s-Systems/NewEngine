# Shadow/FPS stability report — 2026-05-12

## Problem observed

Runtime logs reported an active directional shadow plan and a valid shadow render target, but the image was visually unshadowed. FPS also degraded over time while the non-instanced V3 path replayed hundreds of primitive draw calls per frame.

## Root cause

The shader compiler legacy embedded table aliased the current shadowed runtime shaders to the old compact textured fallback:

- `editor_lit_shadowed_v3.vert/frag` -> `REMOVED_EMBEDDED_EDITOR_LIT_TEXTURED_*`
- `editor_shadow_depth_v1.vert/frag` -> `REMOVED_EMBEDDED_EDITOR_LIT_TEXTURED_*`

That fallback is safe for legacy textured rendering, but it ignores runtime shadow sampling and is depth-incorrect for the shadow pass. The engine therefore showed `shadow_map` in diagnostics while the actual SPIR-V path was effectively unshadowed.

## Fix

The current shadowed runtime shaders are no longer allowed to silently use the legacy legacy embedded fallback. They now compile from GLSL/runtime cache. If `glslc`/Vulkan SDK is unavailable, the renderer must fail loudly instead of pretending that shadows are active.

The GLSL shadow sampling path was also hardened:

- clamped shadow taps;
- white-cleared shadow map samples are treated as lit;
- shadow strength is raised for visual validation;
- depth value handling is kept explicit in the shadow depth pass.

## FPS stabilization

The runtime now has a deterministic primitive draw budget for the current non-instanced V3 path:

- nearest runtime opaque primitives: 128;
- nearest runtime shadow primitives: 96;
- editor budgets are higher for inspection.

This is not the final AAA solution. It is a stability guard until the renderer has GPU instancing, meshlet/cluster culling, HZB/occlusion, and streaming LODs.

## Shutdown stability

Vulkan explicit teardown is now guarded by `NEWENGINE_VULKAN_STRICT_DROP=1`. By default, process-exit safe drop avoids explicit driver teardown paths that were producing `STATUS_ACCESS_VIOLATION` during close. Strict explicit destruction remains available for backend debugging.

## Next foundation step

The correct next rendering foundation is:

```text
RenderVisibilitySystem
  camera frustum
  object bounds
  distance bands
  LOD policy
  draw budget
  shadow caster budget

GpuScene
  instance buffers
  material table
  per-frame visible instance lists
  indirect draw batches
```

The current pass makes the runtime stable and traceable while preserving a clear path toward that architecture.
