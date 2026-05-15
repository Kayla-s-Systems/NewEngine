# NEYTD Binary Texture Dictionary V2

`NEYTD` is the only runtime texture container for NewEngine material/UI textures.
Source image formats such as PNG, JPG, DDS, TGA and WebP are offline authoring inputs only.
They are not runtime texture assets and must not be referenced by materials.

## Runtime contract

A runtime texture reference is always one of:

```text
textures/fps/world_surfaces.neytd@terrain_forest_floor
textures/fps/world_surfaces.neytd@hash:16390287886862091167
```

The renderer asks AssetManager for one selected dictionary entry through:

```text
asset.texture_dictionary_rgba8_v1
payload: { dictionary_path, texture_name | texture_hash }
```

AssetManager resolves the `.neytd` through VFS, caches the dictionary bytes, selects the entry and returns a runtime `NTRT` RGBA8 texture packet.
The material runtime does not call image importers and does not decode source-image containers.

## File layout

```text
NETD fixed header, 64 bytes
NTDX binary directory
16-byte aligned raw RGBA8 mip payload region
```

There is no JSON in the `.neytd` file. The old JSON directory, `schema`, `source_path`, `payload_compression` and authoring provenance fields are removed.

## Header

```text
0x00  u8[4]  magic = "NETD"
0x04  u16    version = 2
0x06  u16    flags = 0
0x08  u32    header_len = 64
0x0C  u32    entry_count
0x10  u64    directory_offset
0x18  u64    directory_len
0x20  u64    data_offset
0x28  u64    data_len
0x30  u64    data_uncompressed_len = 0
0x38  u8[8]  reserved
```

## Binary directory

```text
NTDX directory header
fixed-size entry records
fixed-size mip records
name table
```

Entry records store name offsets, name hash, extent, format enum, color-space enum, mip range and data range.
Mip records store level, extent and byte range relative to the raw payload region.

## Design constraints

1. No runtime compatibility with legacy image textures.
2. No JSON directory inside `.neytd`.
3. No compression in V2 runtime payload.
4. No source-image paths or authoring provenance in runtime containers.
5. All material texture resolution is centralized through AssetManager texture dictionary lookup.
