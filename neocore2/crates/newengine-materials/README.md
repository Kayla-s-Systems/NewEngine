# newengine-materials

Deterministic, renderer-agnostic material contracts plus runtime registry support for NewEngine.

## Design goals

- **Determinism**: stable ids, stable iteration order, stable binary encoding.
- **Layering**: public contracts in `src/api`, runtime implementation in `src/core`.
- **Growth-ready**: builtins are registration helpers; plugins can register more materials later.
- **Cache safety**: the binary format is little-endian, versioned, and intentionally conservative.

## Quick start

```rust
use newengine_materials::MaterialRegistry;

let reg = MaterialRegistry::with_builtins();
let list = reg.snapshot();
```

## Public API

- `MaterialId`, `MaterialRef`
- `MaterialDescriptor`, `MaterialFlags`
- `MaterialRegistryApi`, `MaterialProvider`
- `binary::{MaterialBinaryAsset, encode_asset, decode_asset}`

## Binary format

The crate provides a deterministic binary container for materials (recommended extension: `.nemat`).

Header layout:

- magic: 8 bytes (`NEMAT\0\0\0`)
- version: `u16`
- header size: `u16`
- payload size: `u32`
- flags: `u32`
- reserved: `u32`

Payload layout:

- name length: `u16`
- reserved: `u16`
- UTF-8 name bytes
- zero padding to 4-byte alignment
- fixed-size `MaterialDescriptor` payload (`68` bytes in v1)

## Notes

- Texture bindings and GPU handles are intentionally not part of the base descriptor yet.
- Emission uses `emissive` multiplied by `emissive_strength`, which maps cleanly to HDR bloom workflows.
- The codec now rejects overlong names instead of silently truncating them.
