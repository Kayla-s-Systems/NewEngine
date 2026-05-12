# AAA Foundation Refactor — 2026-05-12

## Goal

Reduce fragile god files in the runtime/render/game-ready path and turn the first playable frame into an explicit pipeline:

1. native loading surface,
2. scene launch gate,
3. resource residency,
4. first stable Vulkan frame,
5. gameplay/player possession.

## Render controller split

`render_controller/module_impl/mod.rs` is now only the module declaration/root. Runtime responsibilities are split by contract:

- `lifecycle.rs` — module start/shutdown.
- `render_entry.rs` — frame orchestration and final publication to resources.
- `prelaunch_gate.rs` — native loading / no-present scene gate.
- `playable_viewport.rs` — viewport-level branch and failure handling.
- `world_tick.rs` — camera, play-mode transition, player possession and physics tick.
- `scene_submit.rs` — extraction, shadow plan, draw-list providers and render graph submission.
- `frame_types.rs` — DTOs between render phases.
- `frame_submit.rs` — graph submit and draw-list recording helpers.
- `shadow_cache.rs` — explicit shadow refresh policy.

## Draw-list and pass split

The largest low-level render files are physically separated into stable include parts to avoid risky semantic movement while still making review diffs small:

- `draw_lists_parts/registry.rs`
- `draw_lists_parts/plugin_bridge.rs`
- `draw_lists_parts/extraction.rs`
- `passes_parts/mesh_passes.rs`
- `passes_parts/debug_passes.rs`
- `light_extraction_parts/registry.rs`
- `light_extraction_parts/plugin_bridge.rs`

## Game-ready scene split

`scene_bridge/game_ready.rs` and `scene_bridge/game_ready/content.rs` are split into explicit responsibility groups:

- materials and terrain assembly,
- foliage placement,
- imported asset / skydome / bootstrap,
- raw profile payload,
- profile parsing,
- sanitizers and defaults.

## Stability rules preserved

- No scene present before scene launch gate release.
- Player control remains locked until `GameReadyWorldLaunchGate` marks `play_activated`.
- Render graph submission stays isolated from native loading.
- Shadow map rendering is separated from shadow texture reuse.
- Runtime `SceneLaunchStatus` remains the platform-visible loading contract.

## Remaining large files outside this pass

This pass deliberately focuses the playable runtime path. Remaining large files in editor/plugin/core layers should be split in later passes by domain, not mechanically:

- logging plugin configuration and module orchestration,
- input plugin module,
- Vulkan resource/graph executor internals,
- editor UI panels and schema,
- core startup loader/module boot,
- runtime-host platform integration.

Preferred next split order:

1. Vulkan renderer resource lifetime and graph executor.
2. Runtime-host platform bootstrap / close / loading overlay.
3. Core startup loader and module boot.
4. AssetManager service/plugin modules.
5. Editor UI panels/providers.
