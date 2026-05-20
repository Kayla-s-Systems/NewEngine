# 23 — AssetManager codec registry

## Verdict

AssetManager is a pure VFS/bytes/dispatch host.

```text
AssetManager
  -> reads bytes from VFS/source
  -> asks registered codec descriptors who owns the bytes
  -> validates broad codec rules
  -> calls the selected codec provider
  -> returns codec-defined output through proxy APIs
```

It must not contain concrete knowledge of runtime file formats. New formats are added by codec providers, not by editing AssetManager.

## Codec loading

AssetManager loads codec DLLs from `codecs_dir`.

It does not load importer DLLs. A codec DLL registers an `asset_codec` service descriptor with:

```text
extensions
codec_type
magic / text policy
outputs
method
priority
format/container metadata
```

The codec descriptor is published to `engine.assets.file_types` when the codec is ready. The file-type registry stores descriptors and supports browsing/probing. It does not parse files.

## Codec types

```text
containerType
  May expose nested VFS entries.
  May recursively host other assets.
  Example: .nepak.

listType
  One file contains multiple records of one domain.
  Requires magic bytes.
  Cannot expose nested VFS.
  Examples: .neytd texture dictionaries, .ydd drawable dictionaries.

singleType
  One binary file -> one decoded object.
  Requires magic bytes.
  Cannot expose nested VFS.
  Example: .nemat.

plainText
  Textual file without binary magic.
  Identified by extension/content policy.
  Cannot expose nested VFS.
  Example: future .bindings.json codec.
```

## Container rule

Only `containerType` codecs may return `container.vfs_layer` or nested VFS sources.

Every other codec type is rejected if it declares nested outputs. This prevents `.neytd`, `.ydd`, `.nemat` or plain text assets from silently becoming asset containers.

## NEPAK rule

`.nepak` is a codec-owned container format, not a built-in AssetManager source type.

```text
container layer request
  -> generic VFS source kind: container
  -> read bytes
  -> codec registry selects containerType codec by extension and magic
  -> NEPAK codec creates nested VFS source
```

The AssetManager source registry only knows the generic `container` source kind. It does not know NEPAK layout.

## No hardcoded format table

Bad shape:

```text
AssetManager knows .neytd/.ydd/.nemat/.nepak implementations
```

Correct shape:

```text
codec provider knows format
AssetManager stores descriptor and calls decode
engine.assets.file_types stores navigation/probe descriptor
```

## Strictness

- No `.pak` compatibility layer.
- No importer loader.
- No source-format fallback.
- No non-container nesting.
- Binary codecs require magic bytes unless declared `plainText`.

## First-party codec worker projects

The first-party runtime codecs are real AssetManager-private codec DLL projects under:

```text
Plugins/AssetManager/codecs/newengine-codec-nepak
Plugins/AssetManager/codecs/newengine-codec-neytd
Plugins/AssetManager/codecs/newengine-codec-nemat
Plugins/AssetManager/codecs/newengine-codec-ydd
```

Shared ABI/helper code lives in:

```text
Plugins/AssetManager/newengine-codec-api
Plugins/AssetManager/codecs/newengine-codec-common
```

There is no monolithic `newengine-codec-firstparty` worker. Each format owns its parser and descriptor. The build manifest installs codec workers into:

```text
NewEngine/neocore2/plugins/codecs
```

Expected startup evidence after codec worker sync:

```text
assets: codec loaded file='newengine-codec-nepak-...dll'
assets: codec loaded file='newengine-codec-neytd-...dll'
assets: codec loaded file='newengine-codec-nemat-...dll'
assets: codec loaded file='newengine-codec-ydd-...dll'
assets: codec discovered id='asset.codec.neytd' ... exts=["neytd"]
```

If the world starts untextured and `asset.decode_v1` reports `no registered codec accepted path='*.neytd'`, the `.neytd` codec worker is not installed or was rejected by descriptor validation.
