# newengine-assets-ui-runtime

Runtime-hosted semantic compiler for `.neui` UI dictionaries.

```text
engine.assets / VFS      -> bytes
NEF8 validation          -> envelope and body integrity
engine.assets.ui         -> XMLcentral semantics and compiled DTOs
engine.ui                -> mounted state, input and drawing
```

Consumers call `engine.assets.ui`; they do not parse `.neui`, NEF8, deflate, XMLcentral or VFS details.

## Internal architecture

The crate is organized by responsibility rather than by one compiler/service file:

```text
src/
├── lib.rs                     public facade and stable re-exports
├── dto.rs                     request/response DTOs
├── state.rs                   runtime caches and provider state
├── compile_helpers.rs         direct byte/XMLcentral compile helpers
├── service/
│   ├── gateway.rs             gateway declaration and router construction
│   ├── handlers.rs            shared typed request handlers
│   └── invoke.rs              legacy invoke_json compatibility route
├── compile_request/
│   ├── document.rs            document compilation pipeline
│   ├── loader.rs              NEF8 decode and XMLcentral loading
│   ├── reference.rs           reference normalization and selection
│   ├── dialect.rs             asset-backed dialect resolution
│   ├── cache.rs               explicit cache invalidation
│   └── diagnostics.rs         normalized diagnostic responses
├── node_compile/
│   ├── surface.rs             surface/layout root compilation
│   ├── node.rs                recursive node compilation
│   ├── layout.rs              layout attribute projection
│   └── span.rs                source locations
├── theme/
│   ├── surface.rs             surface and dependency metadata
│   ├── bindings.rs            binding graph compilation
│   ├── components.rs          component libraries/templates
│   └── tokens.rs              theme token resolution
├── navigation/
│   ├── derive.rs              navigation projection from layouts
│   ├── parse.rs               authored navigation documents
│   ├── action_map.rs          action-map routes
│   └── routes.rs              route/transition decoding
├── neui_dialect/
│   ├── types.rs               dialect model and asset parsing
│   ├── catalog.rs             built-in tag catalog
│   └── helpers.rs             normalization helpers
├── xml.rs                     bounded XMLcentral utilities
└── tests.rs                   crate-level integration tests
```

The typed router and compatibility `invoke_json` route share the same handlers. New methods must be implemented once in `service/handlers.rs`, then wired into both transports.

## Ownership rules

- `engine.assets` owns bytes and VFS access.
- `engine.assets.ui` owns `.neui` semantics and compiled DTOs.
- `engine.ui` owns only live runtime state, input and drawing.
- XML tag aliases belong in the dialect catalog or an asset-backed dialect, never in screen-specific branches.
- Cache invalidation is explicit through the `INVALIDATE_V1` contract.
- Public request/response types remain re-exported from the crate root.

## Validation

The crate is expected to pass:

```text
cargo fmt --all -- --check
cargo clippy -p newengine-assets-ui-runtime --lib -- -D warnings
cargo test -p newengine-assets-ui-runtime --lib
cargo check -p game-ready-fps
```

<!-- NORTHSTAR-DIR-README:BEGIN -->

## Directory purpose

**Path:** `NewEngine/neocore2/crates/newengine-assets-ui-runtime`

**Role:** Runtime semantic compiler and gateway provider for `.neui` assets.

## Working rules

- Keep public API compatibility in `lib.rs`; put implementation in responsibility-specific modules.
- Do not put transient build output in this directory.
- Do not introduce direct concrete AssetManager or renderer coupling.
- Keep runtime assets and editable source assets separate.

<!-- NORTHSTAR-DIR-README:END -->
