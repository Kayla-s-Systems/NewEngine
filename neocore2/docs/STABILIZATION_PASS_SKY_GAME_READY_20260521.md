# Stabilization pass — GameReady sky / terrain / material model

## Intent

This pass removes the most visible local divergence introduced by the live sky work:

- sky lifecycle no longer lives inside the terrain/material god file;
- sky visual spawning is driven by a single `SKY_VISUAL_SPAWN_ORDER` registry;
- material registration is driven by a canonical role-definition table;
- foliage material slot matching uses a rule table instead of ad-hoc `if/else`;
- launch-gate optional texture matching uses a central tag list;
- terrain streaming owns its own chunk/residency/generation code path.

## Canonical ownership after pass

```text
game_ready.rs
  shared GameReady runtime prelude/imports
  include order / module assembly only

game_ready_parts/sky.rs
  SkyDomeRuntime
  SkyVisualKind / SkyVisualRuntime
  SkyAtmosphereRuntime
  SkyCycleRuntime
  sky visual registry
  time-of-day sampling
  ambient/sun/sky visual application

game_ready_parts/materials_terrain.rs
  demo material roles
  demo material definition table
  common primitive spawn helper

game_ready_parts/terrain_streaming.rs
  TerrainSurfaceLayers
  PreparedTerrainPrimitiveMesh
  terrain chunk generation
  terrain streaming state and tick

game_ready_parts/assets_bootstrap.rs
  asset mesh decode/import
  skydome primitive loading
  scene assembly
```

## Removed divergence

Old shape:

```text
SkyDomeRuntime/SkyVisualRuntime/SkyAtmosphereRuntime lived in materials_terrain.rs
spawn_skydome manually spawned dome, sun and moon through separate paths
MoonDisk call site duplicated the kind argument risk
material ids were registered through repeated inline calls
foliage slot binding used local string conditionals
readiness optional textures used local hard-coded boolean expression
```

New shape:

```text
Sky runtime model is centralized in sky.rs
sky entities spawn from SKY_VISUAL_SPAWN_ORDER
sky kind owns name, primitive selection, initial radius/color and follow-camera policy
materials register from DemoMaterialDefinition rows
foliage slot role resolves from FOLIAGE_SLOT_RULES
launch readiness resolves optional texture tags from LAUNCH_OPTIONAL_TEXTURE_TAGS
```

## Build note

The patch was prepared in an environment without `cargo`, `rustc` or `rustfmt`; run the normal project build and formatter after applying it.
