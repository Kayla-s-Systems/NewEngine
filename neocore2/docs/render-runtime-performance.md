# Render Runtime Performance Pass

This pass moves the renderer toward a render-base architecture where heavy GPU work is staged, measurable, and backend-neutral.

## Goals

- Avoid hidden texture uploads inside material draw paths.
- Make material texture state explicit: `Missing`, `Loading`, `Ready`, `Failed`.
- Keep upload work bounded by a per-frame budget.
- Warm core pipelines during loading-screen startup instead of the first visible frame.
- Persist Vulkan pipeline cache data between runs.
- Cache dynamically baked runtime shaders on disk.
- Replace ad-hoc render-target destruction queues with a render-graph-style lifetime queue.

## Runtime contracts

The render API exposes these new neutral contracts:

- `UploadPumpDesc` / `UploadPumpReport`
- `TextureResidencySnapshot`
- `PipelineWarmupDesc` / `PipelineWarmupReport`
- `ShaderRuntimeCacheStats`
- `RenderGraphResourceLifetime`

The host can now ask the backend to pump uploads explicitly, query residency, warm pipelines, and inspect cache/queue diagnostics through `render.api.v1`.

## Vulkan behavior

The Vulkan backend now keeps a staged texture upload queue. Textures created with deferred data are inserted as GPU objects first, then their payload is uploaded through bounded pump calls. Material bindings use fallback textures until `TextureResidencySnapshot.state == Ready`.

The backend also keeps a Vulkan pipeline cache at:

```text
cache/render/vulkan-pipeline-cache.bin
```

Override with:

```text
NEWENGINE_VULKAN_PIPELINE_CACHE=<path>
```

## Shader runtime cache

`newengine-shader-compiler` now stores dynamically compiled SPIR-V at:

```text
cache/shaders/runtime
```

Override with:

```text
NEWENGINE_SHADER_CACHE_DIR=<path>
```

Disable runtime shader cache with:

```text
NEWENGINE_SHADER_RUNTIME_CACHE=off
```

Force runtime glslc compilation instead of prebaked lookup with:

```text
NEWENGINE_SHADER_BAKE_MODE=runtime
```

Use strict runtime mode to fail instead of falling back to prebaked shaders:

```text
NEWENGINE_SHADER_BAKE_MODE=strict-runtime
```

## Important invariant

The renderer still does not know whether assets came from loose files or `.pak`. Asset resolution remains owned by AssetManager/VFS. The renderer consumes decoded bytes and stable render descriptors only.
