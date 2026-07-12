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
materials/world_garage.nemat@garage_door
```

Runtime material loading resolves one selected library entry through `engine.assets.materials`, validates `.ytd@entry` texture refs, then emits a renderer-agnostic `RenderMaterialPacket`.

Texture tiling is authored per `.nemat` entry rather than hard-coded in scene/runtime code:

```xml
<Params>
  <Param name="uv_scale" type="float2" value="128,128" />
  <Param name="uv_offset" type="float2" value="0,0" />
</Params>
```

`uv_scale` multiplies the mesh UV channel. Textured opaque materials use repeat sampling, so values above `1` tile the selected `.ytd@entry`. Surface culling is likewise asset-owned through `Surface two_sided="true|false"`.

The old single-material binary helpers remain low-level descriptor payload utilities only; they are not the public `.nemat` file contract.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-materials`

**Role:** Material libraries, material sources, or material runtime assets.

**Local contents:** 1 direct subdirectories, 2 direct files.

**Direct file examples:** `Cargo.toml`

## Working rules

- Do not put transient build output in this directory unless the directory is explicitly a runtime output/cache location.
- Keep runtime assets and editable source assets separate: source assets are packed into runtime formats through explicit tools/manifests.
- Do not introduce hidden provider/backend coupling here; use declared descriptors, gateways, DTOs, and explicit maintenance scripts.

<!-- NORTHSTAR-DIR-README:END -->
