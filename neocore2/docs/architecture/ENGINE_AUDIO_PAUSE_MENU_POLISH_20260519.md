# Engine Audio + Premium Pause Menu Polish — 2026-05-19

## Summary

This pass turns pause-menu feedback into a real engine-domain event flow instead
of embedding sound behavior inside UI code.

```text
engine.input.bindings -> semantic menu actions
engine runtime        -> pause/menu state + audio feedback events
engine.ui             -> premium modal draw-list projection
engine.audio          -> semantic audio event gateway
```

## New gateway

```text
engine.audio
  provider service: engine.audio        # current engine-owned queue provider
  future provider:  audio.api / vendor.audio.api
  capability:       audio.backend
  service_kind:     audio
```

The current implementation is intentionally lightweight: it queues and logs
semantic events such as `ui.menu.open`, `ui.menu.navigate`, `ui.menu.confirm`,
`ui.menu.back`, `ui.menu.close`, and `ui.menu.rebind`. A future mixer plugin can
implement `audio.api + audio.backend` and override the engine-owned route without
changing UI/gameplay code.

## UI polish

The pause menu state now carries presentation hints:

```text
animation_alpha
backdrop_opacity
blur_radius_px
```

The egui UI provider uses these hints to draw:

- animated slide/fade modal entry;
- dimmed gameplay backdrop;
- faux blur layers until the renderer exposes a true UI postprocess blur;
- premium dark/gold panel layout;
- side information rail with input/config/audio routing context.

## Runtime behavior

When the modal is logically open:

- gameplay simulation is paused;
- player/camera input is suppressed;
- cursor capture is released;
- provider UI remains refreshed every frame.

During close animation, UI can continue drawing a fade-out while gameplay input
is already allowed again.

## Build fix included

The prior pause-menu pass left `UiRuntimeDebugOverlayTelemetry` without its
`Serialize/Deserialize` derive while retaining `#[serde(default)]` fields. This
pass restores the derive and keeps the UI API serializable.
