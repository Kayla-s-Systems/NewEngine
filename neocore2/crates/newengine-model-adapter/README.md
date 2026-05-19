# newengine-model-adapter

AssetManager-backed model adapter for runtime avatar/model assets.

The crate owns the neutral asset hierarchy:

```text
model request / optional manifest
  -> model logical path
  -> mesh payload
  -> skeleton payload
  -> material library / material manifest
  -> .neytd dictionary texture entries
  -> collision descriptors
  -> ModelAssetBundle
```

Runtime/game code should ask this adapter for a resolved bundle instead of hand-assembling OBJ/MTL/YMT/texture paths.

The adapter remains backend-neutral: it returns primitive meshes, material descriptors, skeleton metadata and collision refs. It does not own ECS entities, renderer handles or physics backend handles.
