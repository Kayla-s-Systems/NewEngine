# Game-first runtime boundary

## Hard rule

A standalone game is an application consumer of engine services. It must not own renderer internals.

```text
game-ready-fps app
  -> newengine-game-runtime profile / game bootstrap
  -> newengine-engine-runtime scene + render controller
  -> newengine-render-api service contract
  -> renderer plugin, e.g. newengine.renderer.vulkan
```

The game layer may:

- select a runtime profile;
- provide an app asset root;
- provide a scene/profile asset;
- request gameplay startup;
- consume input/runtime services.

The game layer must not:

- create Vulkan resources;
- create pipelines;
- perform texture uploads;
- decide shadow passes;
- build post-processing passes;
- patch render service protocol variants;
- fallback around an outdated render plugin ABI.

## Crate split

| Crate | Role |
|---|---|
| `newengine-game-runtime` | Thin standalone game profile and bootstrap gates. |
| `newengine-engine-runtime` | Reusable scene bridge, gameplay components, viewport bridge and render controller. |
| `newengine-render-api` | Backend-neutral render service protocol and resource contracts. |
| `vulkan_renderer` plugin | Vulkan backend implementation: GPU resources, upload queue, pipeline cache, shader bake/cache, shadows/postFX execution. |
| `newengine-editor-runtime` | Editor composition and UI; reuses `newengine-engine-runtime` for shared runtime systems. |

## ABI policy

Render service protocol mismatch is a build/deploy error. Do not add fallback branches for missing `RenderServiceRequest` or `RenderCommand` variants. Rebuild and resync the renderer plugin when the render API changes.
