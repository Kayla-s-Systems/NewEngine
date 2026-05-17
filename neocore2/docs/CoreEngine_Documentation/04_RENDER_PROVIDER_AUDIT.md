# 04 — Render provider audit

## Summary

Render is already on the target backend-plugin boundary.

```text
Renderer provider plugin
  descriptor provides render.api + render.backend
  init_v3 registers ServiceV1 under render.api
      ↓
RenderBackendRuntimeModule
  RenderServiceClient
  provider resolver validates service owner capability
  ServiceBackedRenderApi
  RenderApiRef
      ↓
RuntimeRenderController
  RenderFrameEnvelope
```

## Clean points

- Native renderer implementation is plugin-owned.
- Runtime-host no longer contains in-process null renderer fallback.
- Backend selection config/aliases are removed.
- Runtime render controller calls `dyn RenderApi` only.
- GameReady material/render feature policy is profile-owned, not reusable engine-runtime backend code.
- Backend provider discovery is descriptor/capability-based rather than filename-based.

## Remaining render work

- provider conformance tests;
- zero-warning CI for visibility warnings;
- reload/recreate tests;
- binary packet option for larger hot path payloads if JSON/control packets become too expensive.
