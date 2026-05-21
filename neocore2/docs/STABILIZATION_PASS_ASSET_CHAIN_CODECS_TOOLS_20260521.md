# Stabilization Pass — AssetManager, Codecs, Asset Chain Tools

Date: 2026-05-21

## Goal

Strengthen the data-driven asset pipeline around the canonical model chain:

```text
.ytyp Definition Entries -> .ydd drawable dictionary -> .ytd texture dictionary
```

The engine core remains a dispatch host. It does not parse YTYP XML, YDD/YTD RSC7
payloads, or runtime texture packets directly. Concrete file formats are declared
by self-registering codec workers and surfaced through `engine.assets.file_types`.

## Structural changes

### AssetManager god-object split

Large AssetManager modules were split into smaller responsibility files:

- `module/service.rs` is now a thin `ServiceV1` router.
- `module/service/handlers_core.rs` owns VFS/import/format/mount methods.
- `module/service/handlers_status.rs` owns status graph/projection methods.
- `module/service/handlers_bytes.rs` owns raw bytes/decode/texture packet methods.
- `module/service/runtime.rs` owns worker/runtime/rescan state.
- `module/service/info.rs` owns service info and `invoke_json` envelope behavior.
- `module/service/texture.rs` owns runtime texture wire packet validation.
- `module/plugin/lifecycle.rs` owns plugin init, layer mounting and shutdown.
- `module/codec_loader/{host,selection,descriptor}.rs` separates codec host ABI,
  worker selection/profile policy and descriptor publication.

No AssetManager `.rs` file in the touched area remains above 550 lines.

### Canonical chain model

`newengine-model-domain-api` now owns the reusable chain table and serializable
manifest:

- `ModelAssetChainRoleSpec`
- `ModelAssetChainRole`
- `ModelAssetChainManifest`
- `MODEL_ASSET_CHAIN_ROLES`
- `model_asset_chain_role_by_extension()`
- `model_asset_chain_role_by_kind()`

`DefinitionAssetRef` now carries `role` in addition to extension/kind/path hint,
so `.ytyp` outputs do not only say “there is a `.ydd`”; they say which chain role
that file satisfies.

### Codec alignment

- `newengine-codec-ytd` now returns canonical `TextureDictionaryManifest` from
  `newengine-model-domain-api`, not an ad-hoc JSON object.
- `newengine-codec-ydd` uses the central drawable dictionary container constants.
- `newengine-codec-ytyp` remains the Definition Entries codec and emits the same
  canonical chain refs through the domain API.

### Engine model gateway

`engine.model` now exposes:

```text
model.asset_chain_json_v1
```

This returns `ModelAssetChainManifest` and gives tools/AssetBrowser one stable
place to ask what the model asset pipeline expects.

### Tooling

Added:

```text
tools/neassetchain
```

Commands:

```powershell
cargo run --manifest-path .\tools\neassetchain\Cargo.toml -- doctor
cargo run --manifest-path .\tools\neassetchain\Cargo.toml -- chain-json
cargo run --manifest-path .\tools\neassetchain\Cargo.toml -- role ytyp
```

Workspace helper also exposes:

```powershell
.\tools\workspace\NewEngine.Workspace.ps1 asset-chain
.\tools\workspace\NewEngine.Workspace.ps1 asset-chain -Plugin chain-json
```

## Why this matters

Adding a new model-chain role should start in `newengine-model-domain-api` and then
be picked up by codecs/tools. AssetManager remains a generic byte/codec dispatch
host and does not gain format-specific branching for each new file type.
