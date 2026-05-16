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


## HDR scene -> tonemap -> display contract

The runtime frame graph now reserves a separate linear scene color resource:

```text
RG_SCENE_HDR_COLOR : Rgba16Float, linear HDR world/material output
RG_SURFACE_COLOR   : swapchain/UI/debug composite target
```

Material shaders must write linear lighting into `RG_SCENE_HDR_COLOR` when `hdr_scene_enabled` is set.
They must not apply gamma or display encoding themselves. Display conversion belongs to the `PostFx` phase:

```text
viewport_forward -> scene_hdr_color
postfx           -> tonemap/display encode into swapchain
ui_composite     -> UI over display color
debug_overlay    -> debug over final surface
```

This keeps bloom, exposure, color grading and future HDR monitor output behind one shader-owned post-processing base.
