# Asset codecs overview

NewEngine runtime-owned proprietary asset containers:

```text
.neytd  NewEngine Texture Dictionary
.nepak  NewEngine Asset Package
.ytyp   Rockstar CMapTypes / NewEngine Definition Entries metadata
.ydd    Rockstar drawable dictionary boundary
```

`.neytd` is selected through AssetManager texture dictionary methods and returns runtime-ready texture packets.

`.nepak` is mounted as a VFS source and returns verified raw asset bytes to AssetManager.

`.ytyp` is decoded by `asset.codec.ytyp` into Definition Entries so object metadata, LOD, bounds and dictionary references come from assets instead of scene hardcode.

`.ydd` is decoded by `asset.codec.ydd` as a drawable dictionary boundary; full native `.neydd` can later connect to the same model-domain contracts.

Source formats such as PNG/JPG/TGA/FBX/GLTF remain authoring/import inputs; runtime profile assets should reference NewEngine runtime containers when possible.

See:

- `docs/codecs/NEYTD_TEXTURE_DICTIONARY.md`
- `docs/codecs/NEPAK_ASSET_PACKAGE.md`
- `docs/codecs/YTYP_DEFINITION_ENTRIES.md`
