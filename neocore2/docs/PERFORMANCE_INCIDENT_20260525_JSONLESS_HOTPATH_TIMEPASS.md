# Performance incident 2026-05-25 — JSONless hot path + realtime fixed-step pass

> [!INFO] INFO BLOCK — текущее положение дел
> **У нас сейчас:** FPS держится около 15.6 даже после перевода modal UI и texture decode на более лёгкий путь. Свежий profiler показывает не один огромный stall, а постоянный frame-tax: `plugin_lifecycle`, `service_call` JSON diagnostics, `logging.sink.write_json`, `render.submit`, и fixed-step catch-up.
>
> **Technical details (EN):** profiler snapshot `profiler_report_20260525_205159_422Z.zip`: `plugin_lifecycle=243.374ms`, `service_call=265.765ms`, `logging.sink.write_json=73.762ms`, `render.api::invoke_json=63.905ms`, `time.advance_fixed_v1=96 calls`, plugin `fixed_update` around `96-99 calls` per short run.

## Decision

Runtime hot path must not use JSON as an internal engine/plugin transport.

```text
Allowed JSON:
  user/API edge
  tools/import-export
  debug snapshots
  profiler/report files
  manifests/descriptors

Forbidden JSON on hot path:
  per-frame service diagnostics
  per-plugin lifecycle begin/end
  per-fixed-tick job envelopes
  render-frame command traffic when binary exists
```

## What changed in this pass

### 1. Plugin lifecycle JSON diagnostics removed from hot path

`PluginManager::call_plugin()` no longer creates JSON `begin/end` task envelopes for every:

```text
fixed_update
update
render
```

It now performs a direct ABI call and logs only slow/error paths.

### 2. Service-call JSON diagnostics removed from hot path

`call_service_v1()` no longer creates JSON diagnostics for every service call.

It still:

```text
routes through engine gateway registry
calls selected provider service
catches provider panic
logs slow/error calls compactly
```

But it does not serialize `serde_json::json!` begin/end events for every call.

### 3. Realtime fixed-step debt policy

`engine.time` and `Engine::begin_frame()` now default to:

```text
max fixed steps per visible frame = 1
no render-thread catch-up backlog
excess debt is dropped in realtime profile
```

This is not an FPS cap. The platform/render loop remains free to run as fast as it can. The change only prevents simulation fixed-update work from multiplying when a frame is already late.

### 4. Winit playable loop defaults to Poll

Playable runtime now defaults to:

```text
ControlFlow::Poll
```

instead of waiting for redraw-idle cadence. If a slower debug mode is needed, it can be explicitly requested:

```bat
set NEWENGINE_PLATFORM_FRAME_DRIVER=wait
```

### 5. Winit early frame logging is opt-in

Per-frame early log writes are disabled unless:

```bat
set NEWENGINE_WINIT_FRAME_EARLY_LOG=1
```

This avoids file flushes in the frame driver.

## Expected profiler changes

The next profiler report should show a large drop or disappearance of:

```text
newengine-plugin-host:plugin_lifecycle::*::fixed_update
newengine-plugin-host:service_call::service:newengine.logging.sink.v1::write_json
newengine-plugin-host:service_call::service:render.api::invoke_json
newengine-plugin-host:service_call::service:time.api::time.advance_fixed_v1
```

`render_controller:render cpu profile/submit` may remain if the GPU/backend submit path is now the true bottleneck.

## Next architectural pass

This pass removes JSON diagnostics from the hot path, but it does not yet complete the whole binary-internal migration.

Target model:

```text
call_service_v1:
  remains sync for small control/query methods

heavy work:
  submit job -> ticket
  worker executes provider call
  render/sim polls ticket
  apply stage consumes binary result

internal DTO transport:
  binary by default
  JSON only at user/tool/debug boundaries
```

The next pass should introduce explicit binary methods for time/input/UI telemetry and remove remaining `*_json` calls from frame code.
