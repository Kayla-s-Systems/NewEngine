# North Star Asset Inspector

Standalone GUI application for `gameAssets`.

## Architecture

- Dedicated executable: `asset-inspector.exe`.
- Dedicated startup configuration: `apps/AssetInspector/config.json`.
- Authored Aurelia UI: `gameAssets/ui/src/tools/asset_inspector.neui.xml`.
- Runtime UI asset: `gameAssets/ui/tools/asset_inspector.neui`.
- Runtime assets are read and decoded only through `engine.assets` and registered native codecs.
- Editable `source/` assets are read through the same VFS, then inspected by native Rust parsers.
- No GameReady scene, gameplay, physics or editor-shell embedding.

## Supported inspection paths

Runtime ListFiles: `.ytd`, `.ydd`, `.ymap`, `.ytyp`, `.nemat`, `.neui`, `.neftd`, `.neitems`
and the other registered NEF8 descriptors. The tool requests provider-owned `domain.manifest_json`
and ListFile manifests instead of parsing runtime files in the GUI.

Source: authored XML, JSON, DDS, OBJ/MTL, glTF/GLB, PNG/BMP/JPEG, TTF, SPIR-V, FBX,
shader text and binary buffers.

## Run

```text
apps\AssetInspector\run_asset_inspector.cmd
```

or:

```text
cargo run -p asset-inspector
```
