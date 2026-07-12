# NorthStar `newengine-assets` modularization — 2026-07-12

## Scope

Refactored `crates/newengine-assets` without changing its exported gateway API.
The work targeted two implementation monoliths:

- `asset_document_service.rs`: 1303 lines.
- `asset_type_registry.rs`: 396 lines.

## Resulting module layout

### Asset document service

| Module | Lines | Responsibility |
|---|---:|---|
| `asset_document_service.rs` | 84 | façade, shared DTO/state declarations, re-exports |
| `asset_document_service/inspect.rs` | 328 | descriptor resolution and document hydration |
| `asset_document_service/edit.rs` | 158 | patch validation and writer dispatch |
| `asset_document_service/actions.rs` | 150 | action DTO construction |
| `asset_document_service/sections.rs` | 289 | sections and field descriptors |
| `asset_document_service/schema.rs` | 89 | schema patch/transaction projections |
| `asset_document_service/path.rs` | 60 | asset-reference normalization |
| `asset_document_service/transport.rs` | 206 | JSON invoke routing and gateway registration |
| `asset_document_service/tests.rs` | 27 | façade/path/action regressions |

### Asset type registry

| Module | Lines | Responsibility |
|---|---:|---|
| `asset_type_registry.rs` | 52 | façade, state declaration, public re-exports |
| `asset_type_registry/state.rs` | 153 | validation, storage, priority resolution, probing |
| `asset_type_registry/service.rs` | 84 | service construction and registration |
| `asset_type_registry/path.rs` | 16 | normalized logical paths/extensions |
| `asset_type_registry/tests.rs` | 139 | provider/priority/container/suffix regressions |

## Performance changes

1. `AssetServiceClient` is cached in inspect/edit gateway state. Previously an inspect request constructed two clients and an apply request constructed one client. Each client initializes the complete cached `MethodName` table.
2. The asset-type gateway service ID and resolve method are cached instead of reconstructed for every inspected document.
3. Extension probing no longer lowercases the path twice or constructs `format!(".{extension}")` for every registered descriptor.
4. Registered extension suffixes are maintained longest-first during registration. Probe lookup is an allocation-free ordered scan and stops at the first match.
5. The document icon is resolved once and reused for the document and preview DTO.

No wall-clock performance claim is made for these changes; they remove deterministic per-request allocation and repeated lookup work.

## Compatibility

The existing public functions remain available:

- `asset_document_inspect_gateway_service`
- `asset_document_edit_gateway_service`
- `register_asset_document_gateways_best_effort`
- `asset_types_gateway_service`
- `asset_types_service_info`
- `register_asset_type_descriptor_best_effort`
- `register_asset_types_gateway_best_effort`

Public service-info DTOs remain in their historical modules.

## Validation

- `cargo fmt --all -- --check`: passed.
- `cargo test -p newengine-assets --lib`: 8 passed, 0 failed.
- `cargo clippy -p newengine-assets --lib -- -D warnings`: passed.
- `cargo check -p game-ready-fps`: passed.
- `cargo build -p game-ready-fps`: passed.
- Forest Road runtime smoke: launch gate released, public Play activated, clean shutdown, no device-loss/frame-graph/shader/material-texture failures.

The GameReady smoke profile currently reports `engine.assets.inspect` as an optional unregistered capability; therefore the smoke validates engine integration and linkage, while the asset gateway logic itself is covered by crate tests and compilation.
