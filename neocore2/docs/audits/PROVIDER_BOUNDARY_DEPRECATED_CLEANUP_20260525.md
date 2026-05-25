# Provider Boundary Deprecated Cleanup — 2026-05-25

## Rule

```text
Engine owns the question.
Provider owns the answer.
Capability declares availability.
Profile/registry decides the active route.
Runtime degrades cleanly if route/capability is absent.
```

Reusable engine runtime may depend on API domains and gateway ids. It must not depend on concrete provider/backend implementation names, concrete plugin crates or backend-specific branching.

## Removed in this patch

| File | Action | Reason |
|---|---|---|
| `NewEngine/neocore2/crates/newengine-loading-api/src/pre_aurelia_ui.rs` | delete | Provider-specific naming leaked into a shared pre-runtime UI contract. Replaced by neutral `bootstrap_ui`. |

## Fixed in this patch

| Area | Action |
|---|---|
| UI input DTO | `UiInputFrame` moved to `newengine-ui-api`; `newengine-ui` now re-exports it for compatibility. Reusable runtime no longer imports UI implementation DTOs. |
| UI draw DTO | `newengine-engine-runtime` and `newengine-runtime-host` use `newengine-ui-api::UiDrawList`. |
| Route checks | runtime/platform paths use `newengine_core::has_engine_gateway_route` rather than `newengine_plugin_host::has_service`. |
| Optional gateway calls | GameReady scene hydration calls to `engine.assets.definitions`, `engine.assets.graph`, `engine.assets.materials`, and `engine.time` now use optional gateway calls and degrade cleanly when the route is absent. |
| Generic UI content | generic XML/theme snippets no longer name Aurelia. |
| CI | Added `scripts/provider_boundary_scan.py` and wired it into CI. |

## Deprecated / cleanup candidates

These are not all deleted in this patch because some require staged extraction, but they should be treated as cleanup targets.

| File / group | Recommended action | Reason |
|---|---|---|
| `NewEngine/neocore2/crates/newengine-engine-runtime/src/scene_bridge/game_ready.rs` | move out of reusable engine runtime into `newengine-game-ready-profile` or a profile plugin crate | Product/profile-specific runtime should not live in generic engine orchestration. |
| `NewEngine/neocore2/crates/newengine-engine-runtime/src/scene_bridge/game_ready/**` | move with `game_ready.rs` | Same ownership issue: profile authored content hydration is not reusable engine core. |
| `NewEngine/neocore2/crates/newengine-engine-runtime/src/scene_bridge/game_ready_parts/**` | move with `game_ready.rs` | Same ownership issue; profile-specific material/sky/foliage/player code belongs behind a profile/domain provider. |
| `NewEngine/neocore2/assets/loading/loading_ui.ytd` | inspect and delete if superseded by `assets/loading/loaderWindow.ytd` | Potential duplicated loading asset dictionary. Keep only one authoritative loader UI dictionary. |
| `Plugins/build_all_plugins.cmd` legacy deletion block for `eguiUiProvider-*` | remove after migration window | Legacy provider alias cleanup should be finite, not permanent runtime/build policy. |
| `Plugins/build_manifest.json` provider names | normalize to descriptor-driven discovery if consumed by tools | Static plugin-name manifests must not become active-route policy. |

## Audit guardrail

Run:

```text
python scripts/provider_boundary_scan.py
```

The scanner fails if reusable runtime crates reintroduce direct provider/backend references or direct route presence checks through plugin-host internals.
