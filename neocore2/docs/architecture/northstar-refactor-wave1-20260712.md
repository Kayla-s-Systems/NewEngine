# NorthStar Engine Refactor — Wave 1

Date: 2026-07-12
Scope: `NewEngine/neocore2` with a repository-wide audit of `NorthStar` and `PluginsSrc`.

## Executive result

This wave removed an inactive render pass, fixed a per-frame GPU resource churn path, split two engine monoliths, and centralized repeated hashing/log/path infrastructure.

The largest measured hot path was `gameready.primitive_mesh`:

| Metric | Baseline sampled mean | Post-refactor warm sample | Change |
|---|---:|---:|---:|
| `gameready.primitive_mesh` | 19.38 ms | 1.80 ms | -90.7% indicative |
| Feature extraction | 22.49 ms | 2.04 ms | -90.9% indicative |
| Render CPU frame | 29.26 ms | 16.95 ms | -42.1% indicative |

The post-refactor result is a single steady-state trace sample because normal fast frames no longer cross the warning threshold. It is strong evidence of the removed allocation/churn path, but not a statistically complete benchmark. A repeatable benchmark harness remains a Wave 2 item.

## Applied changes

### 1. Pass routing is now explicit

`SceneExtractionCtx` now carries the active deferred/forward policy.

- Forward runtime no longer constructs GBuffer draw lists.
- Deferred runtime routes opaque roles to GBuffer.
- Sky, transparent, weather and first-person view-model roles remain in forward.
- Opaque geometry is not rendered twice when deferred is enabled.

Primary files:

- `crates/newengine-render-feature-api/src/lib.rs`
- `crates/newengine-render-feature-gameready/src/lib.rs`
- `crates/newengine-engine-runtime/src/render_controller/module_impl/frame_orchestrator.rs`
- `crates/newengine-engine-runtime/src/render_controller/module_impl/draw_lists_parts/extraction.rs`

### 2. Primitive UBO/bind-group churn removed

The old instanced shadow UBO key included the current light-view matrix and mesh ID. A moving sun therefore generated new persistent UBO and bind-group entries across frames even though the matrix is instance data.

The new policy:

- keys shared UBOs by pipeline + texture set + sampler;
- keeps mesh identity only in the instance batch key;
- keeps light-view/model transforms in instance data;
- writes each shared UBO once per pass/frame;
- retires stale per-draw UBO and bind-group resources through renderer frame-completion events.

Observed sharing:

- Forward: 30 material/mesh plans -> 23 shared UBOs.
- Shadow: 14 plans -> 7 shared UBOs.

Primary files:

- `mesh_passes_primitive.rs`
- `mesh_passes_shadow.rs`
- `instancing.rs`
- `resource_cache.rs`
- `resource_lifetime.rs`

### 3. Render monolith split

Before:

- `mesh_passes.rs`: 1001 lines, terrain + primitive forward/GBuffer planning.

After:

- `mesh_passes.rs`: 364 lines, terrain pass and façade.
- `mesh_passes_primitive.rs`: 630 lines, primitive pass planning/batching.
- `mesh_passes_shadow.rs`: 404 lines, shadow-only extraction.
- `scene_mesh_pass.rs`: shared pass labels and diagnostic cadence.

The split preserves the public pass surface while separating terrain, primitive and shadow responsibilities.

### 4. Inventory monolith split

Before:

- `gameplay/inventory.rs`: 1047 lines.

After:

- `inventory.rs`: 96-line façade and stable re-export surface.
- `definitions.rs`: definitions and item-domain DTOs.
- `catalog.rs`: item registration and lookup.
- `storage.rs`: inventory state and mutations.
- `loadouts.rs`: loadout domain.
- `operations.rs`: world-facing inventory operations.
- existing equipment/world/tests remain isolated modules.

All existing public import paths remain available through the façade.

### 5. Hashing unified

New canonical engine functions in `newengine-math/src/hash.rs`:

- `hash_combine_u64`
- `avalanche_u64`
- `fnv1a_64`

Local copies were removed from:

- procedural noise graph/heightfield;
- render primitive batching;
- combat deterministic queries;
- inventory IDs/world pickups;
- authored item asset IDs.

Reference-vector tests protect persistent deterministic behavior.

### 6. Logging/path formatting unified

Repeated `log_fmt.rs` and `path_fmt.rs` implementations in `newengine-core` and `newengine-plugin-host` now delegate to canonical modules in `newengine-ulog-api`:

- `newengine_ulog_api::formatting`
- `newengine_ulog_api::path_format`

This removes two independent implementations of boxed diagnostics, tables, ellipsizing, canonicalization and stable path display.

### 7. Runtime root discovery unified

Duplicate `find_neocore2_root()` implementations in launcher and early platform logging were replaced by:

- `newengine-runtime-host/src/path_resolver.rs`

The resolver has a focused test for case-insensitive `neocore2` matching.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo check -p game-ready-fps`
- `cargo build -p game-ready-fps`
- `cargo test -p newengine-math --lib` — 4 passed
- `cargo test -p newengine-procedural-noise --lib` — 7 passed
- `cargo test -p newengine-engine-runtime inventory --lib` — 14 passed
- `cargo test -p newengine-runtime-host path_resolver --lib` — 1 passed
- `cargo test -p newengine-render-feature-gameready --lib`

Final runtime smoke:

- launch gate: 36/36 static objects, 26/26 GPU meshes, 50/50 textures;
- public Play activated;
- shadow caster path active;
- native sky cycle active;
- no ERROR/FATAL records;
- no device loss;
- no frame-graph submit failure;
- no shader or material-texture failure.

Logs:

- `cache/logs/archive/refactor-wave1-final-smoke-20260712.log`
- `cache/logs/archive/refactor-wave1-ubo-dedup-benchmark-20260712.log`

## Repository observations

`NorthStar` is a suite of repositories rather than one Git worktree:

- `NewEngine` is one Git repository.
- Plugin directories under `PluginsSrc` are independent repositories.
- Eight plugin repositories contain byte-identical `plugin_cdylib_build.rs` copies; Winit has one small platform-specific variation.

A direct external `include!` would reduce duplication but break standalone plugin clones. Wave 2 should introduce either a versioned build-support crate or a generated-copy synchronizer with hash verification.

## Remaining priority work

### P0 — render throughput

1. Build a frame-local primitive scene snapshot once and reuse it for shadow and forward/GBuffer extraction.
2. Cache owned resolved material plans across frames with explicit registry generation invalidation.
3. Profile the remaining `submit` cost; the post-refactor trace showed 9.95 ms in backend submission.
4. Add a deterministic benchmark mode that emits a sample every N frames without warning-threshold bias.

### P1 — remaining monoliths

- `scene_bridge/game_ready_parts/sky.rs` — 1048 lines.
- `newengine-world-environment-api/src/lib.rs` — 1351 lines.
- `newengine-render-api/src/protocol.rs` — 1327 lines.
- `newengine-assets/src/asset_document_service.rs` — 1303 lines.
- plugin-side Flecs/AssetManager/Vulkan files above 1000 lines.

### P1 — protocol/code duplication

Unify the repeated binary readers/writers in:

- `newengine-ui-draw/src/binary.rs`
- `newengine-ui-api/src/frame_binary.rs`
- `newengine-render-api/src/protocol.rs`

A small bounded binary-codec crate should own checked offsets, endian reads, length limits and error context.

## Safety and rollback

The `NewEngine/neocore2` worktree already contained extensive unrelated modifications before this wave. No commit or broad reset was performed. Targeted `.bak-20260712-*` backups were created beside changed files for local rollback.
