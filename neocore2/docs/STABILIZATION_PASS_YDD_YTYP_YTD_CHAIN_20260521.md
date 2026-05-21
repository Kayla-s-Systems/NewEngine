# Stabilization pass — YTYP / YDD / YTD canonical chain

## Goal

Make the model/asset metadata path declarative and data-driven:

```text
.ytyp Definition Entries
  -> .ydd drawable dictionary
  -> .ytd texture dictionary role
  -> imported/compiled .neytd runtime texture packets
```

`.ydd` is the only drawable dictionary container in this model. There is no
parallel drawable extension. `.ytd` is the texture dictionary role referenced by
Definition Entries. `.neytd` remains the NewEngine runtime-ready texture packet
container selected through AssetManager texture dictionary APIs.

## Code changes

- Split `newengine-model-domain-api/src/lib.rs` into isolated contract modules:
  - `definition.rs` — Definition Entries, dictionary references and canonical asset chain.
  - `drawable.rs` — `.ydd` drawable dictionary request/manifest DTOs.
  - `construction.rs` — model bundle/construction request DTOs.
- Split `newengine-codec-ytyp/src/lib.rs` into isolated codec modules:
  - `binary.rs` — `NEYTYP01` binary/deflate envelope.
  - `xml.rs` — legacy `CMapTypes` XML parsing.
  - `manifest.rs` — canonical manifest normalization and selector filtering.
  - `tests.rs` — codec contract tests.
- Removed duplicate local YTYP DTO definitions from the codec worker. The codec now serializes the canonical DTOs from `newengine-model-domain-api`.
- Updated `.ydd` codec to emit the canonical `DrawableDictionaryManifest` DTO from `newengine-model-domain-api` instead of an ad-hoc JSON shape.
- Added `newengine-codec-ytd` as a first-party source/domain texture dictionary codec. It validates the RSC7 boundary and exposes source manifest/raw outputs. Runtime GPU packets still come from `.neytd`.

## Central model

`DefinitionEntry` now contains both the original dictionary names and a normalized `asset_chain`:

```text
DefinitionEntry.dictionaries.texture     -> source dictionary name from YTYP
DefinitionEntry.asset_chain.texture      -> .ytd texture dictionary role
DefinitionEntry.asset_chain.drawable     -> .ydd drawable dictionary role
DefinitionEntry.asset_chain.definition   -> .ytyp owner metadata
```

This keeps the raw source facts and the engine-facing chain in one canonical DTO.
Downstream code should consume the chain instead of rebuilding extension rules in
scene bootstrap, renderer code or AssetManager.

## Data-driven invariant

Adding a new codec must mean adding a self-registering codec worker or provider,
not editing a central switch/table inside AssetManager. The file-type registry
remains a descriptor registry only.
