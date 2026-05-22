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

## `.nemat` material library contract

`.nemat` is a NEF8/ListFile material library with multiple addressable entries:

```text
materials/world/garage.nemat@garage_door
```

Runtime material loading resolves one selected library entry through `engine.materials`, validates `.ytd@entry` texture refs, then emits a renderer-agnostic `RenderMaterialPacket`.

The old single-material binary helpers remain low-level descriptor payload utilities only; they are not the public `.nemat` file contract.
