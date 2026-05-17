# Math and Collections Policy

## Goal

`newengine-math` is the canonical entry point for math types, math operations, and low-level runtime collections.

## Engine rule

Hot-path engine code uses `newengine-math` directly for math types and collection aliases. Domain crates and plugins should not choose their own hashers or collection backends in runtime paths.

Recommended aliases:

- `NeHashMap`, `NeHashSet`: fast internal containers.
- `NeSecureHashMap`, `NeSecureHashSet`: untrusted data from network, files, JSON, mods, or users.
- `NeBTreeMap`, `NeBTreeSet`: stable iteration and serialization order.
- `NeVecDeque`: queues, LRU, and FIFO structures.

## Plugin rule

Plugins use `newengine_math::collections_prelude`. They do not choose collection policy independently.

## Why math is not only a DLL service

Per-frame math such as dot products, matrix operations, normalization, camera calculations, frustum tests, and renderer transforms must inline. ABI or service calls per scalar operation would destroy the hot path.

Correct model: `newengine-math` is the compile-time foundation; `MathPlugin` can provide service-style extension operations on top of the same contract.

## CI audit

CI should reject new direct runtime dependencies on ad-hoc collection or math crates outside approved adapter boundaries.
