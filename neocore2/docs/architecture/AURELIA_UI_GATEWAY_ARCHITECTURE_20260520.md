# Aurelia UI gateway architecture

## Verdict

Aurelia UI is the first-party UI system name, but it must not become a privileged in-engine SDK. The clean shape is:

```text
consumer/runtime -> engine.ui
                 -> ActiveGatewayRegistry
                 -> aurelia.ui.api provider service
                 -> UiDrawList / UiDrawFrame packets
                 -> engine.render renderer bridge
                 -> active renderer provider
```

`legacy egui tooling_ui_provider` is legacy/debug-only. It may remain useful for quick internal tooling, but it is not the strategic game/runtime UI provider.

## Engine As Host invariant

```text
Engine owns the gateway.
Aurelia owns the implementation.
Text is a dynamic Aurelia/UI subdomain behind engine.ui.text, not a separate top-level engine service.
Render owns GPU execution behind engine.render.
The registry owns provider selection.
```

Aurelia is therefore a provider candidate for `engine.ui`, not a core dependency and not a Vulkan submodule.

## Layers

```text
newengine-ui-api
  engine.ui constants, service methods, UiFrameRequest/Response, UiDocumentRequest/Response

newengine-ui-draw
  renderer-neutral draw packets: UiDrawList, UiMesh, UiTextureDelta, UiDrawCmd

newengine-text-api
  engine.ui.text constants and DTOs for font fallback, shaping, atlas planning, localization

Plugins/AureliaUiProvider
  aurelia.ui.api + ui.backend provider
  XML documents
  retained tree / immediate draw-frame projection
  standard controls: label, panel, progress, menu-list, text-list, reticle

engine.render
  consumes UiDrawList through RenderCommand::SetUiDrawList
  must not know Aurelia internals
```

## Loading screen rule

`engine.loading` remains the state/progress domain. The visual shell is a normal Aurelia surface named `engine.loading`. Native loading compositor is a pre-render/provider-missing fallback only.

## Renderer bridge rule

Aurelia never imports Vulkan internals. It emits `UiDrawList` packets. The renderer provider is free to implement the packet however it wants: Vulkan, Null, future DX12/Metal, remote renderer, capture renderer.

## Text rule

Aurelia may provide the first `engine.ui.text` route, but `engine.ui.text` is a dynamic UI subdomain declared by provider metadata, not a hardcoded core startup service. The target path is:

```text
Aurelia layout/text nodes
  -> engine.ui.text shape/font fallback/atlas plan
  -> UiDrawList glyph quads with atlas texture refs
  -> engine.render bridge
```

## Provider metadata

```json
{
  "service_kind": "ui",
  "engine_gateway": "engine.ui",
  "contract": "aurelia.ui.api",
  "backend_priority": 500
}
```

## Non-goals

- No Scaleform runtime.
- No SWF/ActionScript dependency.
- No direct Vulkan calls from UI.
- No hardcoded UI toolkit aliases in startup config.
- No special loading-screen rendering path once `engine.ui` provider is active.
