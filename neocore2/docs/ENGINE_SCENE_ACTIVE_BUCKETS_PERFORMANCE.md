# Engine Scene Active Buckets Performance Pass

## Problem

The previous terrain residency pass removed cold terrain GPU uploads from draw
extraction, but the runtime still treated every render-resident cell/entity as
active for every frame. A small scene could therefore spend tens of milliseconds
in CPU feature extraction even when the backend submit path was mostly idle.

The log shape that triggered this pass was:

```text
render feature profile: total_ms ~= 32 ms
  gameready.terrain ~= 20 ms
  gameready.primitive_mesh ~= 12 ms
```

That means the next bottleneck is no longer chunk generation. It is active scene
selection: render extraction is doing too much work for cells/entities that are
resident but not currently important to the camera.

## Direction copied as architecture, not code

The reference scene runtime separates active scene lists into scored buckets:
visible/important entities are requested and processed first, low-value invisible
entities are delayed, and simulation residency is not the same thing as render
residency.

NewEngine now mirrors that model with clean-room vocabulary:

```text
SceneStreamingBucket
  ActiveSimulation
  VisibleNear
  VisibleFar
  PredictedNear
  SimulationOnly
  InvisibleFar
  Sleeping
```

These buckets are scene policy. They are not Vulkan or renderer state.

## Runtime changes

Render extraction now receives camera forward direction in addition to camera
position. GameReady draw lists use this to apply a conservative active-bucket
cull before material binding, UBO writes and draw command generation.

```text
resident world cell
  -> active render bucket test
  -> only visible/predicted cells enter draw extraction
  -> invisible simulation cells keep world state but do not write GPU commands
```

Terrain and primitive mesh extraction now perform cheap sphere/cone/distance
checks before hot-path work. This is intentionally wider than the true camera
frustum to avoid visible popping.

## Tunables

```text
NEWENGINE_RENDER_SCENE_CULLING=0|1
NEWENGINE_TERRAIN_RENDER_DISTANCE=150
NEWENGINE_PRIMITIVE_RENDER_DISTANCE=92
NEWENGINE_PRIMITIVE_SHADOW_DISTANCE=120
NEWENGINE_RENDER_FORWARD_CONE_DOT=-0.25
NEWENGINE_TERRAIN_NEAR_ACCEPT_DISTANCE=<derived>
NEWENGINE_PRIMITIVE_NEAR_ACCEPT_DISTANCE=12
NEWENGINE_RUNTIME_OPAQUE_PRIMITIVE_BUDGET=96
NEWENGINE_RUNTIME_SHADOW_PRIMITIVE_BUDGET=64
NEWENGINE_SHADOW_REFRESH_FRAMES=180
```

## Invariants

- Render culling must not despawn or stop simulation.
- Simulation residency can be wider than render residency.
- GPU upload residency remains outside draw extraction.
- Draw extraction is allowed to skip not-active resident cells.
- Plugin/backend render ownership remains behind `engine.render`.

## Next required pass

The next structural step is a persistent `engine.scene.streaming` snapshot and a
first-class active-entity list. The active list should be built once per frame by
scene runtime, then render, physics broadphase, AI perception and editor debug
views should consume the same scored snapshot instead of each domain rescanning
the whole ECS world.
