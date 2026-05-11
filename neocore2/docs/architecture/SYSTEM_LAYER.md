# NewEngine System Layer

`system.zip` is used as an architectural reference for production-engine system services: boot orchestration,
streaming install/apply progress, telemetry, benchmark, recovery and system-level status surfaces.

NewEngine separates these responsibilities into explicit layers:

```text
Core
  minimal contracts, resources, job-system, diagnostics types

System Layer
  boot orchestration
  screen overlay status
  settings apply policy
  telemetry
  benchmark
  recovery
  install/sync/apply progress

Platform Layer
  window
  OS integration
  native overlay drawing
  font registration
  input shell

Runtime Layer
  gameplay
  scene
  render controller
  physics
  terrain

Plugins
  renderer
  assets
  UI provider
  importers
```

## Crates

```text
crates/newengine-system-contracts/
  src/screen_overlay.rs
  src/task_status.rs
  src/settings_impact.rs
  src/diagnostics.rs
  src/recovery.rs

crates/newengine-system-runtime/
  src/screen_overlay_bus.rs
  src/startup_status_mapper.rs
  src/job_status_bridge.rs
  src/render_status_bridge.rs
  src/asset_status_bridge.rs
```

## Boundary rules

- Renderer debug text is renderer-owned dev mark only.
- UI provider owns runtime/editor UI.
- `ScreenOverlayStatus` owns platform/system state: boot, loading, syncing, applying, ready, degraded, recovery and errors.
- Platform shells render the overlay but do not invent subsystem messages manually.
- Runtime subsystems publish typed state; presentation mapping lives in `newengine-system-runtime`.

## Example

```rust
ScreenOverlayStatus {
    kind: ScreenOverlayStatusKind::Degraded,
    reason: ScreenOverlayReason::GpuDeviceLost,
    title: "NEWENGINE // DEGRADED MODE".into(),
    status: "Renderer backend degraded at end_frame".into(),
    detail: "Vulkan device lost during end_frame.queue_submit".into(),
    progress: None,
    terminal: true,
}
```

This keeps degraded mode visible, deterministic and diagnostic without freezing on a stale frame and without using renderer text as UI.
