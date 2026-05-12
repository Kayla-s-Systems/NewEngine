# newengine-render-frame-graph

Declarative stable Render API frame planning layer.

This crate owns the runtime-facing frame graph contract:

```text
FrameGraphBuilder
  .shadow_map()
  .viewport_gbuffer_or_forward()
  .lighting()
  .postfx()
  .ui_composite()
  .submit()
```

It does not own backend resources and does not execute Vulkan/DirectX commands directly.
Backends receive `RenderGraphDesc` through stable Render API and may either compile/execute it natively
or validate it while the runtime still uses immediate callbacks during the transition period.
