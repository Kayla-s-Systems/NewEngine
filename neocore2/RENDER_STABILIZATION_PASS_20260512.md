# NewEngine render/runtime stabilization pass — 2026-05-12

## Scope

This pass targets issues visible in the provided `game-ready-fps` log and aligns the engine with the attached mature renderer reference at the architecture level without attempting a full renderer rewrite.

## Findings from the log

1. **Shutdown crash candidate**
   - Process exits with `STATUS_ACCESS_VIOLATION` after module shutdown reaches `shutdown-system`.
   - `LoadedPlugin` kept `Library` before the ABI module object, so the dynamic library could be unloaded before the ABI object using vtables/destructors from that library was dropped.

2. **Dead legacy startup path**
   - `newengine-core::engine::module_boot` emitted dead-code warnings for `start_strict`, `start_resilient`, `complete_startup`, and `shutdown_slot`.
   - The active startup path is the incremental FSM/startup-graph pump, so the old strict/resilient helpers were legacy.

3. **First playable-frame stall**
   - The log shows a large gap while the scene launch gate waits for material texture residency.
   - The render controller still had a blocking helper path using AssetManager `wait_ready(..., 3s)` during material texture loading.

4. **Misleading viewport diagnostics**
   - Logs printed `viewport=0x0` even when direct surface rendering later adopted `1600x900`.
   - This made diagnostics look like a viewport bug while the effective rendering extent was valid.

5. **Render hot path weaknesses**
   - Draw-list replay shows `opaque_forward: recorded=647 indexed=129` with state binds matching draw count: `pipe=129,vb=129,ib=129,bg=129`.
   - This indicates missing material sorting / state bucketing / instancing for repeated foliage and primitive meshes.
   - The frame graph is still mostly forward-only: `shadow_map -> viewport_forward -> debug_overlay`.

## Changes applied

### Plugin lifetime / Windows shutdown crash

- Reordered `LoadedPlugin` so `Library` is the last field and is dropped last.
- This keeps the DLL loaded while ABI objects stored in `module` are dropped.

Files:
- `crates/newengine-plugin-host/src/manager/types.rs`
- `crates/newengine-plugin-host/src/manager/loader.rs`

### Core startup legacy cleanup

Removed no-longer-used legacy startup helpers:
- `start_strict`
- `start_resilient`
- `complete_startup`
- `shutdown_slot`

The active path remains the FSM + startup graph + incremental readiness dispatch.

File:
- `crates/newengine-core/src/engine/module_boot.rs`

### Non-blocking material texture residency

Changed material texture loading from a blocking render-loop path to a staged residency state machine:

```text
Requested -> AssetLoading -> GpuLoading -> Ready
                         \-> Failed
```

- `Requested`: path declared, no request sent yet.
- `AssetLoading`: AssetManager owns IO/import; render loop polls state.
- `GpuLoading`: CPU payload decoded, GPU upload queued; render loop polls GPU residency.
- `Ready`: renderable texture.
- `Failed`: explicit failure state with message.

Files:
- `crates/newengine-engine-runtime/src/render_controller/material_bindings.rs`
- `crates/newengine-engine-runtime/src/render_controller/resource_cache.rs`
- `crates/newengine-engine-runtime/src/render_controller/module_impl/readiness.rs`

### Render diagnostics cleanup

- `trace_render_begin()` now logs effective viewport and marks `direct_surface=true` when the viewport bridge has not published an explicit viewport yet.
- This removes false `viewport=0x0` suspicion from normal direct-surface startup.

File:
- `crates/newengine-engine-runtime/src/render_controller/module_impl/render_entry.rs`

## Reference renderer alignment

The attached renderer reference is structured around:

- `RenderPhases/*` and `fw/RenderPhaseList.*`: declarative phase orchestration.
- `DrawLists/*`: draw-list collection, sorting, profiling, and replay separation.
- `Deferred/GBuffer.*` and `Deferred/DeferredLighting.*`: GBuffer/deferred lighting path.
- `Lights/TiledLighting.*`: tiled light classification/culling.
- `Entities/InstancedEntityRenderer.*`: dedicated instancing path.
- `PlantsGrassRenderer.*` and `PlantsMgr.*`: specialized foliage management.
- `RenderTargetMgr.*`: centralized render target lifetime/ownership.
- `PostProcessFX.*`, `MLAA.*`, `AdaptiveDOF.*`: explicit post-process chain.

NewEngine's next architectural step should be a typed render-frame declaration layer, not more ad-hoc render-controller code.

## Recommended next pass

1. Introduce a declarative `RenderPhaseRegistry` and `RenderFrameRecipe`:
   - `ShadowMap`
   - `DepthPrepass`
   - `GBufferOpaque`
   - `LightingTiled`
   - `ForwardAlpha`
   - `SkyAtmosphere`
   - `PostFx`
   - `UiComposite`

2. Add draw-list bucketing:
   - sort by `pipeline -> material/bind-group -> mesh -> instance batch`;
   - collect replay counters before and after sorting;
   - target reduction of `pipe/vb/ib/bg` binds from per-draw to per-bucket.

3. Add foliage/primitive instancing:
   - `primitive_tree_cluster` and repeated prefab placement should not emit one draw path per tree;
   - create an instance buffer per prefab/material/lod bucket.

4. Replace duplicated diagnostics prints with one `StartupDiagnosticsEmitter` / `PluginDiagnosticsEmitter` policy.

5. Add runtime frame-time buckets:
   - CPU extraction
   - draw-list build
   - graph build
   - upload submit
   - GPU frame
   - present wait

## Validation note

This patch was prepared in an environment without `cargo`, `rustc`, or `rustfmt`, so syntax/build verification must be run on the Windows machine with the project toolchain installed.
