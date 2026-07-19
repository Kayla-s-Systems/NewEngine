# North Star Asset Inspector

Standalone light-theme GUI for viewing and changing assets through engine-owned providers.

## Architecture

`asset-inspector.exe` is a thin facade. It does not parse formats or open asset files directly.

- `RuntimeHost` discovers and mounts application asset roots.
- `engine.assets` owns VFS access and provider routing.
- `engine.assets.inspect` returns the selected `AssetDocument`.
- `engine.assets.edit` validates and applies provider-owned `AssetPatch` values.
- `engine.schema` supplies field descriptors, source pointers and transaction semantics.
- `engine.ui` renders the authored NEUI surface.

The product UI contains generic VFS rows, provider preview metadata, schema fields, diagnostics and provider-declared actions. Adding support for a new asset format should require a new engine format provider, not a change in this application.

## Editing policy

A field is editable only when all of the following are true:

1. The provider marks the field editable.
2. The provider exposes a canonical source pointer or schema JSON pointer.
3. The document exposes a concrete writer capability.
4. `engine.assets.edit.validate_patch_json_v1` accepts the generated patch.

The application does not guess extensions, counterpart paths, serializers or repack commands.

## UI assets

- Source surface: `gameAssets/ui/src/tools/asset_inspector.neui.xml`
- Source theme: `gameAssets/ui/src/themes/asset_inspector.neui.xml`
- Source components: `gameAssets/ui/src/components/asset_inspector.neui.xml`
- Runtime surface: `gameAssets/ui/tools/asset_inspector.neui`

## Run

```text
apps\AssetInspector\run_asset_inspector.cmd
```

or:

```text
cargo run -p asset-inspector
```
