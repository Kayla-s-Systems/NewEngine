# YTYP Definition Entries codec

## Purpose

`.ytyp` is the data-driven object metadata source for model/domain construction.
It imports Rockstar-style `CMapTypes` XML as NewEngine Definition Entries today, and keeps the same runtime contract for a future native binary or deflate-compressed `.ytyp` envelope.

The engine should not hardcode object bounds, LOD distances, texture dictionary
names, drawable dictionary names or asset type rules in scene/bootstrap code.
Those facts belong to asset metadata and are read through `engine.assets`.

## Runtime path

```text
.ytyp logical path
  -> engine.assets / asset.decode_v1(output=model.definition_entries_json)
  -> asset.codec.ytyp
  -> DefinitionEntriesManifest
  -> engine.model / model.definition_entries_json_v1
```

## Canonical output

```json
{
  "schema": "newengine.ytyp.definition_entries.v1",
  "codec": "asset.codec.ytyp",
  "source_format": "rockstar.map_types.ytyp.xml",
  "source_encoding": "utf8.xml",
  "source": "metadata/hei_downtown_01_metadata_001.ytyp",
  "name": "hei_downtown_01_metadata_001",
  "definition_entries": [
    {
      "entry_kind": "CBaseArchetypeDef",
      "name": "hei_dt1_03_garage_lod",
      "asset_name": "hei_dt1_03_garage_lod",
      "asset_type": "ASSET_TYPE_DRAWABLE",
      "lod_dist": 50.0,
      "hd_texture_dist": 25.0,
      "flags": 0,
      "special_attribute": 0,
      "bounds": {
        "bb_min": [-5.094882, -8.727641, 0.0],
        "bb_max": [6.050597, 4.56425, 4.392919],
        "bs_centre": [0.4778576, -2.081696, 2.19646],
        "bs_radius": 8.94698
      },
      "dictionaries": {
        "texture": "DT1_03_Garage_LOD",
        "drawable": null,
        "clip": null,
        "physics": null
      }
    }
  ]
}
```

## Contract

- `.ytyp` is `definitionType`, not `plainText`. The logical contract is
  Definition Entries; XML, binary and compression are source encodings owned by
  the codec.
- Current authoring input may be legacy `CMapTypes` XML. The codec also reserves
  the native `NEYTYP01` envelope for binary/deflate payloads under the same
  `model.definition_entries_json` output.
- AssetManager dispatches it through codec registration; it does not parse XML,
  binary records or deflate streams.
- Model runtime exposes it through `engine.model` as Definition Entries.
- Future `.ydd` / `.neydd` containers can declare or reference which `.ytyp`
  metadata owns their archetype definition without changing core engine code.

## Native envelope v1

The reserved binary source envelope keeps `.ytyp` extensible without changing
the model-domain contract:

```text
u8[8] magic = NEYTYP01
u16   version = 1
u16   flags         # bit 0 = deflate payload
u16   payload_kind  # 1 = XML, 2 = canonical DefinitionEntriesManifest JSON
u16   header_len    # currently 32
u64   payload_len
u64   raw_len       # checked after inflate when non-zero
payload bytes
```

This means a future tool can emit a compressed binary `.ytyp` while runtime
consumers still ask for `model.definition_entries_json`.
