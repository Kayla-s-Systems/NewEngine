# newengine-math

Canonical math and runtime collection policy crate. Hot-path systems should use this crate instead of choosing ad-hoc math or collection dependencies.

## Architecture notes

This crate is part of the CoreEngine host/plugin architecture. Runtime-facing code should prefer engine gateways and typed adapters over concrete provider implementation crates.
