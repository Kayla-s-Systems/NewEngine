# Prefab material training slice

## Goal

Add a stable prefab/content slice to the standalone game so we can iterate on materials without depending on the editor pipeline.

## Added asset

- `assets/prefabs/tree_animate/scene.gltf`
- `assets/prefabs/tree_animate/scene.bin`
- `assets/prefabs/tree_animate/textures/*`
- `assets/prefabs/tree_animate/tree_animate.prefab.json`

## Runtime strategy

Current engine/runtime does **not** yet have a full imported multi-mesh GLTF render path for standalone gameplay.
So this pass uses a deliberate transitional architecture:

1. Keep the **real prefab source asset** in `assets/prefabs/tree_animate`.
2. Keep a **declarative prefab manifest** with material slots and texture references.
3. Spawn a **training proxy composition** in `game-ready-fps`:
   - bark trunk
   - branch elements
   - foliage canopy
4. Add nearby **material preview swatches** for quick iteration.

This lets us test the engine, the standalone game path, material descriptors and asset organization immediately.

## Next pass

1. Add true static-mesh / GLTF runtime rendering.
2. Add texture-bound material descriptors.
3. Replace the proxy composition with the real imported prefab while preserving the same prefab manifest + material-slot contract.

## Current standalone placement contract

The tree proxy is driven from `apps/game-ready-fps/assets/game_ready_highlands.scene.json`:

- `prefabs[]` declares the prefab id, source manifest and proxy strategy.
- `foliage` owns deterministic placement seed, grid, jitter, count and scale range.
- `materials.tree_bark`, `materials.tree_leaf` and `materials.tree_branch` bind the staged texture paths.

Object placement remains in the scene/runtime layer. The renderer only receives the resulting ECS render components through the normal draw-list providers.
