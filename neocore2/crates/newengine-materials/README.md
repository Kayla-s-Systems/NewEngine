# newengine-materials

Deterministic, renderer-agnostic material contracts + runtime registry for NewEngine.

## Design goals

- **Determinism**: stable ids and stable iteration order.
- **Layering**: public contracts in `src/api`, runtime implementation in `src/core`.
- **Growth-ready**: builtins are just registration helpers; plugins can register new materials.

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

## Notes

Textures and GPU bindings are intentionally not part of the base descriptor yet.
They should be introduced once the asset pipeline exposes a stable texture handle contract.

## Binary format

The crate provides a deterministic binary container for materials (recommended extension: `.nemat`).

- `newengine_materials::binary::{MaterialBinaryAsset, encode_asset, decode_asset}`
- Emission uses `emissive` (color) * `emissive_strength` (scalar), which supports HDR-style bloom workflows.
