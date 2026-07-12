# newengine-assets

Runtime-facing asset gateway orchestration for North Star Engine.

Consumers use the stable `engine.assets`, `engine.assets.inspect`,
`engine.assets.edit`, and `engine.assets.types` contracts rather than linking to
a concrete AssetManager/provider implementation.

## Internal layout

```text
src/
├── asset_document_service.rs       public façade
├── asset_document_service/
│   ├── inspect.rs                  descriptor resolution and document hydration
│   ├── edit.rs                     patch validation and writer dispatch
│   ├── actions.rs                  editor action DTO construction
│   ├── sections.rs                 document sections and fields
│   ├── schema.rs                   schema/transaction projections
│   ├── path.rs                     canonical asset-reference helpers
│   ├── transport.rs                gateway services and invoke routing
│   └── tests.rs
├── asset_type_registry.rs          public façade
└── asset_type_registry/
    ├── state.rs                    descriptor storage and suffix probing
    ├── service.rs                  registration/gateway transport
    ├── path.rs                     logical path normalization
    └── tests.rs
```

## Ownership rules

- This crate owns gateway orchestration and normalized document/type DTOs.
- Format parsing and byte storage remain provider responsibilities.
- UI code must not parse asset formats directly.
- Write-back requires an explicit format/package writer capability.
- Asset references are normalized once at gateway boundaries.

## Performance policy

- `AssetServiceClient` and service method identifiers are cached in gateway
  state instead of reconstructed per inspect/edit request.
- Registered extension suffixes are maintained longest-first so type probing
  performs an allocation-free ordered suffix scan.
- Descriptor replacement preserves deterministic priority and provider-name
  tie-breaking.

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-assets`

**Role:** Stable asset gateway orchestration and provider-neutral asset document/type services.

## Working rules

- Do not place concrete codec/provider implementations here.
- Keep runtime assets and editable source assets separate.
- Add new behavior through declared gateway methods and typed DTO contracts.
- Preserve the façade exports when splitting implementation modules.

<!-- NORTHSTAR-DIR-README:END -->
