# Model Construction Gateways

## Goal

Model construction is now a domain service, not a player-only helper path. The engine-facing entry point is `engine.model`; player, NPC and prop construction should request the same `ModelAssetRequest` and receive the same `ModelAssetBundle` shape.

This keeps the project aligned with the architecture rule:

```text
engine as host, service as plugin
```

The host owns the stable gateway. Runtime/profile code selects and registers the provider. Importers and mappers stay replaceable behind the service boundary.

## Crate split

```text
newengine-model-domain-api
  ModelConstructionManifest
  ModelAssetRequest
  ModelAssetBundle
  ModelMeshPart
  ModelMaterialBinding
  ModelCollisionRef
  engine.model constants and method ids

newengine-model-runtime
  ModelAssetAdapter
  engine.model ServiceV1 gateway
  ModelGatewayClient
  bundle assembly
  AssetServiceClient dependency injection

newengine-model-import-obj
  OBJ parser
  MTL parser
  ObjPart
  ModelMaterialSource
  mesh normalization

newengine-model-skeleton-api
  ModelSkeletonMetadata
  ModelSkeletonJointMetadata
  ModelSkeletonAnchors

newengine-model-skeleton-rsc7
  opaque RSC7/YMT probe
  humanoid fallback anchor derivation

newengine-model-collision-runtime
  default collision derivation
  humanoid capsule mapping

newengine-material-runtime
  ModelMaterialSource -> MaterialDescriptor
  texture dictionary selector resolution
```

## Host wiring

`ModelAssetAdapter` no longer calls `default_host_api()`. It is constructed only with an injected asset client:

```rust
let adapter = ModelAssetAdapter::with_client(asset_client);
```

Product/profile integration owns host wiring:

```rust
newengine_model_runtime::register_model_gateway_best_effort(
    newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api()),
);
```

This is deliberate. Pure adapters must not discover host services by themselves.

## Runtime use

Gameplay/runtime code should call the gateway client:

```rust
let constructor = ModelGatewayClient::new(newengine_plugin_host::default_host_api());
let bundle = constructor.assemble_bundle(&request)?;
```

That call goes through `engine.model`, so a future plugin can replace model construction without rewriting player/NPC construction code.

## Method contract

`engine.model` / `model.api` exposes:

```text
info_json
invoke_json
assemble_json_v1
validate_json_v1
shutdown_v1
```

`assemble_json_v1` accepts `ModelAssetRequest` and returns `ModelAssetBundle`.

`validate_json_v1` accepts `ModelAssetRequest` and returns `ModelConstructionValidation`.

## Player and NPC construction

Player construction no longer creates `ModelAssetAdapter::new()`. It builds a `ModelAssetRequest` and sends it to `engine.model`. NPC construction should use the same route. Differences between player and NPC should be expressed in manifests, skeleton refs, material sets, collision refs and controller/AI components, not in separate asset helper code.

