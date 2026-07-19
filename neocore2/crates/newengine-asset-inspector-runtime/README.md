# newengine-asset-inspector-runtime

Thin product facade for the North Star Asset Inspector.

## Ownership boundary

The crate owns only:

- VFS navigation and selection state;
- projection of provider DTOs into retained UI state;
- schema-driven input conversion;
- dispatch of provider-owned document actions.

The crate does **not** own asset formats, parsers, writers, source/runtime pairing rules or filesystem paths.

## Engine contracts

- `engine.assets` provides VFS listings and provider manifests.
- `engine.assets.inspect` returns `AssetDocument` with preview, schema sections, diagnostics and actions.
- `engine.assets.edit` validates and applies `AssetPatch` values.
- `engine.schema` owns property semantics, validation and undo/redo transaction DTOs.

Editable fields are applied only when the provider exposes a concrete writer and a canonical source pointer. The facade never invents a JSON pointer or branches on a file extension.

The authored light UI is stored in `gameAssets/ui/src/tools/asset_inspector.neui.xml` and rendered by the active `engine.ui` provider.
