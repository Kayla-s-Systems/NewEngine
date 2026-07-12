# NorthStar input family modularization — 2026-07-12

## Scope

- `newengine-input-profile-gameready`
- `newengine-input-contexts-api`
- `newengine-input-bindings-runtime`
- `newengine-input-bindings-api`
- `newengine-input-api`

## Before and after

| Crate | Previous `lib.rs` | New facade | Source files | Largest implementation file |
|---|---:|---:|---:|---:|
| `newengine-input-profile-gameready` | 695 | 34 | 9 | 174 |
| `newengine-input-contexts-api` | 191 | 16 | 5 | 92 |
| `newengine-input-bindings-runtime` | 413 | 50 | 7 | 114 |
| `newengine-input-bindings-api` | 952 | 35 | 12 | 194 |
| `newengine-input-api` | 309 | 19 | 10 | 143 |

All crate roots are compatibility facades. Historical root-level public types, constants, functions and public device modules remain re-exported.

## Responsibility boundaries

### Raw input API

`newengine-input-api` now separates service contracts, canonical key codes, key identities, mouse/gamepad vocabularies and provider snapshots.

### Context contract

`newengine-input-contexts-api` separates context-stack semantics, modal capture DTOs and service metadata.

### Bindings API

`newengine-input-bindings-api` separates contracts, DTOs, profile canonicalization, profile mutation, queries, frame resolution and display labels. The previous 310-line profile implementation is further split into `profile/canonicalize.rs`, `profile/mutate.rs` and `profile/query.rs`.

### Bindings runtime

`newengine-input-bindings-runtime` separates state ownership, persistence, service routing, public mutation API and gateway registration.

### GameReady profile

`newengine-input-profile-gameready` separates action identifiers, key catalog, semantic action catalog, listener policy, button bindings, axes and product profile assembly.

## Performance changes

1. `InputBindingsProfile::resolve` no longer creates a `BTreeMap` action catalog and `BTreeSet<String>` duplicate set on every input frame. It resolves definitions directly from the canonical profile and uses the already-produced action frame for duplicate suppression.
2. `save_input_bindings_profile` no longer reacquires the global bindings mutex to clone the profile after mutation. Path and snapshot are captured during the original critical section and persisted after releasing the lock.

## Validation

Successful targeted validation during this wave:

- per-package `cargo fmt -- --check`: all five crates passed;
- `newengine-input-api`: 3 tests passed;
- `newengine-input-contexts-api`: 2 tests passed;
- `newengine-input-bindings-api`: 3 tests passed;
- `newengine-input-bindings-runtime`: 2 tests passed;
- `newengine-input-profile-gameready`: 4 tests passed;
- per-package Clippy with `-D warnings`: passed for all five crates (`--no-deps` for runtime/profile dependency isolation);
- `git diff --check` for the five target crates: passed.

A later clean aggregate rebuild is currently blocked outside this scope by concurrent/incomplete changes:

- `newengine-plugin-host` imports `newengine_ulog_api::formatting` and `newengine_ulog_api::path_format`, which are not currently exported by that crate;
- `newengine-engine-runtime` has unresolved `default_*` functions in `scene_bridge/game_ready/content_parts/raw_payload.rs` while its defaults split is incomplete.

Consequently this wave does not claim a fresh `game-ready-fps` runtime smoke. The five target crates were independently formatted, tested and linted before the unrelated aggregate blockers appeared.

## Repository state

No commit was created. The repository was already dirty before this work. Existing user modifications were not reset. A pre-refactor source backup is stored under:

`cache/refactor_backups/input-family-monoliths-20260712`
