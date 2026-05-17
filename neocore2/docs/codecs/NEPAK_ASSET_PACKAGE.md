# 13 — `.nepak` codec spec — NewEngine Asset Package

## Purpose

`.nepak` is NewEngine's proprietary deterministic asset archive format for VFS layers.

It packages loose assets into a single archive with payload blobs, a binary index and a root `index.json` virtual asset manifest. The AssetManager mounts it as a VFS source.

## Runtime contract

```text
logical asset path
  -> VFS layer resolution
  -> .nepak source
  -> binary index lookup
  -> payload read/decompress/verify
  -> bytes returned to AssetManager pipeline
```

The engine addresses assets by logical path. The archive decides where the payload lives physically inside the package.

## Container identity

```text
canonical extension: .nepak
accepted compatibility extension/type: .pak / pak
magic: NEPAK\x01\x00\x00
footer magic: NEPAKEND
tool: tools/NePak
AssetManager source kind: nepak
```

The current reader accepts `pak` and `nepak` layer types. Documentation and new content should prefer `.nepak` for proprietary NewEngine packages.

## Root `index.json`

Every strict package contains a root `index.json` entry:

```json
{
  "version": "1.0",
  "assets": [
    {
      "name": "textures/fps/world_surfaces.neytd",
      "path": "textures/fps/world_surfaces.neytd",
      "type": "texture_dictionary",
      "hash": "blake3:<hex>"
    }
  ]
}
```

`name` is the virtual path visible to the engine. `path` is the physical archive path. Hashes validate raw uncompressed payload bytes.

## Binary layout

```text
header
payload blobs
binary index
footer
```

Header:

```text
u8[8] magic = "NEPAK\x01\x00\x00"
```

Binary index:

```text
u8[8]  magic = "NEPAK\x01\x00\x00"
u32    entry_count
entry[] sorted by normalized archive path:
  u16 path_len
  u8[path_len] path UTF-8 with / separators
  u64 payload_offset
  u64 payload_len
  u64 raw_len
  u8  payload_kind       # 0=raw, 1=zstd
  u8[32] raw_hash        # blake3 of uncompressed payload
```

Footer:

```text
u8[8] magic = "NEPAKEND"
u64   index_offset
u64   index_len
u32   index_hash         # blake3(index_bytes), truncated to u32
u32   reserved
```

## Payload kinds

```text
0 = raw bytes
1 = zstd-compressed bytes
```

On read, compressed payloads are decompressed, raw length is checked and BLAKE3 hash is verified.

## Determinism

- paths normalize to `/`;
- entries sort lexicographically by path bytes;
- `index.json` is generated from sorted assets;
- payload hash is BLAKE3 of raw bytes;
- duplicate virtual names are invalid.

## VFS usage

Example layer:

```json
{ "type": "nepak", "mount": "core", "priority": 200, "path": "./Paks/core.nepak" }
```

The VFS resolves conflicts by priority, mount order and stable tie-breakers.

## Tooling

`tools/NePak` is the package builder/inspector/extractor/verifier. It should remain an authoring tool; runtime reads `.nepak` through AssetManager only.
