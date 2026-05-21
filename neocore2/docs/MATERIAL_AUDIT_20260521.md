# Material Audit — 2026-05-21

## Current material representations to converge

- `newengine_materials::MaterialDescriptor` — renderer-agnostic authored/runtime descriptor.
- `newengine_materials::MaterialTextureBindings` — texture slot refs; now restricted to `.ytd@entry`.
- `newengine_material_runtime::MaterialAssetGatewayAdapter` — `engine.materials` gateway resolver.
- `newengine_model_domain_api::DrawableMaterialSlotRef` — drawable slot to material descriptor reference.
- GameReady scene material defaults — transitional content bootstrap; must move behind construction plans.
- Renderer-private pipeline/material handles — backend implementation only; not authored content.

## Forbidden runtime pattern

```text
create_material()
attach_material_to_model()
```

## Required pattern

```text
resolve drawable -> resolve material slots -> resolve texture refs -> produce render packet
```

## P0 code changes in this pass

- Added shared `AssetReference` parser in `newengine-assets-api`.
- Restricted material texture refs to `.ytd@entry`.
- Added `materials.manifest_json_v1`, `materials.load_descriptor_v1`, `materials.resolve_graph_v1`, `materials.validate_v1`, `materials.to_render_packet_v1`.
- Added `RenderMaterialPacket` and `ResolvedMaterialGraph` contracts.
- Added declarative `DrawableMaterialSlotRef` to YDD manifest DTOs.
- Removed `.neytd` from model runtime texture dictionary acceptance.
- Switched GameReady/material/UI declarative references to `.ytd@entry` and added `.ytd` copies of existing NETD payloads for migration continuity.

## Remaining follow-up

The opaque RSC7 `.ydd/.ytd` parsers still validate container boundaries but do not enumerate native directories. The DTOs and gateway contracts are now ready for strict entry validation once the RSC7 directory readers are connected.
