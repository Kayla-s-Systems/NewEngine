# Asset Document Entry Directory Model

Asset files may be browsed as document containers when their provider can return `AssetFileManifest`.

```text
path/file.ytd
  @albedo
  @normal
  @roughness

path/file.ytyp
  @archetype_a
  @archetype_b
```

The UI asks for `asset.list_file_manifest` through `engine.assets` and receives addressable entries. It never parses the file body directly.

The global Right Edit Window consumes `EditorSelectionContext`:

```text
asset      -> engine.assets.inspect -> AssetDocument
asset_entry -> engine.assets.inspect -> AssetDocument for file@entry
```

Save/apply is a separate path:

```text
AssetPatch
  -> engine.assets.edit
  -> format/package writer capability
  -> AssetPatchResult diagnostics
```

This keeps Content Browser as a generic UI composition and keeps file semantics in format/domain providers.
