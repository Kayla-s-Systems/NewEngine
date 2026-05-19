# Render domains, UI backdrop blur and platform shortcut interception — 2026-05-19

## Scope

This pass turns three one-off seams into engine-domain contracts:

```text
engine.render            canonical render backend gateway
engine.render.render3d   3D/world render domain intent
engine.render.render2d   2D/UI render domain intent
engine.input.bindings    semantic input binding ownership
engine.ui                provider-owned UI draw surface
engine.audio             semantic UI feedback events
```

The existing stable gateway remains `engine.render`. The third-level render domains are subdomain intent routed through render frame envelopes and render graph pass metadata, not a replacement for the provider backend route.

## ESC policy

ESC is a semantic menu action registered through `engine.input.bindings`. The platform shell maps physical keys into canonical key codes, but it does not own pause-menu behavior and does not close the game as a root shortcut.

Decision order:

```text
ESC pressed
  -> platform key mapping
  -> engine.input
  -> engine.input.bindings
  -> engine.menu.toggle_pause
  -> RenderPauseMenuRuntimeState
```

Game exit is now an explicit `Exit` item in the pause menu. It requests shutdown through the engine lifecycle token, not through a hardcoded platform key path.

## UI backdrop blur

The pause menu still publishes declarative UI state:

```text
UiPauseMenuState.animation_alpha
UiPauseMenuState.backdrop_opacity
UiPauseMenuState.blur_radius_px
```

The render controller converts that state into `PostFxFrameParams.ui_backdrop`. The frame graph inserts a `UiBackdropBlur` pass before `UiComposite` when the menu is visually active.

Provider path:

```text
pause menu state
  -> UiBackdropPostFxParams
  -> PostFxFrameParams.ui_backdrop
  -> StandardRuntimePipelineDesc.ui_backdrop_blur
  -> RenderGraphPassKind::UiBackdropBlur
  -> Vulkan postfx fullscreen pass
  -> UI composite draws the modal over the blurred/dimmed frame
```

The egui provider no longer fakes blur with layered translucent slabs. It owns layout and dim card drawing only.

## 3D / 2D split

Render graph passes now carry `RenderGraphPassDomain`:

```text
Render3d      world/depth/shadow/gbuffer/forward/deferred
PostProcess   scene/postfx chain
Render2d      UI backdrop blur, UI composite, debug overlay
Presentation  reserved for future swapchain/display passes
```

`RenderFrameEnvelope.domains` carries coarse frame-domain intent, including whether UI postprocess is active. This lets future providers schedule 3D and 2D effects independently without hardcoding phase names in gameplay.

## Player model correction

The runtime model root is now grounded relative to the player capsule instead of being attached at capsule center. MTL parsing now captures diffuse/normal texture declarations and resolves canonical `.neytd@entry` texture references when authoring tools provide runtime dictionaries. If only source DDS maps are present, the material falls back to slot-aware colors instead of a white untextured mannequin.

## Non-goals

- `engine.render` is not renamed.
- DDS is not allowed as a runtime material texture reference.
- The current UI blur pass is the first native render-graph path; future provider work can replace it with separable downsample/blur/upsample resources.

## 2026-05-19 correction

`engine.render.render3d` and `engine.render.render2d` remain render graph pass domains, not service gateway domains. Service gateway extension points are now `engine.render.effects` and `engine.render.materials` so third-level service domains complement the parent gateway cleanly.
