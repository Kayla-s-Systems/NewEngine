# Architecture hotfix — 2026-05-09

Goal: stop renderer/plugin failures from killing the whole engine, reduce launcher duplication, and move plugin policy toward a declarative layout.

## Changes

### Shared launcher crate

Added `crates/newengine-app-launcher`.

`apps/editor` and `apps/game-ready-fps` are now thin wrappers. The duplicated bootstrap logic was moved into one place:

- run id/env setup;
- crash reporter install;
- `config.json` loading;
- log file sharding;
- bootstrap plugins;
- asset roots;
- platform runtime detection;
- UI markup loading;
- platform runtime handoff.

### Render fallback hardening

`ResolvedRenderBackendConfig` now contains `headless: bool` and `is_headless()`.

When `newengine.renderer.vulkan` is unavailable or rejected, the runtime still installs the null backend, but the editor render controller now treats it as an explicit degraded state and skips GPU frame work instead of driving viewport render passes through a missing renderer.

### Null renderer no-op completeness

`NullRenderApi` now allocates no-op texture and sampler handles. This makes the null backend safe for degraded-mode cleanup/probing code paths.

### Plugin discovery diagnostics

Renderer-looking DLL names are now classified as legacy renderer candidates instead of anonymous unknown dynlibs. This makes broken/stale renderer deployments visible as renderer ABI problems rather than generic discovery noise.

### Declarative plugin policy

Added `plugins/plugins.manifest.json` and `plugins/README.md` to document required runtime plugins, optional renderer fallback, and importer layering.

## Remaining renderer requirement

`vulkan_renderer-*.dll` still must be rebuilt as a V3 plugin exposing:

- `export_plugin_root`
- descriptor id: `newengine.renderer.vulkan`
- capability/service: `render.api.v1`

A renderer DLL without the plugin root cannot be used by the current runtime bridge.
