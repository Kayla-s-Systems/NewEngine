# NewEngine collections policy

All engine crates and runtime plugins must route hash/collection choices through `newengine_math::collections_prelude` or a crate-local policy module that wraps it.

## Rules

- Do not import `std::collections::{HashMap, HashSet}` for engine/plugin runtime state.
- Do not import `hashbrown`, `fxhash`, `ahash` or other hashers outside `newengine-math`.
- Use `NeHashMap` / `NeHashSet` for internal deterministic fast state.
- Use `NeSecureHashMap` / `NeSecureHashSet` for untrusted input from network, user files, JSON, mods or editor scripting.
- Do not call `HashMap::new()` / `HashSet::new()` on engine aliases. Use `ne_hash_map()`, `ne_hash_set()`, `ne_*_with_capacity()` or `Default::default()`.
- Stable serialization/order must use `NeBTreeMap` / `NeBTreeSet` or an explicit sort step. Hash iteration order is not an engine determinism contract.

## Plugin-local policy

Large plugins should expose a tiny internal module, for example:

```rust
pub(crate) type AssetMap<K, V> = newengine_math::collections_prelude::NeHashMap<K, V>;
pub(crate) type AssetSet<T> = newengine_math::collections_prelude::NeHashSet<T>;
```

This lets a plugin standardize naming without leaking raw collection choices through every subsystem.
