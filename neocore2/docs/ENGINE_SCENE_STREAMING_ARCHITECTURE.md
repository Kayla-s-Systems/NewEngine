# Engine Scene Streaming Architecture

## Purpose

`engine.scene` owns world residency policy. Rendering is only one consumer of the scene; it must not decide which parts of a large world exist.

The scene layer separates two independent questions:

```text
simulation residency: what must keep ticking
render residency:     what must have GPU-visible resources
```

A cell can be simulated without being rendered. A rendered cell must have simulation/world data, but it does not force every far-away simulation cell to keep textures, meshes or draw-list resources alive.

## Runtime shape

```text
observer/focus position
  -> scene streaming profile
  -> layered streaming plan
      render layer
      simulation layer
  -> CPU generation / asset requests
  -> explicit GPU residency pump
  -> draw extraction references ready handles only
```

## Layer policy

### Render residency

Render residency is small, view-oriented and memory aggressive.

```text
visible / near-visible cells
  -> CPU scene entities may exist
  -> GPU mesh/texture residency requested with a per-frame budget
  -> draw extraction skips not-ready chunks
  -> cells outside render unload radius are removed from render residency
```

Render extraction must not create large meshes or upload cold GPU buffers. It only records draw packets for already resident GPU handles.

### Simulation residency

Simulation residency can be wider than render residency, but should be cheaper.

```text
near gameplay cells        -> full simulation
far active cells           -> coarse / LOD simulation
inactive far cells         -> serialized state only
render-invisible sim cells -> no GPU resources
```

This is required for large worlds: traffic, NPC schedules, missions, weather, economy and world events can continue without paying render memory cost for invisible districts.

## Terrain streaming in GameReady

The current GameReady terrain now follows this contract:

```text
streaming job builds terrain descriptor + primitive mesh
  -> ECS chunk commit inserts PreparedTerrainPrimitiveMesh
  -> render residency pump uploads at most N terrain chunks/frame
  -> draw extraction draws only cached GPU terrain meshes
  -> not-ready chunks are skipped until their GPU buffers are resident
```

Upload budget is controlled by:

```text
NEWENGINE_TERRAIN_GPU_UPLOADS_PER_FRAME=0..8
```

Default is `1`, which prevents a camera move from uploading many terrain chunks inside the same draw-list extraction frame.

## Invariants

- `engine.scene` decides cell residency.
- Render does not decide world existence.
- Simulation does not imply GPU residency.
- Render extraction never performs cold terrain mesh conversion or cold terrain GPU upload.
- Not-ready render cells are skipped or keep previous resident visuals; they do not block the frame.
- Far invisible cells are not kept as render resources.
- Large-world simulation should use separate full/coarse/sleeping states.

## Future domains

```text
engine.scene.streaming
  plan/snapshot/update policy

engine.scene.simulation_cells
  full/coarse/sleeping serialized world cells

engine.render.residency
  GPU resource upload/eviction budgets

engine.assets.streaming
  async asset IO and package/dependency residency
```
