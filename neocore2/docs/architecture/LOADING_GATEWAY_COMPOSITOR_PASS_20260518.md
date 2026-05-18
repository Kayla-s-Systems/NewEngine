# Loading gateway compositor pass — 2026-05-18

## Verdict

The loading screen is now treated as a domain boundary instead of platform-runtime decoration.

```text
engine.loading
  -> newengine-loading-api DTO/service contract
  -> newengine-loading-runtime shared snapshot + visual animator
  -> engine-owned loading gateway service
  -> platform-winit native compositor bridge
  -> independent compositor thread drawing the current startup snapshot
```

## Why

The old loading surface depended on the platform event loop calling `host.step_v1` and then receiving `RedrawRequested`. During plugin discovery, startup graph work, shader/resource warmup or scene residency waits, the same thread could be busy and the overlay visibly advanced in jumps.

The new shape separates three responsibilities:

```text
startup work       -> publishes loading snapshots
loading domain     -> owns DTOs and smooth visual projection
platform shell     -> owns HWND/native drawing and compositor lifetime
```

## New crates

```text
newengine-loading-api
  ENGINE_LOADING_SERVICE_ID = engine.loading
  LOADING_SERVICE_ID = loading.api
  LOADING_BACKEND_CAPABILITY_ID = loading.backend
  LoadingScreenSnapshot
  LoadingSubsystemSnapshot
  LoadingServiceInfo
  RuntimeServiceRequirementSpec

newengine-loading-runtime
  SharedLoadingSnapshot
  LoadingAnimator
  LoadingCompositorFrame
  ScreenOverlay/UiSurfaceProjection JSON subsystem extraction
```

## Runtime-host bridge

`newengine-runtime-host::platform_runtime::loading_gateway` registers an engine-owned `engine.loading` service and route candidate. The service supports:

```text
info_json
invoke_json
shutdown_v1
snapshot_json_v1
publish_json_v1
```

Runtime startup and scene-launch step results are mirrored into the shared loading snapshot, so diagnostics and tools can inspect the same state the native shell is showing.

## Platform compositor

`platform-winit` now owns a dedicated loading compositor object. It starts when the native window is created, receives overlay snapshots from host step results, and uses its own visual clock to animate spinner, pulse, progress and subsystem cards even when startup work is busy.

On Windows, the compositor thread draws a lightweight native GDI fallback directly from the latest snapshot. The existing richer `RedrawRequested` loading surface remains available when the event loop is responsive.

## Invariant

```text
Loading UX is a domain service.
Native presentation is a platform concern.
Startup work must not own the visual clock.
```
