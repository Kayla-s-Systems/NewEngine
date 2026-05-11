# Renderer performance readiness

## Current target

The renderer plugin should be able to run as an engine render backend, not as game-specific code.

```text
Render controller
  -> declarative render-api commands
  -> renderer plugin
  -> GPU execution
```

## Upload queue

Runtime texture uploads use a persistent transfer context ring:

```text
TextureDataPolicy::Deferred
  -> staged upload queue
  -> persistent upload command pool/context
  -> fence-tracked staging lifetime
  -> residency: Queued -> Uploading -> Ready / Failed
```

The upload pump is frame-budgeted by `RenderWorkBudget`:

- max upload bytes per frame;
- max upload jobs per frame;
- no `queue_wait_idle` in the staged material upload path;
- staging buffers are freed only after the upload fence is signaled.

Timeline semaphore support is intentionally treated as a backend capability gate. The Vulkan backend should not advertise it until logical device creation enables the required Vulkan 1.2 feature or extension.

## Shadows

Shadows must remain renderer-owned:

- scene/game declares lights and shadow settings;
- engine render controller extracts render data;
- renderer plugin owns shadow render targets, depth formats, barriers, samplers and pass execution.

For game-ready quality, the next renderer milestone is stable CSM ownership in render graph terms:

```text
ShadowMapAtlas / Cascades
  lifetime: persistent/resized by settings
  usage: DepthAttachment -> SampledTexture
  consumers: LitForward / Terrain / Foliage
```

## Shaders

Shader compilation policy:

- baked/precompiled shader pack first;
- runtime bake only when allowed by `NEWENGINE_SHADER_BAKE_MODE`;
- disk cache key must include stage, entry point, logical name and source hash;
- loading screen should warm required pipelines before first playable frame.

## Post effects

Post effects should be graph passes, not ad-hoc game code:

```text
SceneColor HDR
  -> exposure
  -> bloom/downsample/upsample
  -> color grading
  -> tonemap
  -> sharpen
  -> UI composite
```

Until the render graph owns all postFX resource lifetimes, postFX must stay behind renderer capabilities and never leak into game bootstrap code.
