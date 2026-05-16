# Plugin-owned content architecture

NewEngine keeps the host thin: executable DLL plugins provide capabilities, while
`plugins/plugins.manifest.json` publishes declarative content payloads owned by
those plugins.

## Policy

- Engine bootstrap must not hard-code game maps as the source of truth.
- Scene/map/prefab/generator data is read from `plugins.manifest.json.content`.
- Runtime code may keep a small fallback profile only to keep dev builds bootable
  when no plugin catalog is present.
- DLL descriptors declare `scene.contribution.v1` through
  `CapabilityKind::SceneContributionV1`.
- Content payloads reference their `provider_plugin` and required capability.

## Thin-engine boundary

The current boundary is deliberately narrow:

- `newengine-plugin-host` reads deployment manifests and exposes a typed `PluginContentCatalog`; it does not know Game Ready FPS internals.
- `newengine-plugin-api` only adds the generic ABI capability kind `SceneContributionV1`; it does not define one-off FPS contracts.
- `newengine-editor-runtime` contains a small adapter from plugin JSON payloads to ECS entities.
- `GameReadyMapPlugin` owns the scene contribution identity and descriptor; the runtime manifest owns the map, terrain generator parameters, prefab anchors, marker layout and palette.

Generator constants such as ridge/vein frequency, amplitudes, seed xor values and smoothing edges are no longer embedded in the scene spawner. They are data in the plugin content payload and can be replaced without changing engine code.

## Current Game Ready FPS flow

1. `newengine-plugin-host::load_plugin_content_catalog_from_dir` reads
   `plugins.manifest.json`.
2. `newengine-editor-runtime::scene_bridge::game_ready::content` selects
   `newengine.scene.game_ready.highlands.v1`.
3. The scene bridge adapts that payload into strongly typed local specs.
4. Terrain, skydome, gameplay markers, prefabs and foliage are spawned from the
   specs, not from scattered literals.

This is intentionally a bridge layer: the plugin manifest owns the content, the
engine owns only the minimal adapter from declared schema to ECS components.

## 2026-05-16 — Physics backend service adapter boundary

Physics now follows the provider/adapter direction used by render. Runtime systems must use `PhysicsApiRef` and `newengine-physics-api` packets; backend plugins provide `physics.api` plus `physics.backend`. The deterministic, Jolt and null physics providers live under `Plugins/` and are selected as service backends. Engine/runtime code must not depend on Jolt or `newengine-physics-runtime` directly.

Target frame boundary:

```text
ECS -> PhysicsFrameInput -> physics.api -> PhysicsFrameOutput -> ECS
```
