# Procedural Noise Architecture

`newengine-procedural-noise` is the deterministic foundation crate for generated terrain, grayscale masks, and texture-like noise data.

## Contract

- No renderer dependency.
- No plugin-host dependency.
- No filesystem dependency.
- No global RNG or time dependency.
- All generation is `descriptor/settings -> deterministic CPU data`.

## Layers

```text
NoiseAlgorithm
  raw deterministic sources: value, billow, ridged, cellular, cellular edges, marble, lightning, veins

NoiseLayer2D
  one declarative source layer with frequency/amplitude/bias/shape/combine

NoiseGraph2D
  domain + optional domain warp + ordered layers + output remap

NoiseTexture2D
  generated R8 grayscale pixels from a graph/preset

TerrainHeightfieldDescriptor
  generated heightfield from a NoiseGraph2D
```

## Presets

Current texture presets:

```text
ElectricVeins  -> high contrast cellular/branch lightning texture
MarbleEnergy   -> warped marble bands with edge glow
SoftCells      -> soft cellular blobs/cloud-cell texture
```

These are intentionally descriptors, not hardcoded assets. Renderer/importer layers may upload the generated `NoiseTexture2D::pixels` as R8/linear textures or use the same graph to build heightfields.

## Standalone FPS demo

`game-ready-fps` depends on `newengine-game-ready-profile`, not directly on the editor profile. The selected profile composes reusable runtime modules, installs the GameReady render feature pack, and delegates engine-owned scene/ECS/entity gateway services to dedicated runtime crates.
