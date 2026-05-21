# Asset codecs overview

NewEngine runtime-owned proprietary asset containers:

```text
.neytd  NewEngine runtime texture dictionary
.nepak  NewEngine Asset Package
.ytyp   Rockstar CMapTypes / NewEngine Definition Entries metadata
.ydd    Rockstar drawable dictionary boundary
.ytd    Rockstar texture dictionary source/domain role
```

`.neytd` is selected through AssetManager texture dictionary methods and returns runtime-ready texture packets.

`.nepak` is mounted as a VFS source and returns verified raw asset bytes to AssetManager.

`.ytyp` is decoded by `asset.codec.ytyp` into Definition Entries so object metadata, LOD, bounds and dictionary references come from assets instead of scene hardcode. Its canonical chain is `.ytyp -> .ydd -> .ytd`; `.neytd` remains the NewEngine runtime texture packet container used after import/compilation.

`.ytd` is decoded by `asset.codec.ytd` as the source/domain texture dictionary boundary referenced by `.ytyp`. It is not runtime-ready; import/compile it to `.neytd` for GPU packets.

`.ydd` is decoded by `asset.codec.ydd` as the only drawable dictionary container in the model chain.

Source formats such as PNG/JPG/TGA/FBX/GLTF remain authoring/import inputs; runtime profile assets should reference NewEngine runtime containers when possible.

See:

- `docs/codecs/NEYTD_TEXTURE_DICTIONARY.md`
- `docs/codecs/NEPAK_ASSET_PACKAGE.md`
- `docs/codecs/YTYP_DEFINITION_ENTRIES.md`
