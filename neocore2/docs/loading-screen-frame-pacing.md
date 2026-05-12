# Loading screen frame pacing

The loading screen is split into two layers:

1. **Truth layer** — core/runtime FSM status.
   - `EngineStartupSnapshot`
   - `ScreenOverlayStatus`
   - subsystem phases and diagnostic details

2. **Presentation layer** — native loading visual interpolation.
   - spinner angle
   - pulse value
   - visual progress bar
   - staged card enter animation
   - message throttling

The FSM is authoritative, but it is intentionally not used as the animation clock.
Startup work can be bursty: plugin discovery, importer probing, Vulkan service binding,
module init and scene residency can take uneven amounts of wall time. If visual animation
is advanced directly by those work steps, the splash looks heavy: the spinner accelerates
and slows down, the progress bar jumps, and status text changes too quickly to read.

## Rules

- Runtime/core owns real status.
- Platform presenter owns visual interpolation.
- Progress is monotonic during non-error startup.
- Text diagnostics are still logged immediately, but the splash holds a readable message
  for a short minimum duration.
- Stage cards only replay their slide-in when the stage topology changes, not whenever
  the FSM publishes a new status string.
- Native loading redraw is capped to a stable 60 Hz cadence through winit `WaitUntil`.

## Current implementation

- `newengine-runtime-host/src/platform_runtime/runtime_host.rs`
  - maps core startup progress into the global bootstrap range;
  - prevents bootstrap progress regression;
  - reduces noisy progress-only log spam.

- `newengine-platform-winit/src/runtime_app/frame_pump.rs`
  - updates host/runtime state once per frame;
  - does not draw twice per pump.

- `newengine-platform-winit/src/runtime_app/loading/status.rs`
  - derives spinner phase from monotonic wall-clock time;
  - smooths global and per-stage progress;
  - throttles visible status/detail text;
  - decouples stage-card enter animation from status text churn.
