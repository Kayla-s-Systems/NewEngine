# Data-driven Material / PAK / listFiles P0 — 2026-05-21

## Decisions

- `.ytd` is the only authored texture dictionary reference. `.neytd` is not accepted in new authored/runtime graphs.
- `.nemat` is the native NewEngine material descriptor/library boundary. Materials resolve through `engine.materials`.
- `.pak` is the canonical package extension. The old `NEPAK` term is retained only as internal wire magic in the current binary container implementation.
- `listFiles` is standardized as `AssetFileManifest` / `AssetEntryManifest` in `newengine-assets-api`.

## Material flow

```text
AuthoredMaterialDescriptor (.nemat)
  -> engine.materials / materials.api
  -> ResolvedMaterialGraph
  -> RenderMaterialPacket
  -> renderer
```

Forbidden runtime pattern:

```text
create_material()
attach_material_to_model()
renderer parses material/texture paths
```

Required pattern:

```text
resolve drawable
  -> resolve material slots
  -> resolve .nemat@material entry through engine.materials
  -> validate .ytd@entry texture refs
  -> produce RenderMaterialPacket
```

## listFiles standard

Every dictionary/container codec should support `asset.list_file_manifest_v1` and return:

```text
AssetFileManifest {
  source,
  file_kind,
  container,
  codec,
  entries: [AssetEntryManifest],
  dependencies,
  warnings,
  policy
}
```

This is the common surface for AssetManager, AssetBrowser, importers, validators, asset graph resolver and tools.

## PAK policy

- `.pak` mounts as a transparent VFS directory.
- Runtime code must not know whether an asset is loose or packaged.
- Package priority is VFS layer priority.
- Nested packages require explicit `AssetProfile` policy: `allow_nested_paks`, `max_nested_depth`, and cycle detection.
