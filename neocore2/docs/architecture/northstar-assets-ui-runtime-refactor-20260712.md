# newengine-assets-ui-runtime refactor — 2026-07-12

## Scope

Refactored `crates/newengine-assets-ui-runtime` without changing its public request/response API or the `engine.assets.ui` gateway contract.

## Monoliths before

| File | Lines |
|---|---:|
| `src/lib.rs` | 1096 |
| `src/compile_request.rs` | 594 |
| `src/neui_dialect.rs` | 608 |
| `src/node_compile.rs` | 529 |
| `src/theme.rs` | 464 |
| `src/navigation.rs` | 348 |

## Facades after

| Facade | Lines |
|---|---:|
| `src/lib.rs` | 96 |
| `src/compile_request.rs` | 15 |
| `src/neui_dialect.rs` | 9 |
| `src/node_compile.rs` | 10 |
| `src/theme.rs` | 12 |
| `src/navigation.rs` | 11 |
| `src/service.rs` | 9 |

Implementation is distributed across responsibility-specific directories: `service`, `compile_request`, `node_compile`, `theme`, `navigation`, and `neui_dialect`.

## Repetition removed

The typed `JsonServiceRouter` and legacy `invoke_json` transport previously implemented document loading, validation, dependency extraction and compile-from-reference separately. Both now call the same functions in `service/handlers.rs`.

## Runtime/cache improvement

Decoded XMLcentral documents are cached by canonical logical reference together with the actual VFS path. An asset loaded through an `assets/` alias no longer misses the cache on the next request because the successful stripped VFS candidate differed from the authored reference.

## Cohesive large files retained

`neui_dialect/catalog.rs` remains approximately 321 lines because it is a declarative static tag/alias table, not an orchestration monolith. Crate-level integration tests remain in a dedicated test module.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p newengine-assets-ui-runtime --lib -- -D warnings`
- `cargo test -p newengine-assets-ui-runtime --lib` — 4 passed
- `cargo check -p game-ready-fps`
- `cargo build -p game-ready-fps`
- Runtime smoke: `engine.assets.ui` active, HUD compiled from `.neui`, launch gate released, public Play activated, no ERROR/FATAL records, no device loss or frame-graph/shader/material failures.

## Runtime log

`cache/logs/archive/assets-ui-runtime-refactor-final-smoke-20260712.log`
