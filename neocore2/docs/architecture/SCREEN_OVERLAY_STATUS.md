# ScreenOverlayStatus foundation

`ScreenOverlayStatus` is the platform-shell status surface for work that happens before or outside the runtime UI provider:

- startup and staged bootstrap;
- plugin discovery and synchronization;
- asset/font loading before UI is available;
- applying staged changes or patch-like commits;
- degraded/fallback states where the event loop is alive but a backend is disabled;
- recoverable startup diagnostics.

It is intentionally **not** renderer debug text and not gameplay UI.

## Ownership

```text
runtime-host / platform-shell -> ScreenOverlayStatus -> PlatformLoadingOverlayV1 -> platform renderer
runtime UI provider           -> normal editor/game UI
renderer debug text           -> immutable renderer-owned dev mark only
```

The renderer must not receive arbitrary status/UI strings through debug text. The platform shell is allowed to draw this overlay because it is the process-level bootstrap/fallback surface.

## Design rules

1. The ABI-facing `PlatformLoadingOverlayV1` remains small and backward-compatible.
2. Rich semantics live in `newengine-runtime-host::platform_runtime::screen_overlay`.
3. Platform plugins map the minimal wire payload into local visual state.
4. Animation is stateful and monotonic: text reveal clamps instead of looping, so degraded/error messages do not blink.
5. Terminal states (`Ready`, `Degraded`, `Error`) do not show an infinite spinner.
6. Fonts are resolved through `AssetManager + fontImporter` after asset services are available, with a system fallback before that.

## Current first implementation

The Windows/winit platform renderer now has:

- `ScreenOverlayPresenter` for classification, reveal timing, pulse and spinner phase;
- styled Win32 GDI rendering with Inter memory-font registration;
- kind-aware palettes for loading, sync, apply, ready, degraded and error;
- a production-safe degraded overlay that communicates fallback instead of looking like a hung loading screen.

## Future expansion

The next production layer should add a structured overlay queue/resource:

```rust
ScreenOverlayStatus {
    kind,
    owner,
    operation_id,
    title,
    status,
    detail,
    progress,
    started_at,
    expected_next_state,
}
```

That queue can be fed by asset sync, plugin sync, job-system phases, render backend recovery and staged filesystem commits.
