# AAA Renderer Alignment Pass — 2026-05-12

## Input reference

`renderer.zip` is useful as an architectural reference because it separates renderer work into stable systems instead of one monolithic frame function:

- `RenderPhases/` — named phase objects with update budgets and explicit viewport/scanner ownership.
- `DrawLists/` — draw-list construction, draw-list manager, command/data separation and profile stats.
- `Deferred/` — GBuffer + deferred lighting split.
- `Shadows/` — dedicated shadow subsystem, cascade/paraboloid variants and debug paths.
- `Lights/` — light classification, culling, tiled lighting, LOD lights and async occlusion.
- `Entities/` — separate entity draw handlers, including instanced entity rendering.

NewEngine should not copy the source style or old platform assumptions. The direction is to absorb the architecture shape:

```text
FrameOrchestrator
  → RenderPhaseGraph
  → Visibility/DrawListExtraction
  → GpuScene/InstanceBatches
  → ShadowSystem/CSM
  → LightingSystem
  → PostFxChain
  → DebugTelemetry
```

## Current pass

This pass keeps the current Vulkan V3 forward path but moves it closer to the target:

1. **Stable shadow map format**
   - Shadow color payload is now `Rgba16Float`, not `Bgra8Unorm`.
   - Shadow depth encoding stays explicit until the backend grows sampleable depth textures.

2. **Stable shadow filtering**
   - Removed the temporary dual-origin `min(lit_a, lit_b)` fallback.
   - That fallback made shadows visible but introduced mirrored false-shadow samples and the black/noisy terrain pattern.
   - The shader now uses one explicit Vulkan RT origin convention and a small stable PCF kernel.

3. **Runtime quality profile seed**
   - Added `render_controller/render_quality.rs` as the first declarative profile surface for renderer constants.
   - This should evolve into a real `RenderSettings` resource loaded from config.

4. **Trace policy**
   - Added `module_impl/trace_policy.rs`.
   - Steady-state render diagnostics are now sparse so they do not distort frame timing.

5. **Material anti-grain tuning**
   - Game-ready terrain UV scale reduced from `10x` to `4x`.
   - Terrain normal map disabled for the current forward path.
   - Tree normal intensity reduced.
   - This avoids high-frequency aliasing until mipmap generation, anisotropic sampling and TAA exist.

## Next required AAA milestones

### Milestone 1 — GpuScene + instance batches

Current path still records many per-object draw calls. The next architectural step is:

```text
Primitive/Prefab extraction
  → stable instance key
  → per-mesh instance buffer
  → one draw_indexed(instance_count=N) per mesh/material
```

Target result:

```text
opaque_forward: indexed ≈ 8..24
shadow_casters: indexed ≈ 8..24
```

instead of per-object/pseudo-prefab replay.

### Milestone 2 — real sampleable depth shadows

Current shadow pass stores depth in a color RT for compatibility. Replace with:

```text
Depth32 shadow texture
comparison sampler
receiver-plane bias
CSM split selection
stable cascades snapped to texel grid
```

### Milestone 3 — post stack

The renderer should move to:

```text
linear scene color
exposure
TAA
bloom
color grading
tonemap
sharpen
UI composite
```

### Milestone 4 — lighting architecture

Absorb the `renderer.zip` shape:

```text
Light extraction
  → light classification
  → tile/cluster culling
  → shadowed light selection
  → LOD/visibility policy
```

## Guardrail

Do not solve AAA quality by increasing per-frame brute force. The foundation should prefer:

- stable phase graph;
- visibility budgets;
- resource residency gates;
- GPU lifetime queues;
- draw-list statistics;
- instance batches;
- declarative quality profiles.
