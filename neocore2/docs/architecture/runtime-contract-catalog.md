# Runtime Contract Catalog

NorthStar deliberately separates contract authority into two layers.

```text
newengine-contract-registry
    immutable normative Engine contracts
              +
plugin-published runtime contracts
              =
RuntimeContractCatalog (per HostContext / Engine instance)
```

## Normative Engine Contract Registry

`newengine-contract-registry` remains compile-time, static and strict. It owns contracts that define NorthStar itself: provider ABIs, runtime-profile/game-module ABIs, NEF8/content schemas, scripting protocols and other engine-controlled boundaries. Plugins cannot replace a normative key or advertised id.

## Runtime Contract Catalog

`newengine-runtime-contract-catalog` owns the runtime overlay. Every `HostContext` creates its own catalog seeded from the normative registry. Plugin entries are owned strings rather than `&'static str`, so DLL/module lifetimes do not leak into the catalog.

Plugin contracts participate in the same provider publication transaction as descriptors, services, event sinks and gateway routes:

```text
Load provider
  -> stage descriptor
  -> normalize declared runtime contracts
  -> validate key / advertised-id ownership
  -> init provider and stage services/routes
  -> atomic topology commit
```

Hot reload replaces same-owner contract entries in the same topology epoch. Failed replacement restores the previous provider publication, including its contract set. Unload removes only that provider's plugin contracts; normative engine contracts remain.

Provider routes that advertise `provider_abi` are admitted only when that advertised id resolves to an ABI contract in the current catalog. A plugin may therefore publish an ABI contract and a route using it in the same provider transaction; the staged contract is visible to validation but remains invisible to runtime readers until commit. Engine-owned provider ABIs such as render, physics, UI and audio stay normative entries in the static registry.

## Plugin declaration ABI

The current module ABI is not expanded merely to add a contracts vector. A plugin publishes a contract declaratively through the reserved `runtime.contract.<key>` capability namespace. `newengine-plugin-api::RuntimeContractDeclaration` owns encoding and decoding of that extensible metadata, so generic host code does not parse contract JSON itself.

Example:

```rust
use newengine_plugin_api::RuntimeContractDeclaration;
use newengine_contract_api::{ContractCompatibility, ContractKind, ContractVersion};

let capability = RuntimeContractDeclaration::new(
    "acme.streaming.protocol",
    ContractKind::Protocol,
    ContractVersion::major(1),
    ContractCompatibility::SameMajor,
)
.advertised_id("acme.streaming/v1")
.into_capability();
```

This allows extension contracts without rebuilding the engine kernel while preserving the static registry as the trust root.

`newengine-plugin-kit` keeps existing `PluginDefinition` source-compatible and exposes an optional authoring wrapper:

```rust
const CONTRACTS: &[PluginContractDefinition] = &[plugin_contract(
    "acme.streaming.protocol",
    ContractKind::Protocol,
    ContractVersion::major(1),
    ContractCompatibility::SameMajor,
    Some("acme.streaming/v1"),
)];

let descriptor = PLUGIN_DEFINITION.with_contracts(CONTRACTS).descriptor();
```

Existing plugins that do not publish contracts require no source change.

## Collision policy

- normative Engine contract keys are immutable;
- normative advertised ids are immutable;
- two different plugins cannot own the same runtime contract key;
- two different contracts cannot claim the same advertised id;
- a plugin may replace its own contract set during hot reload;
- malformed declarations fail provider transaction validation rather than partially publishing metadata.
