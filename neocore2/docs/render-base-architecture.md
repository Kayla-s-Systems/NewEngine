# Render Base Architecture

NewEngine render code must be backend-neutral above the renderer plugin boundary. Vulkan, future WGPU/DX12 backends, headless test renderers and preview renderers all consume the same value/protocol contracts from `newengine-render-api`.

## Layering

```text
Gameplay / editor tools / scene bootstrap
  -> render controller / render base orchestration
  -> newengine-render-api value contracts
  -> render.runtime.loader service bridge
  -> backend plugin service: render.api.v1
  -> Vulkan/WGPU/DX12 implementation
```

Only the backend plugin may know about backend-native handles. Engine code must use stable ids such as `TextureId`, `BufferId`, `PipelineId`, `BindGroupId` and declarative descriptors such as `TextureDesc`, `PipelineDesc` and `BindGroupDesc`.

## Required contracts

The render base layer is expected to reason about:

- backend capabilities: `RenderBackendCapabilities`, `RenderFeature`, `RenderLimits`;
- frame pacing: `RenderFramePacingConfig`;
- resource/upload budget: `RenderWorkBudget`;
- diagnostics snapshots: `RenderDiagnosticsSnapshot`;
- material/resource creation through descriptors, not backend-specific calls.

## Stutter policy

Interactive frames must avoid unbounded synchronous GPU work. Blocking work is still allowed for bootstrap and tiny fallback resources, but runtime resource creation should move toward staged, budgeted paths:

```text
asset bytes -> decode/cook -> upload queue -> per-frame budget -> GPU resource ready event
```

The backend now exposes diagnostics through `RenderServiceRequest::DiagnosticsSnapshot`. Use it to locate:

- long blocking texture uploads;
- pipeline compilation spikes;
- excessive resource churn;
- upload queue backlog;
- slow `begin_frame` / `end_frame` phases.

## One render contract

- Renderer code does not read loose files directly.
- Pak and loose assets remain interchangeable behind AssetManager/VFS.
- Render controller code should not branch on Vulkan-specific details.
- Gameplay/editor code should not create backend-native resources.
