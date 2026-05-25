# PERFORMANCE PASS — UI binary hot path and frame ownership

> [!INFO] INFO BLOCK — текущее положение дел
> **У нас сейчас:** первый post-log-cleanup profiler report показал, что главный FPS offender — не логирование, а `engine.ui` modal path: `aurelia.ui.api::draw_frame_v1` выполнялся через JSON и занимал до ~55 ms за вызов. Одновременно существовал быстрый `draw_frame_bin_v1`, но он вызывался из другого владельца кадра.
>
> **Technical details (EN):** Snapshot: `NorthStar-Engine-source-20260525-224346.zip`. Report: `profiler_report_20260525_194330_942Z.zip`. Top offender: `service:aurelia.ui.api::draw_frame_v1`; fast path already present: `service:aurelia.ui.api::draw_frame_bin_v1`.

## Problem

The runtime did two UI jobs for the same visible modal frame:

```text
runtime-host before engine.step()
  -> publishes previous pause menu state
  -> requests engine.ui draw_frame_bin_v1

render_controller inside engine.step()
  -> updates actual same-frame modal state
  -> publishes pause menu state again
  -> requests engine.ui draw_frame_v1 as JSON
```

This created two defects:

```text
1. stale work: runtime-host draws previous modal state before render_controller updates it
2. expensive work: render_controller then serializes the real modal draw-list through JSON
```

The binary path existed, but the modal owner did not use it.

## Fix in this pass

### 1. `render_controller` modal UI now uses binary draw-frame

`newengine-engine-runtime/src/ui_gateway.rs` now tries:

```text
engine.ui / draw_frame_bin_v1
```

and falls back to:

```text
engine.ui / draw_frame_v1
```

only if the selected provider does not support the binary method.

### 2. `runtime-host` no longer owns pause-menu UI frames

`runtime-host` keeps ownership of:

```text
loading overlay
runtime debug telemetry
idle gameplay HUD cache
native/minimal provider fallback
```

`render_controller` owns:

```text
pause menu
asset-browser modal overlay
same-frame modal draw-list refresh
```

This removes duplicate `engine.ui` work during modal frames.

### 3. Aurelia no longer resends font atlas every modal/debug frame

Font atlas upload is a texture state change, not per-frame draw state.

Correct behavior:

```text
first visible frame -> send atlas
startup recovery keyframes -> send atlas
periodic recovery keyframe -> send atlas
ordinary modal/debug frame -> send vertices/indices only
```

Previous behavior:

```text
pause menu visible -> send atlas every frame
runtime debug overlay visible -> send atlas every frame
```

That behavior was especially expensive over JSON and still wasteful over binary.

## Binary protocol rule

Hot path communication should use binary DTO packets:

```text
frame UI draw-list
render command batches
future time snapshots
future input frame snapshots
future physics frame DTOs
```

JSON remains allowed only for:

```text
info_json
diagnostics
manifest/config/control calls
human-readable traces
import/export tooling
fallback compatibility during migration
```

## Task boundaries after this pass

```text
runtime-host
  owns platform shell, loading overlay, provider UI cache, input ingress before engine.step

render_controller
  owns same-frame render resources, modal UI, Asset Browser blocking overlay, render frame envelope

engine.ui provider
  owns UI layout/draw-list generation and texture delta production

renderer
  owns GPU upload/retained texture ids and compositing UI draw packets
```

## Expected profiler change

The next profiler report should no longer show this as the top offender:

```text
service:aurelia.ui.api::draw_frame_v1
```

Expected replacement:

```text
service:aurelia.ui.api::draw_frame_bin_v1
```

with modal frame cost closer to the previous binary average, not the JSON worst-case.

If FPS is still low after this pass, the next real offender to attack is likely:

```text
render cpu profile/submit
render cpu profile/feature_extract
time fixed-step catch-up if frames still exceed budget
```

## Next pass

```text
P1: binary DTO for pause_menu_state_v1 and debug_overlay_telemetry_v1
P2: binary TimeSnapshotV1 for begin_frame/advance_fixed
P3: retained render extraction and dirty draw-list packets
P4: render submit batching + stable shadow/cache invalidation
```
