# North Star Engine — Low FPS Incident 2026-05-25

## Verdict

The low FPS run was not caused by one tiny rendering bug. It was a runtime feedback loop:

```text
slow frame -> fixed-step catch-up -> hot-path DEBUG/TRACE logs -> more IO/formatting stalls -> larger delta -> more catch-up
```

The profiler report also did not represent the actual gameplay/render window: `newengine.profiler` was disabled during capability validation because it required `host.diagnostics.jobs.v1`, while the host provides `host.events.v1`. The report was flushed around startup and missed the real render-frame profiler samples.

## Evidence from uploaded run

- `newengine.log` contained `5777 DEBUG` and `1427 TRACE` lines for a short run.
- Top log producers were hot-path systems:
  - `time gateway`: 3507 lines
  - `engine.time`: 489 lines
  - `game-ready sky cycle`: 474 lines
  - `render material registry`: 473 lines
  - `render extraction`: 470 lines
  - `gameplay schedule`: 463 lines
  - `render shadow cache`: 367 lines
- `engine.time` ran catch-up almost every frame:
  - 489 sampled frames
  - average fixed ticks per frame: `6.17`
  - distribution: `4x1`, `5x92`, `6x263`, `7x88`, `8x45`
- Render CPU profile from the log showed the actual suspects:
  - average `total_ms`: `19.04 ms`
  - average `submit`: `8.18 ms`
  - average `feature_extract`: `7.26 ms`
  - average `shadow_plan`: `1.98 ms`

## Changes in this patch

### 1. Stop fixed-step spiral

- Default `max_fixed_ticks_per_frame` is reduced from `8` to `4`.
- `engine.time` clamps delta to the accumulator budget instead of a hard `250 ms` wall.
- Per-frame/per-fixed-tick time logs are removed.
- Catch-up is now reported as sampled `WARN`, not hot-path spam.

### 2. Keep diagnostics without killing FPS

Removed or sampled hot-path logs from:

- `time gateway: begin_frame`
- `time gateway: advance_fixed`
- `engine.time: begin_frame_v1`
- `game-ready sky cycle: time source=engine.time`
- `render material registry: pipeline cache hit`
- `render extraction: authority frame`
- `gameplay schedule: world authority`
- `render shadow cache: live refresh because shadow sample space changed`
- input per-frame summaries / rapid transition debug logs

### 3. Fix profiler activation

- Profiler now requires `host.events.v1`, which the host actually provides.
- It should remain active for the real frame run instead of writing a startup-only report.

### 4. Make profiler useful for frame cost

- Profiler now reads `elapsed_ms`, `duration_ms`, `total_ms`, nested metadata timing fields, and simple `"... N ms"` details.
- Render profiler samples are emitted independently from log output.
- Render breakdown strings are expanded into child records, so CSV top offenders can show parts like:
  - `render cpu profile/submit`
  - `render cpu profile/feature_extract`
  - `render cpu profile/shadow_plan`
  - `render cpu profile/pipeline`

## Expected result after rebuild

- Much smaller `newengine.log`.
- Profiler report generated at shutdown after the actual run, not immediately after plugin validation.
- `profiler_top_offenders_latest.csv` should finally show real render/runtime CPU suspects instead of only `0.000 ms` event-bus rows.
- Fixed-step catch-up should be bounded to a maximum of 4 per frame by default.

## Next fundamental pass

This patch removes the runaway diagnostics/catch-up loop. The next performance pass should attack the actual frame cost shown by the profiler:

1. Reduce `submit` cost through command batching, fewer per-frame service calls, and render packet compaction.
2. Reduce `feature_extract` through dirty flags, retained render packets, and broadphase extraction.
3. Fix shadow cache invalidation: current logs showed frequent refresh because shadow sample space changed; the proper solution is stable CSM projection / texel snapping / update policy, not just muting logs.
4. Add frame-budget CI: fail when a hot-path module logs per frame, allocates unbounded strings, or emits unsampled events in runtime loops.
