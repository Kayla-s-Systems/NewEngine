# Tree Animate prefab

Purpose: a training prefab for material authoring.

## Source content

- `scene.gltf`
- `scene.bin`
- `textures/*`

## Material slots

- **Bark** -> trunk/bark study
- **Leaf** -> foliage alpha-test / double-sided study
- **Branch** -> thin branch masked study

## Current engine state

The original GLTF is stored in the asset tree now.

The current standalone FPS demo does not yet render multi-mesh imported GLTF assets directly,
so the demo scene uses a **training proxy prefab composition** built from engine primitives while
preserving the original prefab metadata and material slots.

That gives us a safe place to iterate on materials now, without blocking on the full mesh import path.
