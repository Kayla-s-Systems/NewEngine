# 22 — Asset codec registry, materials and drawable dictionary boundary

## Verdict

NewEngine now treats AssetManager as a pure VFS/bytes/dispatch host.

```text
engine.assets
  -> AssetManager VFS/read bytes
  -> codec registry lookup by longest extension + magic bytes
  -> selected codec provider decode(bytes, request)
  -> codec-defined runtime packet / manifest / raw payload
```

The engine no longer keeps a manually authored table of every runtime file type.
The file-type registry starts empty and is populated only by codec/provider
self-registration.

## Core rule

```text
AssetManager must not know how a format works.
A codec provider knows the format and registers its descriptor when it is ready.
```

AssetManager responsibilities:

- resolve logical paths through VFS and `.nepak`/filesystem layers;
- read raw bytes;
- find the best registered codec by extension and magic bytes;
- call `codec.decode(bytes, request)`;
- return codec output through `asset.decode_v1` or typed proxy methods.

Codec responsibilities:

- declare extension(s), magic bytes, asset kind, outputs and priority;
- parse/validate its own container;
- return runtime-ready DTO/wire packets;
- keep all format-specific knowledge out of AssetManager core.

## Current first-party codec descriptors

These are registered by the AssetManager plugin at startup as first-party codecs.
They are descriptors/providers, not hardcoded registry rows.

| Extension | Codec | Magic | Outputs | Runtime policy |
|---|---|---|---|---|
| `.neytd` | `asset.codec.neytd` | `NETD` | `texture.runtime`, `texture.rgba8` | runtime texture dictionary codec |
| `.nemat` | `asset.codec.nemat` | `NEMAT\0\0\0` | `material.raw` | native material payload boundary; material domain decodes semantics |
| `.ydd` | `asset.codec.ydd` | `RSC7` | `drawable.manifest_json`, `drawable.raw` | drawable dictionary boundary; mesh/material extraction is codec-owned |
| `.ytyp` | `asset.codec.ytyp` | `definitionType`; extension plus codec-owned XML/binary/deflate source policy | `model.definition_entries_json`, `ytyp.raw_source`, `ytyp.raw_payload` | object Definition Entries / archetype metadata; bounds, LOD, drawable/texture dictionary declarations |

There is no `.neydd` runtime type yet. Drawable dictionary is a single `.ydd` codec
boundary until the native drawable dictionary container is formalized.

## File-type registry

```text
engine.assets.file_types
  -> descriptor registry only
  -> asset.file_types.register_json_v1
  -> asset.file_types.manifest_json_v1
  -> asset.file_types.probe_json_v1
```

It does not parse files, decode payloads or contain built-in knowledge. It only
stores descriptors that providers publish.

## Material gateway

```text
engine.materials
  -> materials.api
  -> engine.assets / asset.decode_v1(output=material.raw)
  -> newengine-materials binary decode
  -> MaterialDescriptor / MaterialId / texture refs
```

The material gateway consumes the codec-dispatched payload. It does not bypass
AssetManager/VFS with raw filesystem reads.

## Drawable / definition gateway

```text
engine.model
  -> model.drawable_dictionary_manifest_json_v1
  -> engine.assets / asset.decode_v1(output=drawable.manifest_json)
  -> DrawableDictionaryManifest

engine.model
  -> model.definition_entries_json_v1
  -> engine.assets / asset.decode_v1(output=model.definition_entries_json)
  -> DefinitionEntriesManifest
```

The current `.ydd` codec validates the RSC7 boundary and returns a strict manifest
packet. The `.ytyp` codec reads Definition Entries from XML today and from a reserved native binary/deflate envelope later, which lets
object metadata, bounds, LOD distances and dictionary links come from assets
instead of hardcoded spawn tables.

## Invariants

- No manually registered built-in file-type table in AssetManager.
- No `.ydd`/`.neydd` dual runtime model.
- No hidden fallback from `.ydd` to another extension.
- No direct format parsing in renderer/runtime host.
- New formats arrive by codec/provider registration, not by editing AssetManager.
