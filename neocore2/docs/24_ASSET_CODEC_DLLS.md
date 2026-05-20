# 24 — AssetManager codec DLLs

## Verdict

AssetManager is now a codec host, not a format owner.

```text
AssetManager
  -> VFS bytes
  -> codec registry
  -> codec DLL service
  -> decoded payload / nested container source
```

The first-party runtime codecs live outside the AssetManager crate:

```text
Plugins/AssetManager/codecs/newengine-codec-firstparty
```

This codec worker registers:

```text
asset.codec.nepak   .nepak  containerType
asset.codec.neytd   .neytd  listType
asset.codec.nemat   .nemat  singleType
asset.codec.ydd     .ydd    listType
```

## Codec types

```text
containerType
  binary Magic + container index
  may expose nested VFS entries
  may recursively contain other assets
  example: .nepak

listType
  binary Magic + multiple same-domain records
  cannot expose nested VFS entries
  examples: .neytd, .ydd

singleType
  binary Magic + one object
  cannot expose nested VFS entries
  example: .nemat

plainText
  UTF-8/text policy without Magic
  cannot expose nested VFS entries
```

Only `containerType` can return nested VFS layers. `listType`, `singleType` and `plainText` descriptors are rejected if they request nesting.

## Runtime startup order

```text
1. AssetManager creates an empty AssetStore.
2. AssetManager loads codec DLLs from plugins/codecs.
3. Codec DLLs register their services and descriptors.
4. Container VFS layers are mounted through registered containerType codecs.
5. AssetManager registers engine.assets service.
```

This matters because `.nepak` is no longer a built-in VFS source. It is just bytes until `asset.codec.nepak` is loaded.

## Build output

Runtime plugin sync installs codec workers into:

```text
NewEngine/neocore2/plugins/codecs/
```

AssetManager default config points `codecs_dir` at `codecs`, relative to the runtime plugin directory.

## Hard rule

New file format support must be added as a codec worker or codec provider descriptor. AssetManager should not be modified for `.foo`, `.bar`, source package formats, drawable dictionaries, material containers or texture dictionaries.
