# Procedural terrain rework — 2026-05-09

## Goal

Move the Game Ready FPS demo away from a hand-authored cube arena and introduce a reusable procedural generation foundation:

```text
newengine-procedural-noise
  -> deterministic parameterized noise
  -> heightfield generation
  -> CPU mesh bake
  -> collision proxy bake
  -> renderer/editor integration through ECS component
```

## New crate

`crates/newengine-procedural-noise` is intentionally foundation-level:

- no renderer dependency;
- no plugin-host dependency;
- no filesystem dependency;
- no global RNG/time;
- deterministic output from explicit settings.

Public contracts:

- `FractalNoise2D`
- `ValueNoise2D`
- `NoiseAlgorithm`
- `TerrainHeightfieldSettings`
- `HeightField`
- `TerrainCollisionTileSettings`
- `TerrainCollisionTile`
- `ProceduralTerrain`

## Runtime/demo integration

`game-ready-fps` still boots through `newengine-editor-runtime`, but the scene bootstrap now creates:

- one `ProceduralTerrain` heightfield entity;
- terrain mesh rendered through the editor render controller;
- coarse AABB collision tiles generated from the heightfield;
- deterministic procedural obstacles;
- pickups/hazards/goal placed on the generated terrain height;
- a skydome primitive and staged OBJ asset under `assets/skydome/`.

## Renderer integration

`EditorRenderController` now has a `terrain_cache: FxHashMap<u64, PrimitiveGpu>` keyed by `ProceduralTerrain::mesh_key()`.

The GPU upload path was refactored:

- `upload_primitive_mesh(...)` uploads any `PrimitiveMesh`;
- built-in primitives still use `ensure_primitive_gpu(...)`;
- procedural terrain reuses the same vertex/index layout and lit pipeline.

## Collision policy

Current gameplay physics is AABB-based. The procedural terrain therefore bakes coarse collision tiles from heightfield ranges:

```text
heightfield -> N coarse AABB tiles -> existing CollisionBody/Bounds flow
```

This is not a final AAA heightfield collider. It is the correct incremental architecture for the current physics layer because it avoids adding a parallel collision path before the physics backend contract is ready.

## Next pass

Recommended follow-up:

1. Add first-class `HeightfieldCollider` to the physics API.
2. Move terrain rendering into a dedicated terrain render pass once render graph lands.
3. Add material layers/splat maps generated from slope and altitude.
4. Cook generated terrain into asset-cache entries for streaming.
5. Add skydome OBJ import/cook once static mesh runtime rendering is promoted beyond proxy primitives.
