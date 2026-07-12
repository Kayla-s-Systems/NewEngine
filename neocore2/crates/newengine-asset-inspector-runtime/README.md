# newengine-asset-inspector-runtime

Product runtime for the North Star Asset Inspector.

The crate owns inspection orchestration and authored-UI state only. It never opens runtime asset
files directly. Runtime bytes, VFS listings and native codec output flow through `engine.assets`.
Editable `source/` files are also read through AssetManager, then passed to provider-neutral native
parsers such as `newengine-authored-xml`, `newengine-texture-container` and
`newengine-model-import-obj`.

The GUI is authored in `gameAssets/ui/src/tools/asset_inspector.neui.xml` and rendered by the active
`engine.ui` provider.
