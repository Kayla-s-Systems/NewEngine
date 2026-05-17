# Asset codecs overview

NewEngine runtime-owned proprietary asset containers:

```text
.neytd  NewEngine Texture Dictionary
.nepak  NewEngine Asset Package
```

`.neytd` is selected through AssetManager texture dictionary methods and returns runtime-ready texture packets.

`.nepak` is mounted as a VFS source and returns verified raw asset bytes to AssetManager.

Source formats such as PNG/JPG/TGA/FBX/GLTF remain authoring/import inputs; runtime profile assets should reference NewEngine runtime containers when possible.

See:

- `docs/codecs/NEYTD_TEXTURE_DICTIONARY.md`
- `docs/codecs/NEPAK_ASSET_PACKAGE.md`
