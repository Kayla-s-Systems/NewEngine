# System Layer pass — 2026-05-12

This note records what was extracted from `system.zip` without copying its legacy C++ architecture into NewEngine.

## What is worth adopting

| `system.zip` area | NewEngine adaptation |
| --- | --- |
| `TaskScheduler.*` | Keep one engine-owned job system with explicit lanes/priorities. Do not let runtime/editor/plugins spawn unmanaged worker pools. Shutdown must join jobs before plugin service/DLL teardown. |
| `ThreadPriorities.h` | Treat thread priority as a declarative policy per lane: render-prep, streaming, asset-io, plugin, background. Do not hard-code per-module thread behavior. |
| `SystemInfo.*` | Move host hardware probing into a system-runtime provider so `SystemProbe` no longer reports CPU/GPU/VRAM as `<unknown>`. |
| `SettingsManager.*` | Settings must declare impact: hot-apply, render-device-reset, scene-reload, or app-restart. Runtime should not guess. |
| `ControlMgr.*` | Input must be a system service with explicit capture modes and fallback behavior. Camera code must never own OS cursor capture directly. |
| telemetry / benchmark / capture files | Add system-level diagnostics contracts, not ad-hoc logs inside renderer/runtime. |

## Applied in this pass

1. Cursor capture now has an explicit FSM-like platform state: `Released`, `Locked`, `Confined`, `EmulatedRecenter`.
2. Game-ready launch gate no longer uses two loose booleans; it now has `GameReadyWorldLaunchGatePhase`.
3. Engine shutdown now joins the engine job system before plugin shutdown, preventing queued jobs from touching unloaded plugin state.

## Next code pass

1. Add `newengine-system-contracts` with:
   - `SystemInfoSnapshot`
   - `SettingsImpact`
   - `SystemTaskStatus`
   - `SystemDiagnosticsEvent`
2. Add `newengine-system-runtime` with:
   - host probe provider
   - settings impact router
   - startup/loading status mapper
   - job-system telemetry bridge
3. Move remaining bootstrap overlay/status strings behind declarative status descriptors.
