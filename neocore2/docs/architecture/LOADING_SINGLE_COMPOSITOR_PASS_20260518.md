# Loading screen single-compositor cleanup — 2026-05-18

## Problem

The loading screen had two active presentation paths in the platform shell:

```text
WindowEvent::RedrawRequested -> old native loading overlay
newengine-loading-compositor -> independent native compositor thread
```

Both paths painted into the same Win32 window. Under startup load this caused flicker, jitter and occasional exposure of stale menu/runtime pixels behind the loading screen.

## Decision

`newengine-loading-*` remains the domain/gateway/API layer. `platform-winit` keeps only one native loading presenter:

```text
PlatformLoadingOverlayV1
  -> LoadingCompositor::publish_platform_overlay
  -> SharedLoadingSnapshot
  -> newengine-loading-compositor thread
  -> offscreen GDI backbuffer
  -> single full-window BitBlt
```

The old `RedrawRequested` loading overlay path is removed from the active platform shell. The platform event loop no longer requests redraw while loading is active; the compositor thread owns loading visuals until handoff.

## Removed platform shell pieces

```text
runtime_app/loading/win32.rs
runtime_app/loading/font.rs
runtime_app/loading/image_asset.rs
runtime_app/loading/layout.rs
runtime_app/loading/status.rs
runtime_app/loading/interaction.rs
runtime_app/loading/card_style.rs
runtime_app/loading/compact_error.rs
runtime_app/loading/brand.rs
```

The `newengine-assets` dependency was also removed from `platform-winit`; loading visuals no longer decode provider-owned loading images inside platform shell.

## Flicker fixes

- Window starts hidden and becomes visible only after compositor setup.
- Loading frames render into an offscreen compatible DC/bitmap.
- Each compositor tick performs one full-frame `BitBlt`.
- Loading-active frame pump does not call `request_redraw()`.
- `WindowEvent::RedrawRequested` no longer paints a second loading overlay.

## Invariant

```text
One HWND.
One loading presenter.
One compositor-owned loading frame.
No legacy loading overlay path racing the compositor.
```
