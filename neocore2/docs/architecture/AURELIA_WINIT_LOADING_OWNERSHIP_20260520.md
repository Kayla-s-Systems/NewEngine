# Aurelia / Winit Loading Ownership — 2026-05-20

## Decision

`platform-winit` must not draw UI. Loading screen presentation is an Aurelia `engine.ui` surface.

The platform plugin owns:

- OS window creation;
- native handles;
- input/event loop pumping;
- cursor/focus/window events.

The platform plugin does **not** own:

- loading widgets;
- GDI/Win32 loading compositor drawing;
- UI styling;
- UI texture ownership;
- text or menu rendering.

## Runtime path

```text
engine.loading state
  -> runtime projection
  -> engine.ui / aurelia.ui.api
  -> UiDrawList + UiTextureDelta
  -> engine.render UI bridge
  -> renderer backend composite pass
```

## Loading background

The loading background is referenced as a normal Aurelia XML image:

```xml
<image id="boot.background" src="loading/loading_ui.neytd@loadbg" anchor="fill" fit="cover" />
```

The runtime texture source is `.neytd`, not PNG. Aurelia decodes the provider-owned default `loading/loading_ui.neytd@loadbg` into a UI texture delta and emits a textured full-screen quad. The renderer only receives draw packets and texture deltas; it does not know about Aurelia XML or loading state.

## Winit fallback policy

The old native loading compositor was removed from active presentation. `LoadingCompositor` is intentionally a no-op state sink so existing platform lifecycle hooks remain simple without granting Winit any UI-rendering authority.

If an emergency pre-render fallback is needed later, it must be implemented as an explicit policy mode and must not become a second UI system.
