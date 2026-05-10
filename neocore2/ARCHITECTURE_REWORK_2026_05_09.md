# Architecture Rework — 2026-05-09

Goal: move NewEngine away from god-files and hidden runtime assumptions toward small declarative contracts that can support both the editor and a playable vertical slice.

## Applied changes

### 1. Scene bridge split

`newengine-editor-runtime/src/scene_bridge.rs` was split into:

- `scene_bridge/mod.rs` — public bridge facade and command application.
- `scene_bridge/commands.rs` — command DTOs only.
- `scene_bridge/imported_assets.rs` — imported-asset descriptors and assembly policy.
- `scene_bridge/helpers.rs` — material/bounds/root helpers.
- `scene_bridge/game_ready.rs` — hardcoded FPS demo scene bootstrap.
- `scene_bridge/queue.rs` — command queue container.

The bridge now has a clearer boundary: public facade + declarative command DTOs + scene construction policies.

### 2. UI schema split

`ui/schema.rs` was split into:

- `ui/schema/types.rs` — schema DTOs/enums/type aliases.
- `ui/schema/mod.rs` — schema builders/providers and extension merging.

This removes the mixed DTO + behavior god-file pattern.

### 3. Plugin manager no longer stores duplicate loaded containers

`PluginManager` no longer stores both:

- `loaded: Vec<LoadedPlugin>`
- `loaded_ids: HashSet<String>`

`loaded_ids` was a stale-state risk. Loaded IDs are now derived from the authoritative `loaded` vector only when discovery needs a selection set.

### 4. Plugin config diff extracted

Config diffing moved out of the loader into:

- `manager/config_diff.rs`

`manager/loader.rs` now focuses on loading, ABI selection and init orchestration.

### 5. Declarative plugin manifest integrated into discovery

`plugins/plugins.manifest.json` is now read by the plugin host. The manifest is used to:

- report missing required plugins;
- classify known DLL names when ABI metadata is missing;
- make discovery errors explicit instead of silent `unknown dynlib` drift.

ABI descriptors remain the source of truth. The manifest is an ops/deployment contract and a diagnostic fallback, not a replacement for ABI.

## Runtime policy

- Platform runtime is still loaded explicitly by platform runtime discovery.
- Bootstrap plugins load first.
- Engine plugins load after the platform window is ready.
- Renderer DLLs must be V3 runtime plugins that provide `render.api.v1`.
- Missing renderer must degrade to `newengine.renderer.null` without crashing the app.

## Next pass

Recommended next split targets:

1. `render_controller/gpu.rs` — split resource allocation, swapchain resources, frame submission, and error policy.
2. `plugin_manager/ui.rs` — split row projection, action buttons, diagnostics view, and asset/icon rendering.
3. `gameplay.rs` — split components, player control, collision step, runtime snapshots, FPS demo rules.
4. `newengine-render-api/src/lib.rs` — split ABI DTOs by surface/frame/resources/debug overlays.
