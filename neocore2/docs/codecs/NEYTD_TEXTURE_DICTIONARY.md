# 12 — `.neytd` codec spec — NewEngine Texture Dictionary

## Purpose

`.neytd` is NewEngine's proprietary runtime texture dictionary container.

It is the runtime texture format for materials/UI. Source authoring formats such as PNG/JPG/TGA/WebP/DDS are importer/tool inputs, not material runtime references.

## Runtime path syntax

```text
textures/fps/world_surfaces.neytd@terrain_forest_floor
textures/fps/world_surfaces.neytd@hash:16390287886862091167
```

The part before `@` is the VFS logical path to the dictionary. The part after `@` selects an entry by normalized texture name or stable hash.

## Runtime contract

```text
Material/UI texture reference
  -> AssetManager texture_dictionary_* service
  -> VFS resolves .neytd bytes
  -> texture dictionary parser selects one entry
  -> runtime texture packet returned to renderer/UI
```

Runtime code does not decode source image containers.

## Container identity

```text
extension: .neytd
magic:     NETD
version:   2
crate:     newengine-texture-container
tool:      tools/netexturetool
```

## Layout

```text
HeaderV2, 64 bytes
NTDX binary directory
aligned mip payload region
```

Header fields:

```text
0x00  u8[4]  magic = "NETD"
0x04  u16    version = 2
0x06  u16    flags
0x08  u32    header_len = 64
0x0C  u32    entry_count
0x10  u64    directory_offset
0x18  u64    directory_len
0x20  u64    data_offset
0x28  u64    data_len
0x30  u64    data_uncompressed_len
0x38  u8[8]  reserved
```

Directory identity:

```text
magic: NTDX
version: 1
entry record length: 64
mip record length: 32
```

The directory stores entry names, stable name hashes, pixel format ids, color-space ids, extents, mip ranges and payload byte ranges.

## Supported runtime formats

```text
RGBA8_UNORM
RGBA8_SRGB
BC1_RGBA_UNORM
BC1_RGBA_SRGB
BC3_RGBA_UNORM
BC3_RGBA_SRGB
BC5_RG_UNORM
BC7_RGBA_UNORM
BC7_RGBA_SRGB
```

BCn entries store runtime-ready compressed block mip chains. RGBA8 remains supported for UI/editor/backward-compatible cases.

## Determinism and strictness

- no JSON directory in V2 runtime container;
- no source image provenance in runtime payload;
- no material references to PNG/JPG/TGA/etc;
- normalized names and stable `u64` hashes;
- full mip chains are stored per entry;
- payload ranges are validated before use.

## AssetManager methods

```text
texture_dictionary_rgba8_v1
  returns selected entry decoded/normalized as RGBA8 runtime packet

texture_dictionary_runtime_v1
  returns selected GPU-native runtime texture packet when supported
```

## Authoring/tooling

`tools/netexturetool` is the intended authoring UI. Importers may transform source textures into `.neytd`; runtime loads only `.neytd` dictionaries.
