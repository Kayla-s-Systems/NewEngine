# 09 — Render strict service backend report

## Verdict

```text
Service backend plugin through render.api: yes
In-process renderer fallback removed: yes
Legacy backend aliases/settings removed: yes
GameReady render features removed from reusable runtime: yes
Descriptor/capability-based backend selection: yes
```

## Runtime path

```text
Renderer provider plugin
  -> render.api ServiceV1
  -> RenderServiceClient
  -> ServiceBackedRenderApi
  -> RenderApiRef
  -> RuntimeRenderController
  -> RenderFrameEnvelope
```

## Removed legacy concepts

```text
NullRenderApi in runtime-host
backend alias matching
NEWENGINE_RENDER_BACKEND
auto/default/headless/vulkan alias selection
StartupConfig.render_backend
GameReady renderer hardcoded shader/pipeline ownership in reusable engine-runtime
```

## Remaining hygiene

The current run still emits Rust visibility/dead-code warnings in render internals. These are not service-boundary violations but should be cleaned for release CI.
