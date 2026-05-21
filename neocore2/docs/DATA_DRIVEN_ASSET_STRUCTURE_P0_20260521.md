# P0 Data-driven Asset Structure — YTYP/YDD/YTD/PAK

## Fixed authored chain

```text
.ytyp@archetype -> .ydd@drawable -> .nemat@material -> .ytd@texture
```

`.pak` is the delivery/VFS package container. `.neytd` is legacy/cache-only and is not valid for new authored references.

## Asset reference syntax

All runtime/content references use:

```text
<logical-path>[@entry]
```

Examples:

```text
player/abigail/textures/abigail.ytd@head_diff_000_a_uni
player/abigail/models/abigail.ydd@head
world/interiors/safehouse.ytyp@bedroom_archetype
player/abigail/materials/abigail_skin.nemat@head
```

Rules:

- References are VFS logical paths, never physical paths.
- `@entry` is the universal dictionary selector.
- Material/runtime graphs must not reference raw PNG/JPG/TGA/DDS files.
- New authored references must not use `.neytd`.
- No hidden `.ytd -> .neytd` fallback. Legacy readers require explicit project/VFS policy.
- Existing NETD texture dictionaries are migrated to public `.ytd` filenames; `.neytd` files are not authored references.

## Material boundary

`engine.materials` is the required material resolve gateway:

```text
AuthoredMaterialDescriptor -> ResolvedMaterialGraph -> RenderMaterialPacket
```

Renderer-owned material state is not a content contract. The renderer receives `RenderMaterialPacket` only.

## YDD material slots

YDD drawable entries carry declarative slot refs:

```json
{
  "name": "head",
  "material_slots": [
    {
      "slot": "skin_head",
      "material": "player/abigail/materials/abigail_skin.nemat@head"
    }
  ]
}
```

The resolver path is:

```text
resolve drawable -> resolve material slots -> resolve texture refs -> produce render packet
```
