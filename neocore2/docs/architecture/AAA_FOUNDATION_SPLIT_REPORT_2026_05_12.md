# AAA Foundation Split Report — 2026-05-12

## Fixed in this pass

- Restored `newengine-engine-runtime` compile shape after the render/scene split.
- Removed dangling attributes left by mechanical file slicing.
- Moved `game_ready::content` declaration back to the real module root.
- Corrected `CameraRigComp` vs `CameraRig` split-boundary regression.
- Made draw-list visibility copy-safe and reusable.
- Split remaining `newengine-engine-runtime` files above 400 lines.

## newengine-engine-runtime large-file status

After this pass, no Rust source file under `crates/newengine-engine-runtime/src` is above 400 lines.

## Workspace files still above 400 lines

| lines | file |
|---:|---|
| 764 | `crates/newengine-editor-runtime/src/ui/extension_abi.rs` |
| 710 | `crates/newengine-render-api/src/render_graph.rs` |
| 668 | `crates/newengine-core/src/startup/loader.rs` |
| 621 | `crates/newengine-gizmo/src/egui/draw_rotate.rs` |
| 604 | `crates/newengine-editor-runtime/src/plugin_manager/ui.rs` |
| 594 | `crates/newengine-core/src/console/runtime.rs` |
| 590 | `crates/newengine-editor-runtime/src/ui/panels/viewport.rs` |
| 584 | `crates/newengine-ui/src/markup/egui_render.rs` |
| 578 | `crates/newengine-runtime-host/src/platform_runtime/runtime_host.rs` |
| 576 | `crates/newengine-plugin-host/src/plugin_config_service.rs` |
| 571 | `crates/newengine-procedural-noise/src/graph.rs` |
| 559 | `crates/newengine-camera/src/editor.rs` |
| 553 | `crates/newengine-plugin-host/src/manager/loader.rs` |
| 551 | `crates/newengine-editor-runtime/src/ui/providers.rs` |
| 545 | `crates/newengine-core/src/engine/module_boot.rs` |
| 541 | `crates/newengine-previews/src/primitive.rs` |
| 527 | `crates/newengine-editor-runtime/src/ui/schema/mod.rs` |
| 526 | `crates/newengine-math/src/quat.rs` |
| 517 | `crates/newengine-plugin-host/src/host_context.rs` |
| 468 | `crates/newengine-gizmo/src/egui/controller.rs` |
| 467 | `crates/newengine-ecs/src/world.rs` |
| 467 | `crates/newengine-core/src/jobs.rs` |
| 452 | `crates/newengine-core/src/crash.rs` |
| 440 | `crates/newengine-editor-runtime/src/ui/state.rs` |
| 434 | `crates/newengine-runtime-host/src/render_runtime/service_api.rs` |
| 431 | `crates/newengine-procedural-noise/src/heightfield.rs` |
| 425 | `crates/newengine-runtime-host/src/platform_runtime/config.rs` |

These remaining files are outside the current game-ready/runtime render hot path and should be split by subsystem-owned passes instead of mechanical slicing.

## Next recommended split passes

1. `newengine-runtime-host/platform_runtime/runtime_host.rs` → bootstrap overlay, event loop, shutdown coordinator.
2. `newengine-render-api/render_graph.rs` → graph model, compiler, validation, submit report DTOs.
3. `newengine-core/startup/loader.rs` → config source, plugin phase selection, startup graph reporting.
4. `newengine-plugin-host/*` → config service, loader plan, ABI probe, capability validation.
5. Editor/UI/gizmo crates → contracts first, adapter rendering second.

## Compile stabilization addendum

Follow-up build diagnostics exposed three split-boundary mistakes in `game_ready::content`:

- `paths` / `profile` were declared from an included `content_parts/*` file, so Rust resolved them relative to `content_parts/` instead of the real `content/` directory. They are now declared explicitly in `content.rs` using `#[path = ...]`.
- `fallback_game_ready_map_profile` was split across two files, leaving `profile_parse.rs` with an unclosed function header. The complete function now lives in `sanitize_defaults.rs`.
- `raw_payload.rs` is now a pure payload DTO file; it no longer owns child module wiring.

This keeps the split declarative: module wiring stays in the module root, DTOs stay in raw payload files, parsing stays in profile parsing, and default/sanitize policy stays in defaults.

## Compile stabilization addendum 2

Follow-up build diagnostics exposed two final split-boundary regressions:

- `RawShadowSpec` kept field-level `#[serde(default = ...)]` attributes but lost its `#[derive(Debug, Deserialize)]` item derive during the content split. The derive is restored, so `RawLightingSpec` can deserialize `shadows` declaratively again.
- `render_controller/resource_cache.rs` used `RenderTargetId` after cache/lifetime code was extracted from the controller, but the import was not carried with the extracted file. The type is now imported from `newengine_core::render` next to the other render handle DTOs.

Policy note: split files must preserve complete Rust items. Field attributes, derives, imports, and module declarations belong to the same ownership boundary as the DTO/function they describe. Future split passes should move whole item clusters rather than slicing by line ranges.

## Compile stabilization addendum 3

Follow-up build diagnostics exposed two pass-split integration issues in `module_impl::passes`:

- `mesh_visibility.rs` was previously `include!`-expanded directly into `passes.rs`, so its imports shared the same namespace as `mesh_passes.rs` and caused duplicate `Mat4` / `Vec3` definitions. It now lives inside an explicit `mesh_visibility` helper module.
- Primitive visibility sorting now uses a small `DistanceKeyEntry` trait instead of a hard-coded `(f32, u64, T)` tuple shape. This keeps the sort reusable for both compact tuples and the richer primitive entries used by the mesh pass.

Policy note: helper files that own imports should be normal submodules, not direct textual includes into another file's namespace. Direct `include!` is only safe for fragments that intentionally share the parent module's imports and item namespace.
